use grapevine_workbench::{
    model::{BuildKind, Role},
    searches::{self, Clause, Comparison, Field, Mode, Query},
    store::Library,
};
use std::{fs, path::Path};
const ST: Role = Role::Storyteller;
fn query(field: Field, value: &str) -> Query {
    Query {
        mode: Mode::All,
        clauses: vec![Clause {
            field,
            comparison: Comparison::Equal,
            negate: false,
            value: value.into(),
            trait_name: String::new(),
            number: 0,
        }],
    }
}
fn imported(dir: &Path) -> Library {
    let source = dir.join("synthetic.gex");
    fs::write(&source,br#"<grapevine version="3"><player name="PRIVATE_PLAYER"/><vampire name="One" player="PRIVATE_PLAYER" status="Active" group="Group A" npc="no"><notes>PRIVATE_NOTES</notes><traitlist name="Abilities"><trait name="Investigation" val="3" note="PRIVATE_TRAIT_NOTE"/></traitlist></vampire><wraith name="Two" status="Active" group="Group B" npc="yes"/><mortal name="Three" status="Inactive"/></grapevine>"#).unwrap();
    let mut lib = Library::create(&dir.join("search.gvlib"), "Synthetic").unwrap();
    let preview = lib.preview_import(&source, "windows-1252", ST).unwrap();
    lib.commit_import(&preview, 0, false, ST).unwrap();
    lib
}
#[test]
fn saved_search_crud_filters_and_population_reports_preserve_data_and_privacy() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = imported(dir.path());
    let before_source = fs::read(dir.path().join("synthetic.gex")).unwrap();
    let character = lib
        .summaries(ST)
        .unwrap()
        .into_iter()
        .find(|r| r.name == "One")
        .unwrap()
        .id;
    lib.create_build(&character, BuildKind::Draft, 1, ST)
        .unwrap();
    let all = Query {
        mode: Mode::All,
        clauses: Vec::new(),
    };
    let snap = lib.search_snapshot(&all, false, ST).unwrap();
    assert_eq!(snap.rows.len(), 3);
    assert!(snap.rows.iter().all(|r| r.player.is_empty()));
    assert_eq!(lib.revision().unwrap(), 2);
    let id = lib
        .save_search(None, " Active ", &query(Field::Status, "Active"), 2, ST)
        .unwrap();
    let entries = lib.saved_searches(ST).unwrap();
    assert_eq!(entries.revision, 3);
    assert_eq!(entries.entries[0].name, "Active");
    assert!(lib.save_search(None, "active", &all, 3, ST).is_err());
    assert!(
        lib.save_search(Some("missing"), "Missing", &all, 3, ST)
            .is_err()
    );
    assert!(lib.save_search(Some(&id), "Renamed", &all, 2, ST).is_err());
    assert!(lib.delete_search(&id, 2, ST).is_err());
    assert_eq!(lib.revision().unwrap(), 3);
    let active = lib
        .search_snapshot(&entries.entries[0].query, false, ST)
        .unwrap();
    assert_eq!(active.rows.len(), 2);
    let by_player = lib
        .search_snapshot(&query(Field::Player, "PRIVATE_PLAYER"), false, ST)
        .unwrap();
    assert_eq!(by_player.rows.len(), 1);
    assert!(
        !serde_json::to_string(&by_player)
            .unwrap()
            .contains("PRIVATE_PLAYER")
    );
    for statistics in [false, true] {
        for language in [
            grapevine_workbench::model::Language::English,
            grapevine_workbench::model::Language::Portuguese,
        ] {
            let html = searches::html(&by_player, language, statistics).unwrap();
            for private in [
                "PRIVATE_PLAYER",
                "PRIVATE_NOTES",
                "PRIVATE_TRAIT_NOTE",
                &character,
            ] {
                assert!(!html.contains(private), "{private}");
            }
        }
    }
    let included = lib
        .search_snapshot(&query(Field::Player, "PRIVATE_PLAYER"), true, ST)
        .unwrap();
    assert_eq!(included.rows[0].player, "PRIVATE_PLAYER");
    let mut trait_query = query(Field::Trait, "AbilityList");
    trait_query.clauses[0].trait_name = "Investigation".into();
    trait_query.clauses[0].number = 3;
    assert_eq!(
        lib.search_snapshot(&trait_query, false, ST)
            .unwrap()
            .rows
            .len(),
        1
    );
    trait_query.clauses[0].negate = true;
    assert_eq!(
        lib.search_snapshot(&trait_query, false, ST)
            .unwrap()
            .rows
            .len(),
        0
    );
    for operation in [
        lib.saved_searches(Role::Player).map(|_| ()),
        lib.search_snapshot(&all, false, Role::Player).map(|_| ()),
        lib.save_search(None, "Denied", &all, 3, Role::Player)
            .map(|_| ()),
        lib.delete_search(&id, 3, Role::Player).map(|_| ()),
    ] {
        assert!(operation.is_err());
    }
    lib.save_search(Some(&id), "All characters", &all, 3, ST)
        .unwrap();
    let backup = dir.path().join("search.backup");
    lib.backup(&backup, ST).unwrap();
    lib.delete_search(&id, 4, ST).unwrap();
    assert!(lib.saved_searches(ST).unwrap().entries.is_empty());
    assert!(lib.delete_search(&id, 5, ST).is_err());
    assert_eq!(lib.revision().unwrap(), 5);
    lib.restore(&backup, 5, ST).unwrap();
    assert_eq!(lib.saved_searches(ST).unwrap().entries[0].id, id);
    assert_eq!(lib.search_snapshot(&all, false, ST).unwrap().rows.len(), 3);
    assert_eq!(
        fs::read(dir.path().join("synthetic.gex")).unwrap(),
        before_source
    );
}
#[test]
fn schema6_upgrade_is_atomic_backed_up_and_rejects_an_already_open_paired_writer() {
    for fail in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let lib = imported(dir.path());
        let path = lib.path.clone();
        drop(lib);
        let mut old = rusqlite::Connection::open(&path).unwrap();
        old.execute_batch("DROP TABLE saved_searches; DROP TRIGGER library_revision_guard_before_update; PRAGMA user_version=6; UPDATE purchase_write_guard SET schema_revision=next_revision; CREATE TRIGGER library_revision_guard_before_update BEFORE UPDATE OF revision ON library WHEN NEW.revision != COALESCE((SELECT next_revision FROM purchase_write_guard WHERE singleton=1),-1) OR NEW.revision != COALESCE((SELECT schema_revision FROM purchase_write_guard WHERE singleton=1),-1) BEGIN SELECT RAISE(ABORT,'old paired writer'); END;").unwrap();
        let raw: String = old
            .query_row(
                "SELECT group_concat(data,'|') FROM records ORDER BY id",
                [],
                |r| r.get(0),
            )
            .unwrap();
        if fail {
            old.execute_batch("CREATE TRIGGER reject_upgrade BEFORE UPDATE ON library BEGIN SELECT RAISE(ABORT,'synthetic migration failure'); END;").unwrap();
        }
        let result = Library::open(&path);
        assert_eq!(result.is_err(), fail);
        assert_eq!(
            old.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            if fail { 6 } else { 9 }
        );
        assert_eq!(
            old.query_row(
                "SELECT group_concat(data,'|') FROM records ORDER BY id",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            raw
        );
        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("pre-schema9-"))
            .collect();
        assert_eq!(backups.len(), 1);
        let backup = rusqlite::Connection::open(backups[0].path()).unwrap();
        assert_eq!(
            backup
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            6
        );
        if let Ok(mut upgraded) = result {
            let revision = upgraded.revision().unwrap();
            let tx = old.transaction().unwrap();
            tx.execute(
                "UPDATE purchase_write_guard SET next_revision=?1,schema_revision=?1",
                [revision + 1],
            )
            .unwrap();
            assert!(
                tx.execute("UPDATE library SET revision=?1", [revision + 1])
                    .is_err()
            );
            drop(tx);
            upgraded
                .save_search(
                    None,
                    "Proof",
                    &Query {
                        mode: Mode::All,
                        clauses: Vec::new(),
                    },
                    revision,
                    ST,
                )
                .unwrap();
        } else {
            assert!(
                !old.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='saved_searches')",
                    [],
                    |r| r.get::<_, bool>(0)
                )
                .unwrap()
            );
        }
    }
}

#[test]
fn older_unmapped_xml_lists_are_read_without_writes_and_explicit_fields_win() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = imported(dir.path());
    let id = lib
        .summaries(ST)
        .unwrap()
        .into_iter()
        .find(|r| r.name == "One")
        .unwrap()
        .id;
    let sql = rusqlite::Connection::open(&lib.path).unwrap();
    let encoded: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut record: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let list = record["private_fields"]["UnmappedTraitLists"].clone();
    record["private_fields"]["UnmappedTraitLists"] = list.clone();
    sql.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    let mut criteria = query(Field::Trait, "AbilityList");
    criteria.clauses[0].trait_name = "Investigation".into();
    criteria.clauses[0].number = 3;
    assert_eq!(
        lib.search_snapshot(&criteria, false, ST)
            .unwrap()
            .rows
            .len(),
        1
    );
    assert_eq!(lib.revision().unwrap(), 1);
    let after: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    assert_eq!(after, record.to_string());
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.lists.get_mut("AbilityList").unwrap()["Traits"] = serde_json::json!([]);
    lib.save_sheet(&id, &edit, 1, ST).unwrap();
    assert!(
        lib.search_snapshot(&criteria, false, ST)
            .unwrap()
            .rows
            .is_empty()
    );
    record["private_fields"]["UnmappedTraitLists"] = serde_json::json!([list.clone(), list]);
    record["private_fields"]
        .as_object_mut()
        .unwrap()
        .remove("AbilityList");
    record["public_fields"]
        .as_object_mut()
        .unwrap()
        .remove("AbilityList");
    sql.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    assert!(
        lib.search_snapshot(&criteria, false, ST)
            .unwrap()
            .rows
            .is_empty()
    );
}

#[test]
fn invalid_search_backup_restoration_rolls_back_all_current_data() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = imported(dir.path());
    let all = Query {
        mode: Mode::All,
        clauses: Vec::new(),
    };
    let id = lib.save_search(None, "Original", &all, 1, ST).unwrap();
    let backup = dir.path().join("corrupt-search.backup");
    lib.backup(&backup, ST).unwrap();
    let sql = rusqlite::Connection::open(&backup).unwrap();
    sql.execute("UPDATE saved_searches SET data='{}'", [])
        .unwrap();
    drop(sql);
    lib.save_search(Some(&id), "Current", &all, 2, ST).unwrap();
    let before = lib.saved_searches(ST).unwrap().entries;
    assert!(lib.restore(&backup, 3, ST).is_err());
    assert_eq!(lib.revision().unwrap(), 3);
    assert_eq!(lib.saved_searches(ST).unwrap().entries, before);
    assert_eq!(lib.search_snapshot(&all, false, ST).unwrap().rows.len(), 3);
}
