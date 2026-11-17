use grapevine_workbench::{
    model::{PlayerLink, Role},
    player_points::*,
    store::Library,
};
use tempfile::TempDir;
#[test]
fn player_points_are_separate_private_atomic_and_recoverable() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("points.gvlib");
    let mut lib = Library::create(&path, "Synthetic points").unwrap();
    let owner = lib
        .create_player("Synthetic Player", 0, Role::Storyteller)
        .unwrap();
    let character = lib
        .create_character("Vampire", "Synthetic Character", 1, Role::Storyteller)
        .unwrap();
    let mut sheet = lib.sheet_edit(&character, Role::Storyteller).unwrap();
    sheet.player_link = PlayerLink::Assign(owner.clone());
    lib.save_sheet(&character, &sheet, 2, Role::Storyteller)
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let before: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&character], |r| {
            r.get(0)
        })
        .unwrap();
    let empty = dir.path().join("empty.gvbackup");
    lib.backup(&empty, Role::Storyteller).unwrap();
    let baseline = Baseline {
        day: "2026-01-01".into(),
        balance: "5.25".into(),
        actor: "SECRET_ACTOR".into(),
        reason: "SECRET_REASON".into(),
        evidence: "SECRET_EVIDENCE".into(),
    };
    let rev = lib.revision().unwrap();
    assert!(
        lib.initialize_player_points(&owner, &baseline, rev, Role::Player)
            .is_err()
    );
    assert!(lib.player_points(&owner, Role::Player).is_err());
    assert!(
        lib.initialize_player_points(&character, &baseline, rev, Role::Storyteller)
            .is_err()
    );
    lib.initialize_player_points(&owner, &baseline, rev, Role::Storyteller)
        .unwrap();
    let mut command = Command {
        id: uuid::Uuid::new_v4().to_string(),
        kind: Kind::Award,
        amount: "2".into(),
        day: "2026-01-02".into(),
        actor: "SECRET_ACTOR".into(),
        reason: "SECRET_REASON".into(),
        evidence: "SECRET_EVIDENCE".into(),
        reversal_of: None,
    };
    let rev = lib.revision().unwrap();
    assert!(
        lib.post_player_points(&owner, &command, rev, Role::Player)
            .is_err()
    );
    lib.post_player_points(&owner, &command, rev, Role::Storyteller)
        .unwrap();
    assert_eq!(
        lib.post_player_points(&owner, &command, rev, Role::Storyteller)
            .unwrap(),
        rev + 1
    );
    assert_eq!(
        replay(
            &lib.player_points(&owner, Role::Storyteller)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        "7.25"
    );
    let after: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&character], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(before, after);
    for role in [Role::Player, Role::Storyteller] {
        let view = serde_json::to_string(&lib.record(&owner, role).unwrap()).unwrap();
        assert!(!view.contains("SECRET_") && !view.contains("_AppPlayerPoints"));
    }
    let rev = lib.revision().unwrap();
    command.id = uuid::Uuid::new_v4().to_string();
    command.kind = Kind::Spend;
    command.amount = "100".into();
    assert!(
        lib.post_player_points(&owner, &command, rev, Role::Storyteller)
            .is_err()
    );
    assert_eq!(lib.revision().unwrap(), rev);
    let backup = dir.path().join("points.gvbackup");
    lib.backup(&backup, Role::Storyteller).unwrap();
    drop(lib);
    let mut lib = Library::open(&path).unwrap();
    assert_eq!(
        replay(
            &lib.player_points(&owner, Role::Storyteller)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        "7.25"
    );
    lib.restore(&empty, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert!(
        lib.player_points(&owner, Role::Storyteller)
            .unwrap()
            .is_none()
    );
    lib.restore(&backup, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(
        replay(
            &lib.player_points(&owner, Role::Storyteller)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        "7.25"
    );
    assert!(
        lib.create_player("Denied", lib.revision().unwrap(), Role::Player)
            .is_err()
    );
}
