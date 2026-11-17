use grapevine_workbench::{
    actions::Action,
    model::{BuildKind, Edit, Record, Role},
    player_profiles::Profile as PlayerProfile,
    rumor_audience::{self, Preview},
    rumors::{Entry, Rumor},
    store::Library,
    story_work::Draft,
    world::Edit as WorldEdit,
};
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use tempfile::TempDir;
use uuid::Uuid;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

fn make_rumor(library: &mut Library, title: &str, day: &str, revision: i64) -> String {
    library
        .create_story(
            &Draft::Rumor(Rumor {
                title: title.into(),
                day: day.into(),
                ..Rumor::default()
            }),
            revision,
            ST,
        )
        .unwrap()
}

fn read_record(path: &std::path::Path, id: &str) -> Value {
    let conn = Connection::open(path).unwrap();
    let data: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    serde_json::from_str(&data).unwrap()
}

fn write_record(path: &std::path::Path, id: &str, mut edit: impl FnMut(&mut Value)) {
    let conn = Connection::open(path).unwrap();
    let mut data = read_record(path, id);
    edit(&mut data);
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        params![data.to_string(), id],
    )
    .unwrap();
}

fn set_rumor_day(path: &std::path::Path, id: &str, day: &str) {
    write_record(path, id, |data| {
        data["private_fields"]["AppRumorV1"]["day"] = json!(day);
    });
}

fn selected_preview(library: &Library, rumor_id: &str, character_ids: &[String]) -> Preview {
    library
        .preview_rumor_audience(rumor_id, character_ids, library.revision().unwrap(), ST)
        .unwrap()
}

#[test]
fn audience_preview_uses_ids_and_preserves_labels_unknown_fields_and_legacy_source() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("audience.gvlib");
    let mut library = Library::create(&path, "Audience Chronicle").unwrap();
    let first = library
        .create_character("Vampire", "Duplicate Name", 0, ST)
        .unwrap();
    let second = library
        .create_character("Werewolf", "Duplicate Name", 1, ST)
        .unwrap();
    let rumor_id = make_rumor(&mut library, "Synthetic Rumor", "2026-11-01", 2);
    let app_rumor = read_record(&path, &rumor_id)["private_fields"]["AppRumorV1"].clone();
    let source_evidence = json!({"fields":{"Done":true,"Query":{"future":"retained"}}});
    write_record(&path, &rumor_id, |record| {
        record["original"] = source_evidence.clone();
        record["private_fields"][rumor_audience::FIELD] = json!({
            "recipients": [],
            "future_root": {"keep": "root"}
        });
    });

    let ids = vec![second.clone(), first.clone()];
    let preview = selected_preview(&library, &rumor_id, &ids);
    let mut canonical_ids = ids.clone();
    canonical_ids.sort();
    assert_eq!(preview.character_ids, canonical_ids);
    assert!(preview.legacy && preview.retained);
    assert_eq!(preview.recipients.len(), 2);
    assert!(
        preview
            .recipients
            .iter()
            .all(|recipient| recipient.binding.label_at_binding == "Duplicate Name")
    );
    assert_eq!(library.revision().unwrap(), 3);
    assert!(
        library
            .rumor_audience_snapshot(&rumor_id, ST)
            .unwrap()
            .recipients
            .is_empty()
    );

    assert_eq!(library.commit_rumor_audience(&preview, ST).unwrap(), 4);
    let after_commit = read_record(&path, &rumor_id);
    assert_eq!(after_commit["original"], source_evidence);
    assert_eq!(after_commit["private_fields"]["AppRumorV1"], app_rumor);
    assert_eq!(
        after_commit["private_fields"][rumor_audience::FIELD]["future_root"],
        json!({"keep":"root"})
    );

    write_record(&path, &rumor_id, |record| {
        for binding in record["private_fields"][rumor_audience::FIELD]["recipients"]
            .as_array_mut()
            .unwrap()
        {
            binding["future_binding"] = json!({"keep": binding["character_id"].clone()});
        }
    });
    let again = selected_preview(&library, &rumor_id, &ids);
    assert_eq!(library.commit_rumor_audience(&again, ST).unwrap(), 5);
    let after_unknown = read_record(&path, &rumor_id);
    assert_eq!(
        after_unknown["private_fields"][rumor_audience::FIELD]["future_root"],
        json!({"keep":"root"})
    );
    assert!(
        after_unknown["private_fields"][rumor_audience::FIELD]["recipients"]
            .as_array()
            .unwrap()
            .iter()
            .all(|binding| binding["future_binding"]["keep"] == binding["character_id"])
    );

    let sheet = library.sheet_edit(&first, ST).unwrap();
    library
        .save_character(
            &first,
            &Edit {
                name: "Renamed Character".into(),
                status: "Draft".into(),
                notes: String::new(),
            },
            &sheet,
            5,
            ST,
        )
        .unwrap();
    let snapshot = library.rumor_audience_snapshot(&rumor_id, ST).unwrap();
    let renamed = snapshot
        .recipients
        .iter()
        .find(|recipient| recipient.binding.character_id == first)
        .unwrap();
    assert_eq!(renamed.binding.label_at_binding, "Duplicate Name");
    assert_eq!(renamed.current_name.as_deref(), Some("Renamed Character"));

    let mut rumor = match library.story_snapshot(&rumor_id, ST).unwrap().draft {
        Draft::Rumor(rumor) => rumor,
        Draft::Action(_) => unreachable!(),
    };
    rumor.reviewed = true;
    rumor.entries.push(Entry {
        level: "0".into(),
        body: "Reviewed content".into(),
    });
    library
        .save_story(&rumor_id, &Draft::Rumor(rumor), 6, ST)
        .unwrap();
    assert_eq!(
        library
            .rumor_audience_snapshot(&rumor_id, ST)
            .unwrap()
            .recipients
            .len(),
        2
    );
    let after_old_save = read_record(&path, &rumor_id);
    assert_eq!(after_old_save["original"], source_evidence);
    assert_eq!(
        after_old_save["private_fields"][rumor_audience::FIELD]["future_root"],
        json!({"keep":"root"})
    );
}

#[test]
fn old_missing_binding_can_be_kept_or_explicitly_cleared() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("missing-target.gvlib");
    let mut library = Library::create(&path, "Missing Target Chronicle").unwrap();
    let character = library
        .create_character("Vampire", "Historical Name", 0, ST)
        .unwrap();
    let rumor_id = make_rumor(&mut library, "Rumor", "2026-11-01", 1);
    let preview = selected_preview(&library, &rumor_id, std::slice::from_ref(&character));
    assert_eq!(library.commit_rumor_audience(&preview, ST).unwrap(), 3);

    let conn = Connection::open(&path).unwrap();
    conn.execute("DELETE FROM records WHERE id=?1", [&character])
        .unwrap();
    let missing = library.rumor_audience_snapshot(&rumor_id, ST).unwrap();
    assert_eq!(missing.recipients[0].current_name, None);
    assert_eq!(
        missing.recipients[0].binding.label_at_binding,
        "Historical Name"
    );

    let keep = selected_preview(&library, &rumor_id, std::slice::from_ref(&character));
    assert_eq!(keep.recipients[0].current_name, None);
    assert_eq!(library.commit_rumor_audience(&keep, ST).unwrap(), 4);
    let clear = selected_preview(&library, &rumor_id, &[]);
    assert!(clear.recipients.is_empty());
    assert_eq!(library.commit_rumor_audience(&clear, ST).unwrap(), 5);
    assert!(
        library
            .rumor_audience_snapshot(&rumor_id, ST)
            .unwrap()
            .recipients
            .is_empty()
    );
}

#[test]
fn new_targets_roles_staleness_tampering_and_corrupt_metadata_are_rejected() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("denials.gvlib");
    let mut library = Library::create(&path, "Denials Chronicle").unwrap();
    let vampire = library
        .create_character("Vampire", "Valid Character", 0, ST)
        .unwrap();
    let player = library
        .create_player_profile(
            &PlayerProfile {
                name: "Not A Character".into(),
                ..PlayerProfile::default()
            },
            1,
            ST,
        )
        .unwrap();
    let build = library
        .create_build(&vampire, BuildKind::Draft, 2, ST)
        .unwrap();
    let item = library
        .create_world(
            "Item",
            &WorldEdit {
                name: "Not A Character".into(),
                fields: BTreeMap::new(),
            },
            3,
            ST,
        )
        .unwrap();
    let rumor_id = make_rumor(&mut library, "Rumor", "2026-11-01", 4);
    let revision = library.revision().unwrap();
    let missing = Uuid::new_v4().to_string();

    for invalid in [&player, &build, &item, &missing] {
        assert!(
            library
                .preview_rumor_audience(&rumor_id, std::slice::from_ref(invalid), revision, ST)
                .is_err()
        );
    }
    assert!(
        library
            .preview_rumor_audience(&rumor_id, &[vampire.clone(), vampire.clone()], revision, ST,)
            .is_err()
    );
    assert!(
        library
            .preview_rumor_audience(&rumor_id, &[], revision - 1, ST)
            .is_err()
    );
    assert!(library.rumor_audience_snapshot(&rumor_id, PLAYER).is_err());
    assert!(
        library
            .preview_rumor_audience(&rumor_id, &[], revision, PLAYER)
            .is_err()
    );
    assert_eq!(library.revision().unwrap(), revision);

    let good = selected_preview(&library, &rumor_id, std::slice::from_ref(&vampire));
    let mut tampered = good.clone();
    tampered.rumor.title.push_str(" tampered");
    assert_eq!(
        library
            .commit_rumor_audience(&tampered, ST)
            .unwrap_err()
            .code,
        "RUMOR_AUDIENCE_PREVIEW_CHANGED"
    );
    let mut tampered_ids = good.clone();
    tampered_ids.character_ids.clear();
    assert_eq!(
        library
            .commit_rumor_audience(&tampered_ids, ST)
            .unwrap_err()
            .code,
        "RUMOR_AUDIENCE_PREVIEW_CHANGED"
    );
    assert_eq!(library.revision().unwrap(), revision);

    write_record(&path, &rumor_id, |record| {
        record["private_fields"][rumor_audience::FIELD] = json!("malformed");
    });
    assert_eq!(
        library
            .rumor_audience_snapshot(&rumor_id, ST)
            .unwrap_err()
            .code,
        "RUMOR_AUDIENCE_CORRUPT"
    );
    assert!(library.commit_rumor_audience(&good, ST).is_err());
    assert_eq!(library.revision().unwrap(), revision);
}

#[test]
fn failed_audience_write_rolls_back_revision_audit_and_selection() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("rollback.gvlib");
    let mut library = Library::create(&path, "Rollback Chronicle").unwrap();
    let character = library
        .create_character("Vampire", "Target", 0, ST)
        .unwrap();
    let rumor_id = make_rumor(&mut library, "Rumor", "2026-11-01", 1);
    let preview = selected_preview(&library, &rumor_id, std::slice::from_ref(&character));
    let before_revision = library.revision().unwrap();
    let conn = Connection::open(&path).unwrap();
    let before_audit: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
        .unwrap();
    conn.execute_batch(&format!(
        "CREATE TRIGGER block_rumor_audience BEFORE UPDATE ON records
         WHEN NEW.id='{}' BEGIN SELECT RAISE(ABORT,'synthetic write fault'); END;",
        rumor_id
    ))
    .unwrap();

    assert!(library.commit_rumor_audience(&preview, ST).is_err());
    assert_eq!(library.revision().unwrap(), before_revision);
    let after_audit: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
        .unwrap();
    assert_eq!(after_audit, before_audit);
    assert!(
        library
            .rumor_audience_snapshot(&rumor_id, ST)
            .unwrap()
            .recipients
            .is_empty()
    );
}

#[test]
fn rumor_audience_persists_through_reopen_and_backup_restore() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("recovery.gvlib");
    let backup = dir.path().join("recovery.gvbackup");
    let mut library = Library::create(&path, "Recovery Chronicle").unwrap();
    let first = library.create_character("Vampire", "First", 0, ST).unwrap();
    let second = library
        .create_character("Werewolf", "Second", 1, ST)
        .unwrap();
    let rumor_id = make_rumor(&mut library, "Rumor", "2026-11-01", 2);
    let preview = selected_preview(&library, &rumor_id, std::slice::from_ref(&first));
    library.commit_rumor_audience(&preview, ST).unwrap();
    library.backup(&backup, ST).unwrap();

    let current = library.revision().unwrap();
    let both = selected_preview(&library, &rumor_id, &[first.clone(), second.clone()]);
    library.commit_rumor_audience(&both, ST).unwrap();
    drop(library);
    let mut library = Library::open(&path).unwrap();
    assert_eq!(
        library
            .rumor_audience_snapshot(&rumor_id, ST)
            .unwrap()
            .recipients
            .len(),
        2
    );
    library.restore(&backup, current + 1, ST).unwrap();
    let restored = library.rumor_audience_snapshot(&rumor_id, ST).unwrap();
    assert_eq!(restored.recipients.len(), 1);
    assert_eq!(restored.recipients[0].binding.character_id, first);
}

#[test]
fn date_scoped_queue_is_inclusive_sorted_and_omits_invalid_dates() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue.gvlib");
    let mut library = Library::create(&path, "Queue Chronicle").unwrap();
    let target = library
        .create_character("Vampire", "Queue Target", 0, ST)
        .unwrap();
    let mut revision = 1;
    let mut ids = BTreeMap::new();
    for (title, day) in [
        ("Start B", "2026-01-01"),
        ("Middle", "2026-01-02"),
        ("Start A", "2026-01-01"),
        ("Through", "2026-01-03"),
        ("Before", "2025-12-31"),
        ("After", "2026-01-04"),
        ("Undated", "2026-01-01"),
        ("Invalid", "2026-01-01"),
    ] {
        let id = make_rumor(&mut library, title, day, revision);
        revision += 1;
        ids.insert(title, id);
    }
    let action = Action {
        character: "Queue Target".into(),
        day: "2026-01-02".into(),
        ..Action::default()
    };
    library
        .create_story(&Draft::Action(action), revision, ST)
        .unwrap();
    let selected = selected_preview(&library, &ids["Start A"], std::slice::from_ref(&target));
    library.commit_rumor_audience(&selected, ST).unwrap();

    set_rumor_day(&path, &ids["Undated"], "");
    set_rumor_day(&path, &ids["Invalid"], "not-a-date");
    let queue = library
        .rumor_audience_queue("2026-01-01", "2026-01-03", ST)
        .unwrap();
    assert_eq!(queue.undated_omitted, 2);
    assert_eq!(queue.rows.len(), 4);
    assert_eq!(queue.rows[0].day, "2026-01-01");
    assert_eq!(queue.rows[1].day, "2026-01-01");
    assert!(queue.rows[0].rumor_id < queue.rows[1].rumor_id);
    assert_eq!(queue.rows[2].title, "Middle");
    assert_eq!(queue.rows[3].title, "Through");
    assert_eq!(
        queue
            .rows
            .iter()
            .find(|row| row.title == "Start A")
            .unwrap()
            .recipient_count,
        1
    );
    assert!(
        queue
            .rows
            .iter()
            .all(|row| row.title != "Before" && row.title != "After")
    );
    assert!(
        library
            .rumor_audience_queue("2026-01-03", "2026-01-01", ST)
            .is_err()
    );
    assert!(
        library
            .rumor_audience_queue("2026-01-01", "2026-01-03", PLAYER)
            .is_err()
    );
}

#[test]
fn date_scoped_queue_enforces_the_five_hundred_row_limit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("queue-limit.gvlib");
    let library = Library::create(&path, "Queue Limit Chronicle").unwrap();
    let conn = Connection::open(&path).unwrap();
    let tx = conn.unchecked_transaction().unwrap();
    for _ in 0..501 {
        let id = Uuid::new_v4().to_string();
        let mut private_fields = Map::new();
        private_fields.insert(
            "AppRumorV1".into(),
            json!({
                "title":"Synthetic queue row",
                "day":"2026-01-02",
                "category":0,
                "audience_label":"",
                "influence_list":"",
                "influence_trait":"",
                "reviewed":false,
                "entries":[]
            }),
        );
        let record = Record {
            id: id.clone(),
            kind: "Rumor".into(),
            name: "Synthetic queue row".into(),
            player_id: None,
            legacy_id: None,
            public_fields: Map::new(),
            private_fields,
            original: json!({"created_in_app":true}),
            source_sha256: String::new(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        };
        tx.execute(
            "INSERT INTO records(id,kind,name,data) VALUES(?1,'Rumor',?2,?3)",
            params![id, record.name, serde_json::to_string(&record).unwrap()],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        library
            .rumor_audience_queue("2026-01-01", "2026-01-03", ST)
            .unwrap_err()
            .code,
        "RUMOR_AUDIENCE_QUEUE_LIMIT"
    );
}
