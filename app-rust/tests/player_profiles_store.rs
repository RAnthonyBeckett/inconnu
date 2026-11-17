use grapevine_workbench::{
    model::{PlayerLink, Role},
    player_profiles::Profile,
    store::Library,
};
use serde_json::{Value, json};
const ST: Role = Role::Storyteller;
fn profile() -> Profile {
    Profile {
        name: "  Player Café <literal>  ".into(),
        position: "Volunteer".into(),
        status: "Active".into(),
        email: "PRIVATE_EMAIL".into(),
        phone: "PRIVATE_PHONE".into(),
        address: "PRIVATE_ADDRESS".into(),
        notes: "PRIVATE_NOTES".into(),
    }
}
#[test]
fn player_profile_preserves_identity_legacy_data_links_and_points_and_keeps_contacts_private() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.gvlib");
    let backup = dir.path().join("profile.backup");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    let p = profile();
    assert!(lib.create_player_profile(&p, 0, Role::Player).is_err());
    let id = lib.create_player_profile(&p, 0, ST).unwrap();
    let character = lib
        .create_character("Vampire", "Linked character", 1, ST)
        .unwrap();
    let mut sheet = lib.sheet_edit(&character, ST).unwrap();
    sheet.player_link = PlayerLink::Assign(id.clone());
    lib.save_sheet(&character, &sheet, 2, ST).unwrap();
    lib.initialize_player_points(
        &id,
        &grapevine_workbench::player_points::Baseline {
            day: "2026-01-01".into(),
            balance: "7".into(),
            actor: "Recorder".into(),
            reason: "Opening".into(),
            evidence: "Reference".into(),
        },
        3,
        ST,
    )
    .unwrap();
    let sql = rusqlite::Connection::open(&path).unwrap();
    let raw: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut raw: Value = serde_json::from_str(&raw).unwrap();
    raw["legacy_id"] = json!("LEGACY_ID");
    raw["source_sha256"] = json!("ORIGINAL_HASH");
    raw["original"] = json!({"raw_xml":"ORIGINAL_PAYLOAD"});
    raw["private_fields"]["UnknownField"] = json!({"keep":true});
    sql.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![raw.to_string(), id],
    )
    .unwrap();
    let snapshot = lib.player_profile(&id, ST).unwrap();
    assert_eq!(snapshot.profile.name, "Player Café <literal>");
    assert_eq!(
        snapshot.linked_characters,
        [(character.clone(), "Linked character".into())]
    );
    assert_eq!(snapshot.legacy_id.as_deref(), Some("LEGACY_ID"));
    assert!(lib.player_profile(&id, Role::Player).is_err());
    let public = serde_json::to_string(&lib.record(&id, Role::Player).unwrap()).unwrap();
    assert!(!public.contains("PRIVATE_"));
    lib.backup(&backup, ST).unwrap();
    let mut updated = snapshot.profile;
    updated.name = "Renamed Player".into();
    updated.email = "PRIVATE_NEW_EMAIL".into();
    updated.status = "Inactive".into();
    assert!(lib.save_player_profile(&id, &updated, 3, ST).is_err());
    assert!(
        lib.save_player_profile(&character, &updated, 4, ST)
            .is_err()
    );
    lib.save_player_profile(&id, &updated, 4, ST).unwrap();
    let saved: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let saved: Value = serde_json::from_str(&saved).unwrap();
    for field in ["id", "legacy_id", "source_sha256", "original"] {
        assert_eq!(saved[field], raw[field]);
    }
    assert_eq!(
        saved["private_fields"]["_AppPlayerPointsV1"],
        raw["private_fields"]["_AppPlayerPointsV1"]
    );
    assert_eq!(
        saved["private_fields"]["UnknownField"],
        raw["private_fields"]["UnknownField"]
    );
    assert_eq!(
        lib.record(&character, ST).unwrap().player_name,
        "Renamed Player"
    );
    let invalid = Profile {
        name: " ".into(),
        ..updated.clone()
    };
    assert!(lib.save_player_profile(&id, &invalid, 5, ST).is_err());
    assert_eq!(lib.revision().unwrap(), 5);
    drop(sql);
    drop(lib);
    let mut lib = Library::open(&path).unwrap();
    assert_eq!(lib.player_profile(&id, ST).unwrap().profile, updated);
    lib.restore(&backup, 5, ST).unwrap();
    assert_eq!(
        lib.player_profile(&id, ST).unwrap().profile.email,
        "PRIVATE_EMAIL"
    );
}
