use grapevine_workbench::{model::Role, store::Library};
use rusqlite::{Connection, params};

#[test]
fn restore_rejects_mismatched_profile_atomically_and_older_backup_clears_new_metadata() {
    use grapevine_workbench::chronicle::Profile;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("restore.gvlib");
    let bad = dir.path().join("mismatched.gvbackup");
    let old = dir.path().join("old.gvbackup");
    let mut lib = Library::create(&path, "Original").unwrap();
    let id = lib
        .create_character("Vampire", "Retained", 0, Role::Storyteller)
        .unwrap();
    let mut profile = Profile {
        title: "Current".into(),
        private_notes: "PRIVATE_CURRENT".into(),
        ..Profile::default()
    };
    lib.save_chronicle_profile(&profile, 1, Role::Storyteller)
        .unwrap();
    lib.backup(&bad, Role::Storyteller).unwrap();
    let sql = Connection::open(&bad).unwrap();
    sql.execute(
        "UPDATE chronicle_profile SET data=json_set(data,'$.title','Mismatched')",
        [],
    )
    .unwrap();
    drop(sql);
    let bad_bytes = std::fs::read(&bad).unwrap();
    assert!(lib.restore(&bad, 2, Role::Storyteller).is_err());
    assert_eq!(lib.revision().unwrap(), 2);
    assert_eq!(
        lib.chronicle_profile(Role::Storyteller).unwrap().profile,
        profile
    );
    assert_eq!(lib.title, "Current");
    assert_eq!(lib.record(&id, Role::Storyteller).unwrap().name, "Retained");
    assert_eq!(std::fs::read(&bad).unwrap(), bad_bytes);
    lib.backup(&old, Role::Storyteller).unwrap();
    let sql = Connection::open(&old).unwrap();
    sql.execute_batch("DROP TABLE chronicle_profile; PRAGMA user_version=7;")
        .unwrap();
    drop(sql);
    let old_bytes = std::fs::read(&old).unwrap();
    profile.title = "Later".into();
    lib.save_chronicle_profile(&profile, 2, Role::Storyteller)
        .unwrap();
    assert_eq!(lib.restore(&old, 3, Role::Storyteller).unwrap(), 4);
    let restored = lib.chronicle_profile(Role::Storyteller).unwrap();
    assert_eq!(restored.profile.title, "Current");
    assert!(restored.profile.private_notes.is_empty());
    assert_eq!(std::fs::read(&old).unwrap(), old_bytes);
}

#[test]
fn current_schema_missing_profile_table_is_rejected_without_repair() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("incomplete.gvlib");
    drop(Library::create(&path, "Synthetic").unwrap());
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("DROP TABLE chronicle_profile").unwrap();
    assert!(Library::open(&path).is_err());
    assert_eq!(
        conn.query_row("SELECT revision FROM library", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn schema7_upgrade_preserves_records_and_backup_and_rejects_old_writer() {
    for fail in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schema7.gvlib");
        let mut lib = Library::create(&path, "Synthetic Chronicle").unwrap();
        let id = lib
            .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut old = Connection::open(&path).unwrap();
        old.execute_batch("DROP TABLE chronicle_profile; DROP TRIGGER library_revision_guard_before_update; PRAGMA user_version=7; UPDATE purchase_write_guard SET schema_revision=-next_revision; CREATE TRIGGER library_revision_guard_before_update BEFORE UPDATE OF revision ON library WHEN NEW.revision != COALESCE((SELECT next_revision FROM purchase_write_guard WHERE singleton=1),-1) OR -NEW.revision != COALESCE((SELECT schema_revision FROM purchase_write_guard WHERE singleton=1),1) BEGIN SELECT RAISE(ABORT,'schema7 writer'); END;").unwrap();
        let raw: String = old
            .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
            .unwrap();
        let revision: i64 = old
            .query_row("SELECT revision FROM library", [], |r| r.get(0))
            .unwrap();
        if fail {
            old.execute_batch("CREATE TRIGGER reject_upgrade BEFORE UPDATE ON library BEGIN SELECT RAISE(ABORT,'synthetic migration failure'); END;").unwrap();
        }
        let upgraded = Library::open(&path);
        assert_eq!(upgraded.is_err(), fail);
        let version: i64 = old
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, if fail { 7 } else { 9 });
        assert_eq!(
            old.query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r
                .get::<_, String>(0))
                .unwrap(),
            raw
        );
        let exists: bool = old
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='chronicle_profile')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, !fail);
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("pre-schema9-"))
            .collect();
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(backups[0].path()).unwrap();
        assert_eq!(
            backup
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            7
        );
        assert_eq!(
            backup
                .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            raw
        );
        if let Ok(mut upgraded) = upgraded {
            assert_eq!(upgraded.revision().unwrap(), revision + 2);
            // An already-open schema7 process still supplies the inverse token.
            let tx = old.transaction().unwrap();
            let next = revision + 2;
            tx.execute(
                "UPDATE purchase_write_guard SET next_revision=?1,schema_revision=?2",
                params![next, -next],
            )
            .unwrap();
            tx.execute(
                "UPDATE records SET name='must roll back' WHERE id=?1",
                [&id],
            )
            .unwrap();
            assert!(
                tx.execute("UPDATE library SET revision=?1", [next])
                    .is_err()
            );
            drop(tx);
            assert_eq!(
                upgraded.record(&id, Role::Storyteller).unwrap().name,
                "Synthetic"
            );
            upgraded
                .create_character("Risen", "Current writer", revision + 2, Role::Storyteller)
                .unwrap();
        } else {
            assert_eq!(
                old.query_row("SELECT revision FROM library", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                revision
            );
        }
    }
}
