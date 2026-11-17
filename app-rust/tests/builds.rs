use grapevine_workbench::{model::*, sheet, store::Library};
use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;

fn sample(dir: &Path) -> std::path::PathBuf {
    let p = dir.join("mixed.gex");
    fs::write(&p,br#"<grapevine version="3" stcommentstart="{ST}" stcommentend="{/ST}"><player name="Owner" id="p1"/><wraith name="Source" status="Approved" player="Owner" unknown="RETAIN"><traitlist name="Arcanoi"><trait name="Existing" val="Descriptive" note="PRIVATE"/></traitlist></wraith></grapevine>"#).unwrap();
    p
}
fn imported(dir: &Path) -> (Library, String) {
    let mut lib = Library::create(&dir.join("library.gvlib"), "Build tests").unwrap();
    let p = lib
        .preview_import(&sample(dir), "windows-1252", Role::Storyteller)
        .unwrap();
    let id = p
        .records
        .iter()
        .find(|r| r.kind == "Wraith")
        .unwrap()
        .id
        .clone();
    lib.commit_import(&p, 0, false, Role::Storyteller).unwrap();
    (lib, id)
}
fn raw(lib: &Library, id: &str) -> String {
    rusqlite::Connection::open(&lib.path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .unwrap()
}
#[test]
fn mixed_power_edits_preserve_metadata_and_originals_and_redact_custom_secrets() {
    let d = TempDir::new().unwrap();
    let (mut lib, id) = imported(d.path());
    let original: Value =
        serde_json::from_str::<Value>(&raw(&lib, &id)).unwrap()["original"].clone();
    let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    let list = edit.lists.get_mut("ArcanoiList").unwrap();
    list["extension"] = json!({"retain":true,"secret":"NEVER_SHARE"});
    list["Traits"][0]["Value"] = json!("Descriptive level");
    list["Traits"][0]["extension"] = json!({"untouched":7});
    let mut discipline = sheet::empty_list("DisciplineList");
    discipline["Traits"] =
        json!([{"Name":"Power{ST}HIDDEN{/ST}","Value":"Basic","Note":"PRIVATE"}]);
    edit.lists.insert("DisciplineList".into(), discipline);
    edit.resources.insert("Pathos".into(), 10);
    edit.resources.insert("TempPathos".into(), 3);
    lib.save_sheet(&id, &edit, 1, Role::Storyteller).unwrap();
    let got = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    assert_eq!(got.lists, edit.lists);
    assert_eq!(got.resources, edit.resources);
    let r: Value = serde_json::from_str(&raw(&lib, &id)).unwrap();
    assert_eq!(r["original"], original);
    assert_eq!(r["private_fields"]["unknown"], "RETAIN");
    let public = serde_json::to_string(&lib.record(&id, Role::Player).unwrap()).unwrap();
    assert!(!public.contains("HIDDEN"));
    assert!(!public.contains("PRIVATE"));
    assert!(!public.contains("NEVER_SHARE"));
    let mut second = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    second.lists.get_mut("ArcanoiList").unwrap()["Traits"] = json!([]);
    lib.save_sheet(&id, &second, 2, Role::Storyteller).unwrap();
    assert!(lib.sheet_edit(&id, Role::Player).is_err());
    assert!(lib.save_sheet(&id, &second, 3, Role::Player).is_err());
}
#[test]
fn draft_and_test_edits_never_change_source_xp_or_expose_copies_to_player() {
    let d = TempDir::new().unwrap();
    let (mut lib, id) = imported(d.path());
    let source = raw(&lib, &id);
    let mut revision = 1;
    for kind in [BuildKind::Draft, BuildKind::Test] {
        let build = lib
            .create_build(&id, kind, revision, Role::Storyteller)
            .unwrap();
        revision += 1;
        let mut edit = lib.sheet_edit(&build, Role::Storyteller).unwrap();
        edit.resources.insert("Willpower".into(), 7);
        let mut list = sheet::empty_list("DisciplineList");
        list["Traits"] = json!([{"Name":"Hypothetical","Value":"Elder","Note":"Private"}]);
        edit.lists.insert("DisciplineList".into(), list);
        let basic = Edit {
            name: "Private build".into(),
            status: "Not approved".into(),
            notes: "Test only".into(),
        };
        lib.save_character(&build, &basic, &edit, revision, Role::Storyteller)
            .unwrap();
        revision += 1;
        assert_eq!(raw(&lib, &id), source);
        assert!(lib.record(&build, Role::Player).is_err());
        assert!(
            !lib.summaries(Role::Player)
                .unwrap()
                .iter()
                .any(|r| r.id == build)
        );
        assert!(
            lib.create_build(&build, kind, revision, Role::Storyteller)
                .is_err()
        );
        assert_eq!(
            lib.record(&build, Role::Storyteller).unwrap().build,
            Some(kind)
        );
    }
    assert!(
        lib.create_build(&id, BuildKind::Test, revision, Role::Player)
            .is_err()
    );
    let source_file = sample(d.path());
    assert!(
        lib.preview_import(&source_file, "windows-1252", Role::Storyteller)
            .is_err()
    );
    let db = lib.path.clone();
    drop(lib);
    let lib = Library::open(&db).unwrap();
    assert_eq!(raw(&lib, &id), source);
}
#[test]
fn atomic_basic_and_sheet_save_rejects_stale_invalid_and_injected_failure() {
    let d = TempDir::new().unwrap();
    let (mut lib, id) = imported(d.path());
    let before = raw(&lib, &id);
    let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    edit.resources.insert("Willpower".into(), 5);
    let basic = Edit {
        name: "Changed".into(),
        status: "Draft".into(),
        notes: "Draft".into(),
    };
    assert!(
        lib.save_character(&id, &basic, &edit, 0, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib, &id), before);
    edit.resources.insert("Unknown".into(), 9);
    assert!(
        lib.save_character(&id, &basic, &edit, 1, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib, &id), before);
    edit.resources.remove("Unknown");
    let c = rusqlite::Connection::open(&lib.path).unwrap();
    c.execute_batch("CREATE TRIGGER fail_sheet BEFORE UPDATE ON records BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(
        lib.save_character(&id, &basic, &edit, 1, Role::Storyteller)
            .is_err()
    );
    assert_eq!(lib.revision().unwrap(), 1);
    assert_eq!(raw(&lib, &id), before);
    c.execute_batch("DROP TRIGGER fail_sheet").unwrap();
    lib.save_character(&id, &basic, &edit, 1, Role::Storyteller)
        .unwrap();
    assert_eq!(lib.record(&id, Role::Storyteller).unwrap().name, "Changed");
}
#[test]
fn explicit_player_link_is_validated_and_original_link_evidence_retained() {
    let d = TempDir::new().unwrap();
    let (mut lib, id) = imported(d.path());
    let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    edit.player_link = PlayerLink::Assign(id.clone());
    assert!(lib.save_sheet(&id, &edit, 1, Role::Storyteller).is_err());
    edit.player_link = PlayerLink::Clear;
    lib.save_sheet(&id, &edit, 1, Role::Storyteller).unwrap();
    assert!(
        lib.record(&id, Role::Storyteller)
            .unwrap()
            .player_name
            .is_empty()
    );
    let player = lib
        .summaries(Role::Storyteller)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == "Player")
        .unwrap();
    edit.player_link = PlayerLink::Assign(player.id);
    lib.save_sheet(&id, &edit, 2, Role::Storyteller).unwrap();
    assert_eq!(lib.record(&id, Role::Player).unwrap().player_name, "Owner");
}
#[test]
fn schema1_migration_backs_up_and_advances_revision_and_failed_migration_rolls_back() {
    let d = TempDir::new().unwrap();
    let (lib, id) = imported(d.path());
    let db = lib.path.clone();
    let before = raw(&lib, &id);
    drop(lib);
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch("DROP TRIGGER library_revision_guard_before_update; DROP TABLE purchase_write_guard; DROP TABLE purchase_receipts; DROP TABLE game_sessions; DROP TABLE scheduled_sessions;").unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    drop(c);
    let lib = Library::open(&db).unwrap();
    assert_eq!(lib.revision().unwrap(), 8);
    assert_eq!(raw(&lib, &id), before);
    let backups = fs::read_dir(d.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "gvbackup"))
        .collect::<Vec<_>>();
    assert_eq!(backups.len(), 1);
    let old = rusqlite::Connection::open(&backups[0]).unwrap();
    assert_eq!(
        old.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch("DROP TRIGGER library_revision_guard_before_update; DROP TABLE purchase_write_guard; DROP TABLE purchase_receipts; DROP TABLE game_sessions; DROP TABLE scheduled_sessions;").unwrap();
    c.pragma_update(None, "user_version", 1).unwrap();
    c.execute_batch("CREATE TRIGGER fail_migration BEFORE UPDATE ON library BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    drop(c);
    drop(lib);
    assert!(Library::open(&db).is_err());
    let c = rusqlite::Connection::open(db).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn all_twelve_original_types_and_risen_support_isolated_mixed_power_builds() {
    let d = TempDir::new().unwrap();
    let mut lib = Library::create(&d.path().join("types.gvlib"), "All types").unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-game.gv3");
    let p = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    lib.commit_import(&p, 0, false, Role::Storyteller).unwrap();
    lib.create_character("Risen", "Synthetic Risen", 1, Role::Storyteller)
        .unwrap();
    let originals = lib
        .summaries(Role::Storyteller)
        .unwrap()
        .into_iter()
        .filter(|r| sheet::is_character_kind(&r.kind))
        .collect::<Vec<_>>();
    assert_eq!(originals.len(), 13);
    let mut revision = 2;
    for source in originals {
        let before = raw(&lib, &source.id);
        let id = lib
            .create_build(&source.id, BuildKind::Test, revision, Role::Storyteller)
            .unwrap();
        revision += 1;
        let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
        for key in ["ArcanoiList", "DisciplineList"] {
            let mut list = sheet::empty_list(key);
            list["Traits"] = json!([{"Name":"Mixed","Value":"Descriptive","Note":""}]);
            edit.lists.insert(key.into(), list);
        }
        lib.save_sheet(&id, &edit, revision, Role::Storyteller)
            .unwrap();
        revision += 1;
        assert_eq!(raw(&lib, &source.id), before);
    }
}

#[test]
fn xml_numeric_resources_are_editable_but_descriptive_values_are_retained() {
    let d = TempDir::new().unwrap();
    let source = d.path().join("resources.gex");
    fs::write(&source,br#"<grapevine version="3"><wraith name="Resource test" willpower="7" tempwillpower="3" pathos="Review needed"/></grapevine>"#).unwrap();
    let mut lib = Library::create(&d.path().join("resources.gvlib"), "Resources").unwrap();
    let p = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    let id = p.records[0].id.clone();
    lib.commit_import(&p, 0, false, Role::Storyteller).unwrap();
    let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    assert_eq!(edit.resources["Willpower"], 7);
    assert_eq!(edit.resources["TempWillpower"], 3);
    edit.resources.insert("Pathos".into(), 5);
    assert!(lib.save_sheet(&id, &edit, 1, Role::Storyteller).is_err());
    assert_eq!(
        lib.record(&id, Role::Storyteller).unwrap().fields["Pathos"],
        "Review needed"
    );
}
