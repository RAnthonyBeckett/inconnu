use grapevine_workbench::{
    model::*, printing::*, purchase_preview::AnimalismPower, store::Library,
};
use serde_json::{Value, json};
use tempfile::TempDir;

fn fixture() -> (TempDir, Library, String) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("source.gex");
    std::fs::write(&path, r#"<grapevine version="3" stcommentstart="{ST}" stcommentend="{/ST}"><player name="Owner" id="p1" email="SECRET_CONTACT"/><vampire name="Érica{ST}SECRET_NAME{/ST}" status="Active" player="Owner"><traitlist name="Disciplines"><trait name="Feral Whispers" val="Basic" note="SECRET_TRAIT"/><trait name="Heightened Senses{ST}SECRET_POWER{/ST}" val="Basic"/></traitlist></vampire></grapevine>"#).unwrap();
    let mut lib = Library::create(&dir.path().join("sheet.gvlib"), "Synthetic").unwrap();
    let preview = lib
        .preview_import(&path, "windows-1252", Role::Storyteller)
        .unwrap();
    let id = preview
        .records
        .iter()
        .find(|r| r.kind == "Vampire")
        .unwrap()
        .id
        .clone();
    lib.commit_import(&preview, 0, false, Role::Storyteller)
        .unwrap();
    let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    edit.lists.get_mut("DisciplineList").unwrap()["Traits"][0]["AppSourcePowerId"] =
        json!(AnimalismPower::FeralWhispers.stable_id());
    edit.resources.insert("Willpower".into(), 7);
    edit.resources.insert("TempWillpower".into(), 2);
    lib.save_character(
        &id,
        &Edit {
            name: "Érica{ST}SECRET_NAME{/ST}".into(),
            status: "Active".into(),
            notes: "SECRET_NOTES".into(),
        },
        &edit,
        1,
        Role::Storyteller,
    )
    .unwrap();
    (dir, lib, id)
}
fn raw(lib: &Library, id: &str) -> String {
    rusqlite::Connection::open(&lib.path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .unwrap()
}
#[test]
fn scope_notes_sources_and_read_only_snapshot_survive_backup() {
    let (dir, lib, id) = fixture();
    let before = raw(&lib, &id);
    let revision = lib.revision().unwrap();
    let public = lib
        .print_document(&id, &Options::default(), revision, Role::Player)
        .unwrap();
    let serialized = serde_json::to_string(&public).unwrap();
    assert!(!serialized.contains("SECRET"));
    assert_eq!(public.references.len(), 1);
    assert_eq!(public.references[0].power, "Feral Whispers");
    let opts = Options {
        scope: PrintScope::PrivateStoryteller,
        language: DocLanguage::Portuguese,
        ..Options::default()
    };
    assert!(
        lib.print_document(&id, &opts, revision, Role::Player)
            .is_err()
    );
    let private = lib
        .print_document(&id, &opts, revision, Role::Storyteller)
        .unwrap();
    let text = serde_json::to_string(&private).unwrap();
    assert!(!text.contains("SECRET_NOTES"));
    assert!(!text.contains("SECRET_TRAIT"));
    assert!(!text.contains("SECRET_CONTACT"));
    assert!(
        private
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .any(|r| r.value == "7")
    );
    let with_notes = lib
        .print_document(
            &id,
            &Options {
                include_notes: true,
                ..opts.clone()
            },
            revision,
            Role::Storyteller,
        )
        .unwrap();
    let text = serde_json::to_string(&with_notes).unwrap();
    assert!(text.contains("SECRET_NOTES"));
    assert!(text.contains("SECRET_TRAIT"));
    let without_temp = lib
        .print_document(
            &id,
            &Options {
                include_temporary: false,
                ..opts
            },
            revision,
            Role::Storyteller,
        )
        .unwrap();
    assert!(
        without_temp
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .all(|r| r.value != "2")
    );
    assert_eq!(raw(&lib, &id), before);
    assert_eq!(lib.revision().unwrap(), revision);
    let backup = dir.path().join("backup.gvlib");
    lib.backup(&backup, Role::Storyteller).unwrap();
    let reopened = Library::open(&backup).unwrap();
    assert_eq!(
        serde_json::to_value(public).unwrap(),
        serde_json::to_value(
            reopened
                .print_document(&id, &Options::default(), revision, Role::Player)
                .unwrap()
        )
        .unwrap()
    );
    assert!(
        lib.print_document(&id, &Options::default(), revision - 1, Role::Storyteller)
            .is_err()
    );
}
#[test]
fn builds_require_st_and_source_id_cannot_bless_another_name_or_level() {
    let (_dir, mut lib, id) = fixture();
    let rev = lib.revision().unwrap();
    let build = lib
        .create_build(&id, BuildKind::Test, rev, Role::Storyteller)
        .unwrap();
    let rev = lib.revision().unwrap();
    assert!(
        lib.print_document(&build, &Options::default(), rev, Role::Player)
            .is_err()
    );
    let doc = lib
        .print_document(&build, &Options::default(), rev, Role::Storyteller)
        .unwrap();
    assert_eq!(doc.build, Some(BuildKind::Test));
    assert!(html(&doc).unwrap().contains("TEST COPY"));
    let mut record: Value = serde_json::from_str(&raw(&lib, &id)).unwrap();
    let rows = &mut record["private_fields"]["DisciplineList"]["Traits"];
    rows[0]["AppSourcePowerId"] = json!(AnimalismPower::HeightenedSenses.stable_id());
    rows[1]["AppSourcePowerId"] = json!(AnimalismPower::HeightenedSenses.stable_id());
    rows[1]["Value"] = json!("Intermediate");
    rusqlite::Connection::open(&lib.path)
        .unwrap()
        .execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![record.to_string(), id],
        )
        .unwrap();
    let doc = lib
        .print_document(&id, &Options::default(), rev, Role::Player)
        .unwrap();
    assert!(doc.references.is_empty());
}

#[test]
fn linked_player_name_applies_its_own_custom_and_default_secret_markers() {
    let (_dir, lib, id) = fixture();
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    let data: String = conn
        .query_row("SELECT data FROM records WHERE kind='Player'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let mut player: Value = serde_json::from_str(&data).unwrap();
    player["public_fields"]["Name"] = json!("Owner{{SECRET_LINKED}}[ST]SECRET_DEFAULT[/ST]");
    player["st_markers"] = json!(["{{", "}}"]);
    conn.execute(
        "UPDATE records SET data=?1 WHERE kind='Player'",
        [player.to_string()],
    )
    .unwrap();
    let doc = lib
        .print_document(
            &id,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap();
    assert_eq!(doc.player_name, "Owner");
    assert!(!html(&doc).unwrap().contains("SECRET"));
}
