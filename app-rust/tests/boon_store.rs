use grapevine_workbench::{
    boon_book::{self, Edit},
    importer,
    model::{BuildKind, Role},
    store::Library,
    xp::{Baseline, Ledger},
};
use rusqlite::Connection;
use serde_json::{Value, json};

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

const XML: &[u8] = br#"<?xml version="1.0"?><grapevine version="3">
  <player name="Owner" id="p-1"/>
  <vampire name="XML Holder" id="v-xml" player="Owner">
    <boon type="Major" partner="Keeper" owed="unclear" date="not-a-date">
      <description>Original private wording</description><unknown keep="true">XML_UNKNOWN</unknown>
    </boon>
    <boon type="Major" partner="Keeper" owed="unclear" date="not-a-date">
      <description>Original private wording</description><unknown keep="true">XML_UNKNOWN</unknown>
    </boon>
  </vampire>
  <vampire name="Binary Holder" id="v-bin"/>
  <werewolf name="Wrong Kind" id="w-1"/>
</grapevine>"#;

fn binary_boons() -> Value {
    let row = json!({
        "BoonType":"Major",
        "CharName":"Keeper",
        "IsOwed":{"value":true,"vb_raw":-1},
        "BoonDate":{"ole_automation_days":46000.25},
        "Description":"Binary private wording",
        "unmapped":{"keep":"BINARY_UNKNOWN"}
    });
    json!([row.clone(), row])
}

fn library_with_sources(path: &std::path::Path) -> (Library, String, String, String, String) {
    let mut preview = importer::normalize(XML, "synthetic.gex", "windows-1252").unwrap();
    let mut xml_id = String::new();
    let mut binary_id = String::new();
    let mut player_id = String::new();
    let mut wrong_id = String::new();
    for record in &mut preview.records {
        match (record.kind.as_str(), record.name.as_str()) {
            ("Vampire", "XML Holder") => xml_id.clone_from(&record.id),
            ("Vampire", "Binary Holder") => {
                binary_id.clone_from(&record.id);
                record
                    .private_fields
                    .insert("BoonList".into(), binary_boons());
                record
                    .private_fields
                    .insert("UnrelatedPrivate".into(), json!("PRIVATE_KEEP"));
                record
                    .public_fields
                    .insert("UnrelatedPublic".into(), json!("PUBLIC_KEEP"));
                record.xp_ledger = Some(Ledger {
                    baseline: Baseline {
                        day: "2026-01-01".into(),
                        earned: "10".into(),
                        unspent: "7".into(),
                        actor: "ST".into(),
                        reason: "Synthetic opening".into(),
                        evidence: "fixture".into(),
                    },
                    entries: Vec::new(),
                });
            }
            ("Player", "Owner") => player_id.clone_from(&record.id),
            ("Werewolf", "Wrong Kind") => wrong_id.clone_from(&record.id),
            _ => {}
        }
    }
    assert!(
        !xml_id.is_empty()
            && !binary_id.is_empty()
            && !player_id.is_empty()
            && !wrong_id.is_empty()
    );
    let mut library = Library::create(path, "Synthetic Boons").unwrap();
    library.commit_import(&preview, 0, false, ST).unwrap();
    (library, xml_id, binary_id, player_id, wrong_id)
}

fn raw_record(path: &std::path::Path, id: &str) -> Value {
    let sql = Connection::open(path).unwrap();
    let data: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    serde_json::from_str(&data).unwrap()
}

fn edit(partner: &str, description: &str) -> Edit {
    Edit {
        partner: partner.into(),
        kind: "Major".into(),
        owed_by_holder: Some(true),
        day: "2026-11-11".into(),
        description: description.into(),
    }
}

#[test]
fn binary_entries_adopt_stably_preserve_unknowns_and_survive_backup_restore() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boons.gvlib");
    let backup = dir.path().join("boons-backup.gvlib");
    let (mut lib, _xml_id, binary_id, _player_id, _) = library_with_sources(&path);
    let source_before = raw_record(&path, &binary_id);
    let original = lib.boon_snapshot(&binary_id, ST).unwrap();
    assert_eq!(original.entries.len(), 2);
    assert_ne!(original.entries[0].id, original.entries[1].id);
    assert_eq!(
        original.entries[0].legacy.as_ref().unwrap().owed_by_holder,
        Some(true)
    );
    assert_eq!(
        original.entries[0].legacy.as_ref().unwrap().raw_date,
        json!({"ole_automation_days":46000.25})
    );
    assert_eq!(
        original.entries[0].legacy.as_ref().unwrap().original["unmapped"]["keep"],
        "BINARY_UNKNOWN"
    );
    assert_eq!(original.entries[0].edit.partner, "Keeper");

    let new_id = lib
        .add_boon(
            &binary_id,
            &edit("New Partner", "New private wording"),
            1,
            ST,
        )
        .unwrap();
    let snapshot = lib.boon_snapshot(&binary_id, ST).unwrap();
    assert_eq!(snapshot.revision, 2);
    assert_eq!(snapshot.entries.len(), 3);
    assert!(snapshot.entries.iter().any(|entry| entry.id == new_id));
    for prior in &original.entries {
        assert!(snapshot.entries.iter().any(|entry| entry.id == prior.id));
    }

    // Seed forward-version fields in the serialized overlay; an edit must retain them.
    {
        let sql = Connection::open(&path).unwrap();
        let mut data: Value = sql
            .query_row(
                "SELECT data FROM records WHERE id=?1",
                [&binary_id],
                |row| row.get::<_, String>(0),
            )
            .map(|text| serde_json::from_str(&text).unwrap())
            .unwrap();
        let overlay = &mut data["private_fields"][boon_book::FIELD];
        overlay["future_state"] = json!({"keep":true});
        overlay["entries"][0]["future_entry"] = json!("ENTRY_KEEP");
        overlay["entries"][0]["edit"]["future_edit"] = json!({"keep":1});
        sql.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![data.to_string(), binary_id],
        )
        .unwrap();
    }
    let mut old_legacy = snapshot.entries[0].edit.clone();
    old_legacy.description = "Edited private wording".into();
    lib.edit_boon(&binary_id, &snapshot.entries[0].id, &old_legacy, 2, ST)
        .unwrap();
    lib.backup(&backup, ST).unwrap();

    let after_edit = lib.boon_snapshot(&binary_id, ST).unwrap();
    let removed_id = after_edit.entries[0].id.clone();
    lib.remove_boon(&binary_id, &removed_id, 3, ST).unwrap();
    let removed = lib.boon_snapshot(&binary_id, ST).unwrap();
    assert_eq!(removed.revision, 4);
    assert_eq!(removed.entries.len(), 2);
    assert_eq!(removed.removed_count, 1);

    drop(lib);
    let mut lib = Library::open(&path).unwrap();
    assert_eq!(lib.boon_snapshot(&binary_id, ST).unwrap().removed_count, 1);
    let saved = raw_record(&path, &binary_id);
    assert_eq!(
        saved["private_fields"][boon_book::FIELD]["future_state"]["keep"],
        true
    );
    assert_eq!(
        saved["private_fields"][boon_book::FIELD]["removed"][0]["future_entry"],
        "ENTRY_KEEP"
    );
    assert_eq!(
        saved["private_fields"][boon_book::FIELD]["removed"][0]["edit"]["future_edit"]["keep"],
        1
    );
    assert_eq!(saved["original"], source_before["original"]);
    assert_eq!(saved["source_sha256"], source_before["source_sha256"]);
    assert_eq!(
        saved["private_fields"]["BoonList"],
        source_before["private_fields"]["BoonList"]
    );
    assert_eq!(saved["public_fields"]["UnrelatedPublic"], "PUBLIC_KEEP");
    assert_eq!(saved["private_fields"]["UnrelatedPrivate"], "PRIVATE_KEEP");
    assert_eq!(
        saved["xp_ledger"]["baseline"]["reason"],
        "Synthetic opening"
    );
    let raw_source: Vec<u8> = Connection::open(&path)
        .unwrap()
        .query_row("SELECT source_bytes FROM imports", [], |row| row.get(0))
        .unwrap();
    assert_eq!(raw_source, XML);

    lib.restore(&backup, 4, ST).unwrap();
    let restored = lib.boon_snapshot(&binary_id, ST).unwrap();
    assert_eq!(restored.revision, 5);
    assert_eq!(restored.entries.len(), 3);
    assert_eq!(restored.removed_count, 0);
    assert!(restored.entries.iter().any(|entry| entry.id == removed_id));
}

#[test]
fn xml_unknown_fields_remain_unknown_until_explicitly_edited_and_removed_as_tombstones() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("xml-boons.gvlib");
    let (mut lib, xml_id, _, _, _) = library_with_sources(&path);
    let snapshot = lib.boon_snapshot(&xml_id, ST).unwrap();
    assert_eq!(snapshot.entries.len(), 2);
    let first = &snapshot.entries[0];
    assert_eq!(first.edit.day, "");
    assert_eq!(first.edit.owed_by_holder, None);
    assert_eq!(first.legacy.as_ref().unwrap().raw_date, "not-a-date");
    assert!(
        first
            .legacy
            .as_ref()
            .unwrap()
            .original
            .to_string()
            .contains("XML_UNKNOWN")
    );

    let mut unchanged_unknowns = first.edit.clone();
    unchanged_unknowns.description = "Clarified without guessing date or direction".into();
    assert_eq!(
        lib.edit_boon(&xml_id, &first.id, &unchanged_unknowns, 1, ST)
            .unwrap(),
        2
    );
    let before_invalid = lib.boon_snapshot(&xml_id, ST).unwrap();
    let mut invalid_date = before_invalid.entries[0].edit.clone();
    invalid_date.day = "2026-02-30".into();
    assert_eq!(
        lib.edit_boon(&xml_id, &first.id, &invalid_date, 2, ST)
            .unwrap_err()
            .code,
        "BOON_EDIT_INVALID"
    );
    assert_eq!(lib.revision().unwrap(), 2);
    assert_eq!(
        lib.boon_snapshot(&xml_id, ST).unwrap().entries[0].edit.day,
        ""
    );

    lib.remove_boon(&xml_id, &first.id, 2, ST).unwrap();
    let after_remove = lib.boon_snapshot(&xml_id, ST).unwrap();
    assert_eq!(after_remove.entries.len(), 1);
    assert_eq!(after_remove.removed_count, 1);
    drop(lib);
    let lib = Library::open(&path).unwrap();
    assert_eq!(lib.boon_snapshot(&xml_id, ST).unwrap().removed_count, 1);
}

#[test]
fn authorization_wrong_type_stale_validation_and_build_denials_leave_no_partial_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("guards.gvlib");
    let (mut lib, xml_id, _, player_id, wrong_id) = library_with_sources(&path);
    assert_eq!(
        lib.boon_snapshot(&xml_id, PLAYER).unwrap_err().code,
        "ACCESS_DENIED"
    );
    assert_eq!(
        lib.boon_snapshot(&player_id, ST).unwrap_err().code,
        "BOON_HOLDER_REQUIRED"
    );
    assert_eq!(
        lib.boon_snapshot(&wrong_id, ST).unwrap_err().code,
        "BOON_HOLDER_REQUIRED"
    );

    let start_revision = lib.revision().unwrap();
    let mut invalid = edit("Partner", "No guessed date");
    invalid.day.clear();
    assert_eq!(
        lib.add_boon(&xml_id, &invalid, start_revision, ST)
            .unwrap_err()
            .code,
        "BOON_EDIT_INVALID"
    );
    assert_eq!(
        lib.add_boon(&xml_id, &edit("Partner", "Blocked"), start_revision, PLAYER)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert_eq!(lib.revision().unwrap(), start_revision);
    assert_eq!(lib.boon_snapshot(&xml_id, ST).unwrap().entries.len(), 2);

    let created = lib
        .add_boon(&xml_id, &edit("Partner", "Saved"), start_revision, ST)
        .unwrap();
    assert_eq!(
        lib.add_boon(&xml_id, &edit("Another", "Stale"), start_revision, ST)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert_eq!(lib.revision().unwrap(), start_revision + 1);
    let missing = lib.remove_boon(&xml_id, "missing-boon", start_revision + 1, ST);
    assert_eq!(missing.unwrap_err().code, "BOON_NOT_FOUND");
    assert_eq!(lib.revision().unwrap(), start_revision + 1);

    let build_id = lib
        .create_build(&xml_id, BuildKind::Draft, start_revision + 1, ST)
        .unwrap();
    assert_eq!(
        lib.boon_snapshot(&build_id, ST).unwrap_err().code,
        "BOON_HOLDER_REQUIRED"
    );
    assert_eq!(
        lib.remove_boon(&build_id, &created, start_revision + 2, ST)
            .unwrap_err()
            .code,
        "BOON_HOLDER_REQUIRED"
    );
    let build: Value = {
        let sql = Connection::open(&path).unwrap();
        let data: String = sql
            .query_row("SELECT data FROM records WHERE id=?1", [&build_id], |row| {
                row.get(0)
            })
            .unwrap();
        serde_json::from_str(&data).unwrap()
    };
    assert!(build["private_fields"].get(boon_book::FIELD).is_none());
    assert_eq!(lib.revision().unwrap(), start_revision + 2);
    assert_eq!(lib.boon_snapshot(&xml_id, ST).unwrap().entries.len(), 3);

    let audit: Vec<String> = {
        let sql = Connection::open(&path).unwrap();
        let mut statement = sql
            .prepare("SELECT operation FROM audit WHERE record_id=?1 ORDER BY revision")
            .unwrap();
        statement
            .query_map([&xml_id], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    assert_eq!(audit, ["create_boon"]);
}

#[test]
fn malformed_app_owned_overlay_is_rejected_without_repairing_or_partially_writing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("malformed-overlay.gvlib");
    let (mut lib, xml_id, _, _, _) = library_with_sources(&path);
    let created = lib
        .add_boon(&xml_id, &edit("Partner", "Saved"), 1, ST)
        .unwrap();
    let mut data = raw_record(&path, &xml_id);
    let entries = data["private_fields"][boon_book::FIELD]["entries"]
        .as_array_mut()
        .unwrap();
    let new_entry = entries
        .iter_mut()
        .find(|entry| entry["id"] == created)
        .unwrap();
    new_entry["edit"]["day"] = json!("");
    {
        let sql = Connection::open(&path).unwrap();
        sql.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![data.to_string(), xml_id],
        )
        .unwrap();
    }
    let corrupted = raw_record(&path, &xml_id);
    assert_eq!(
        lib.boon_snapshot(&xml_id, ST).unwrap_err().code,
        "BOON_OVERLAY_INVALID"
    );
    assert_eq!(
        lib.add_boon(&xml_id, &edit("Another", "No write"), 2, ST)
            .unwrap_err()
            .code,
        "BOON_OVERLAY_INVALID"
    );
    assert_eq!(lib.revision().unwrap(), 2);
    assert_eq!(raw_record(&path, &xml_id), corrupted);
}

#[test]
fn explicit_empty_overlay_suppresses_legacy_adoption() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty-overlay.gvlib");
    let (mut lib, xml_id, _, _, _) = library_with_sources(&path);
    let mut data = raw_record(&path, &xml_id);
    data["private_fields"][boon_book::FIELD] = json!({
        "entries":[],
        "removed":[],
        "future_state":{"keep":true}
    });
    {
        let sql = Connection::open(&path).unwrap();
        sql.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![data.to_string(), xml_id],
        )
        .unwrap();
    }
    assert!(lib.boon_snapshot(&xml_id, ST).unwrap().entries.is_empty());
    let id = lib
        .add_boon(&xml_id, &edit("Partner", "Explicit list"), 1, ST)
        .unwrap();
    let snapshot = lib.boon_snapshot(&xml_id, ST).unwrap();
    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].id, id);
    assert!(snapshot.entries[0].legacy.is_none());
    assert_eq!(
        raw_record(&path, &xml_id)["private_fields"][boon_book::FIELD]["future_state"]["keep"],
        true
    );
}
