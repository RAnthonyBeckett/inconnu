use grapevine_workbench::{model::Role, store::Library};
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn schema4_upgrade_is_backed_up_and_atomic_on_failure() {
    for inject_failure in [false, true] {
        let d = TempDir::new().unwrap();
        let path = d.path().join("synthetic.gvlib");
        let mut library = Library::create(&path, "Synthetic migration").unwrap();
        let id = library
            .create_character("Risen", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        drop(library);
        let c = Connection::open(&path).unwrap();
        c.execute_batch(
            "DROP TABLE game_sessions; DROP TABLE scheduled_sessions; PRAGMA user_version=4;",
        )
        .unwrap();
        let before: String = c
            .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
            .unwrap();
        if inject_failure {
            c.execute_batch("CREATE TRIGGER reject_session_migration BEFORE UPDATE ON library BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        }
        drop(c);
        let opened = Library::open(&path);
        assert_eq!(opened.is_err(), inject_failure);
        drop(opened);
        let c = Connection::open(&path).unwrap();
        let version: i64 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, if inject_failure { 4 } else { 9 });
        let exists: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='game_sessions')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, !inject_failure);
        let after: String = c
            .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(before, after);
        let backups: Vec<_> = std::fs::read_dir(d.path())
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
            4
        );
        assert_eq!(
            backup
                .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            before
        );
    }
}

#[test]
fn already_open_schema4_writer_cannot_write_after_upgrade() {
    let d = TempDir::new().unwrap();
    let path = d.path().join("old-writer.gvlib");
    let mut lib = Library::create(&path, "Synthetic old writer").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
        .unwrap();
    drop(lib);
    let mut old = Connection::open(&path).unwrap();
    old.execute_batch("DROP TRIGGER library_revision_guard_before_update; ALTER TABLE purchase_write_guard DROP COLUMN schema_revision; DROP TABLE game_sessions; DROP TABLE scheduled_sessions; PRAGMA user_version=4; CREATE TRIGGER library_revision_guard_before_update BEFORE UPDATE OF revision ON library WHEN NEW.revision!=COALESCE((SELECT next_revision FROM purchase_write_guard WHERE singleton=1),-1) BEGIN SELECT RAISE(ABORT,'old guard'); END;").unwrap();
    let mut upgraded = Library::open(&path).unwrap();
    let rev = upgraded.revision().unwrap();
    {
        let tx = old.transaction().unwrap();
        tx.execute(
            "UPDATE purchase_write_guard SET next_revision=?1 WHERE singleton=1",
            [rev + 1],
        )
        .unwrap();
        assert!(
            tx.execute(
                "UPDATE library SET revision=?1 WHERE revision=?2",
                [rev + 1, rev]
            )
            .is_err()
        );
    }
    assert_eq!(upgraded.revision().unwrap(), rev);
    upgraded
        .create_character("Risen", "Current writer works", rev, Role::Storyteller)
        .unwrap();
    assert_eq!(
        upgraded.record(&id, Role::Storyteller).unwrap().name,
        "Synthetic"
    );
}
