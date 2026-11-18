use grapevine_workbench::{
    actions::Action,
    chronicle::Profile as ChronicleProfile,
    game_sessions::{Attendance, SessionRequest},
    model::{BuildKind, Role},
    player_profiles::Profile as PlayerProfile,
    plots::Plot,
    rumors::Rumor,
    store::Library,
    story_work::Draft,
    world::Edit as WorldEdit,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tempfile::TempDir;
use uuid::Uuid;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

fn chronicle_profile(title: &str) -> ChronicleProfile {
    ChronicleProfile {
        title: title.into(),
        website: "https://example.invalid/chronicle".into(),
        email: "PRIVATE_CHRONICLE_EMAIL".into(),
        phone: "PRIVATE_CHRONICLE_PHONE".into(),
        meeting_place: "PRIVATE_MEETING_PLACE".into(),
        meeting_time: "Thursday evening".into(),
        description: "A synthetic local game.".into(),
        house_rules: "PRIVATE_HOUSE_RULES".into(),
        private_notes: "PRIVATE_ST_HANDOVER_NOTES".into(),
        publish_details: false,
        publish_contacts: false,
        owbn_bylaws_enabled: false,
        allow_non_binding_sources: false,
    }
}

fn count(handover: &grapevine_workbench::chronicle::Handover, key: &str) -> u64 {
    handover
        .counts
        .iter()
        .find(|count| count.key == key)
        .unwrap_or_else(|| panic!("missing handover count {key}"))
        .count
}

fn add_import(conn: &Connection, hash: &str, source_name: &str, report: &str) {
    conn.execute(
        "INSERT INTO imports(hash,source_name,source_bytes,report) VALUES(?1,?2,?3,?4)",
        params![hash, source_name, vec![1_u8, 2, 3], report],
    )
    .unwrap();
}

#[test]
fn profile_default_public_opt_ins_and_unknown_fields_survive_save() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("chronicle.gvlib");
    let mut library = Library::create(&path, "Synthetic Chronicle").unwrap();

    let empty = library.chronicle_profile(ST).unwrap();
    assert_eq!(empty.revision, 0);
    assert_eq!(empty.profile.title, "Synthetic Chronicle");
    assert!(library.chronicle_profile(PLAYER).is_err());
    let public = library.public_chronicle_profile().unwrap();
    assert_eq!(public.profile.title, "Synthetic Chronicle");
    assert!(public.profile.website.is_empty());
    assert!(public.profile.email.is_empty());

    let mut profile = chronicle_profile("  Chronicle Café  ");
    assert_eq!(library.save_chronicle_profile(&profile, 0, ST).unwrap(), 1);
    assert_eq!(library.title, "Chronicle Café");
    assert_eq!(
        library.chronicle_profile(ST).unwrap().profile.title,
        "Chronicle Café"
    );

    profile.title = "Chronicle Café".into();
    profile.publish_details = true;
    assert_eq!(library.save_chronicle_profile(&profile, 1, ST).unwrap(), 2);
    let public = library.public_chronicle_profile().unwrap();
    assert_eq!(public.profile.website, profile.website);
    assert_eq!(public.profile.meeting_place, profile.meeting_place);
    assert!(public.profile.email.is_empty());
    assert!(
        !serde_json::to_string(&public)
            .unwrap()
            .contains("PRIVATE_ST_HANDOVER_NOTES")
    );

    profile.publish_details = false;
    profile.publish_contacts = true;
    assert_eq!(library.save_chronicle_profile(&profile, 2, ST).unwrap(), 3);
    let public = library.public_chronicle_profile().unwrap();
    assert_eq!(public.profile.email, profile.email);
    assert_eq!(public.profile.phone, profile.phone);
    assert!(public.profile.website.is_empty());
    assert!(public.profile.house_rules.is_empty());

    let sql = Connection::open(&path).unwrap();
    let encoded: String = sql
        .query_row(
            "SELECT data FROM chronicle_profile WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut raw: Value = serde_json::from_str(&encoded).unwrap();
    raw["future_unknown"] = json!({"keep": ["value", 17]});
    sql.execute(
        "UPDATE chronicle_profile SET data=?1 WHERE singleton=1",
        [raw.to_string()],
    )
    .unwrap();

    profile.description = "Edited after a future field was stored".into();
    assert_eq!(library.save_chronicle_profile(&profile, 3, ST).unwrap(), 4);
    let saved: String = sql
        .query_row(
            "SELECT data FROM chronicle_profile WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let saved: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved["future_unknown"], json!({"keep": ["value", 17]}));
    assert_eq!(
        saved["description"],
        "Edited after a future field was stored"
    );
    drop(sql);

    let reopened = Library::open(&path).unwrap();
    assert_eq!(reopened.title, "Chronicle Café");
    assert_eq!(reopened.chronicle_profile(ST).unwrap().profile, profile);
}

#[test]
fn profile_stale_invalid_corrupt_and_two_writer_reads_are_safe() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("freshness.gvlib");
    let mut writer = Library::create(&path, "Freshness Chronicle").unwrap();
    let mut reader = Library::open(&path).unwrap();
    let mut profile = chronicle_profile("Renamed Chronicle");
    assert_eq!(writer.save_chronicle_profile(&profile, 0, ST).unwrap(), 1);

    assert_eq!(reader.title, "Freshness Chronicle");
    assert_eq!(
        reader.chronicle_profile(ST).unwrap().profile.title,
        "Renamed Chronicle"
    );
    assert!(
        reader
            .save_chronicle_profile(&chronicle_profile("Stale title"), 0, ST)
            .is_err()
    );
    assert_eq!(reader.title, "Freshness Chronicle");
    assert!(
        writer
            .save_chronicle_profile(&chronicle_profile("Player attempt"), 1, PLAYER)
            .is_err()
    );

    let invalid = ChronicleProfile {
        title: "   ".into(),
        ..profile.clone()
    };
    assert!(writer.save_chronicle_profile(&invalid, 1, ST).is_err());
    profile.title = "Chronicle with NUL\0".into();
    assert!(writer.save_chronicle_profile(&profile, 1, ST).is_err());
    assert_eq!(writer.revision().unwrap(), 1);
    assert_eq!(writer.title, "Renamed Chronicle");

    let sql = Connection::open(&path).unwrap();
    sql.execute(
        "UPDATE chronicle_profile SET data='not-json' WHERE singleton=1",
        [],
    )
    .unwrap();
    assert_eq!(
        writer.chronicle_profile(ST).unwrap_err().code,
        "CHRONICLE_PROFILE_CORRUPT"
    );
    assert!(
        writer
            .save_chronicle_profile(&chronicle_profile("Must roll back"), 1, ST)
            .is_err()
    );
    assert_eq!(writer.revision().unwrap(), 1);
    assert_eq!(writer.title, "Renamed Chronicle");
    sql.execute(
        "UPDATE chronicle_profile SET data=?1 WHERE singleton=1",
        [json!({"title":"Different title"}).to_string()],
    )
    .unwrap();
    assert_eq!(
        writer.public_chronicle_profile().unwrap_err().code,
        "CHRONICLE_PROFILE_TITLE_MISMATCH"
    );
    assert_eq!(writer.revision().unwrap(), 1);
}

#[test]
fn profile_backup_restore_reopens_the_saved_title_and_private_fields() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("restore.gvlib");
    let backup = dir.path().join("restore.gvbackup");
    let mut library = Library::create(&path, "Before Chronicle").unwrap();
    let saved = chronicle_profile("Backup Chronicle");
    library.save_chronicle_profile(&saved, 0, ST).unwrap();
    library.backup(&backup, ST).unwrap();

    let changed = chronicle_profile("After Chronicle");
    library.save_chronicle_profile(&changed, 1, ST).unwrap();
    assert_eq!(library.title, "After Chronicle");
    assert_eq!(library.restore(&backup, 2, ST).unwrap(), 3);
    assert_eq!(library.title, "Backup Chronicle");
    let restored = library.chronicle_profile(ST).unwrap();
    assert_eq!(restored.revision, 3);
    assert_eq!(restored.profile, saved);
}

#[test]
fn handover_counts_existing_records_and_filters_contacts_notes_and_source_names() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("handover.gvlib");
    let mut library = Library::create(&path, "Handover Chronicle").unwrap();
    let profile = chronicle_profile("Handover Chronicle");
    library.save_chronicle_profile(&profile, 0, ST).unwrap();

    let vampire = library
        .create_character("Vampire", "Synthetic Vampire", 1, ST)
        .unwrap();
    library
        .create_player_profile(
            &PlayerProfile {
                name: "Synthetic Player".into(),
                ..PlayerProfile::default()
            },
            2,
            ST,
        )
        .unwrap();
    library
        .create_build(&vampire, BuildKind::Draft, 3, ST)
        .unwrap();
    library
        .create_build(&vampire, BuildKind::Test, 4, ST)
        .unwrap();

    for (kind, name, revision) in [
        ("Item", "Synthetic Item", 5),
        ("Location", "Synthetic Location", 6),
        ("Rote", "Synthetic Rote", 7),
    ] {
        library
            .create_world(
                kind,
                &WorldEdit {
                    name: name.into(),
                    fields: BTreeMap::new(),
                },
                revision,
                ST,
            )
            .unwrap();
    }
    library
        .create_story(
            &Draft::Action(Action {
                character: "Synthetic Vampire".into(),
                day: "2026-11-01".into(),
                ..Action::default()
            }),
            8,
            ST,
        )
        .unwrap();
    library
        .create_story(
            &Draft::Rumor(Rumor {
                title: "Synthetic Rumor".into(),
                day: "2026-11-01".into(),
                ..Rumor::default()
            }),
            9,
            ST,
        )
        .unwrap();
    library
        .create_plot(
            &Plot {
                name: "Synthetic Plot".into(),
                ..Plot::default()
            },
            10,
            ST,
        )
        .unwrap();

    let active_id = Uuid::new_v4().to_string();
    let active_request = SessionRequest {
        id: active_id.clone(),
        title: "Active plan".into(),
        day: "2026-11-02".into(),
        actor: "Recorder".into(),
        reason: "Synthetic count".into(),
        evidence: "Fixture".into(),
        attendance: vec![],
    };
    library
        .save_scheduled_session(&active_request, 11, ST)
        .unwrap();
    let cancelled_id = Uuid::new_v4().to_string();
    let cancelled_request = SessionRequest {
        id: cancelled_id.clone(),
        title: "Cancelled plan".into(),
        day: "2026-11-03".into(),
        actor: "Recorder".into(),
        reason: "Synthetic count".into(),
        evidence: "Fixture".into(),
        attendance: vec![],
    };
    library
        .save_scheduled_session(&cancelled_request, 12, ST)
        .unwrap();
    library
        .cancel_scheduled_session(&cancelled_id, 13, ST)
        .unwrap();

    let committed_request = SessionRequest {
        id: Uuid::new_v4().to_string(),
        title: "Committed session".into(),
        day: "2026-11-04".into(),
        actor: "Recorder".into(),
        reason: "Synthetic count".into(),
        evidence: "Fixture".into(),
        attendance: vec![Attendance {
            record_id: vampire,
            present: false,
            award: None,
        }],
    };
    let quote = library.quote_session(&committed_request, 14, ST).unwrap();
    library.commit_session(&quote, ST).unwrap();

    let sql = Connection::open(&path).unwrap();
    sql.execute(
        "INSERT INTO saved_searches(id,data) VALUES('synthetic-search','{}')",
        [],
    )
    .unwrap();
    add_import(
        &sql,
        "hash-a",
        r"C:\PRIVATE_DIRECTORY\source.gv3",
        r#"{"format":"grapevine-binary"}"#,
    );
    add_import(&sql, "hash-b", "source.xml", "not-json");

    let handover = library.chronicle_handover(false, false, false, ST).unwrap();
    assert_eq!(handover.revision, 15);
    assert_eq!(handover.profile.email, "");
    assert_eq!(handover.profile.phone, "");
    assert_eq!(handover.profile.private_notes, "");
    assert_eq!(count(&handover, "characters_total"), 1);
    assert_eq!(count(&handover, "characters_kind:Vampire"), 1);
    assert_eq!(count(&handover, "characters_without_local_xp_opening"), 1);
    assert_eq!(count(&handover, "players"), 1);
    assert_eq!(count(&handover, "draft_copies"), 1);
    assert_eq!(count(&handover, "test_copies"), 1);
    assert_eq!(count(&handover, "items"), 1);
    assert_eq!(count(&handover, "locations"), 1);
    assert_eq!(count(&handover, "rotes"), 1);
    assert_eq!(count(&handover, "actions"), 1);
    assert_eq!(count(&handover, "rumors"), 1);
    assert_eq!(count(&handover, "plots"), 1);
    assert_eq!(count(&handover, "saved_searches"), 1);
    assert_eq!(count(&handover, "scheduled_plans_active"), 1);
    assert_eq!(count(&handover, "scheduled_plans_cancelled"), 1);
    assert_eq!(count(&handover, "committed_sessions"), 1);
    assert_eq!(count(&handover, "imports"), 2);
    let audit_rows: i64 = sql
        .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count(&handover, "audit_rows"), audit_rows as u64);
    assert!(handover.sources.iter().all(|source| source.name.is_empty()));
    assert!(
        handover
            .sources
            .iter()
            .any(|source| source.format == "retained source")
    );
    assert!(
        !serde_json::to_string(&handover)
            .unwrap()
            .contains("PRIVATE_DIRECTORY")
    );
    assert!(
        library
            .chronicle_handover(true, true, true, PLAYER)
            .is_err()
    );

    let complete = library.chronicle_handover(true, true, true, ST).unwrap();
    assert_eq!(complete.profile.email, profile.email);
    assert_eq!(complete.profile.phone, profile.phone);
    assert_eq!(complete.profile.private_notes, profile.private_notes);
    assert!(
        complete
            .sources
            .iter()
            .any(|source| source.name == "source.gv3")
    );
    assert!(
        complete
            .sources
            .iter()
            .all(|source| !source.name.contains(':'))
    );
    assert!(
        !serde_json::to_string(&complete)
            .unwrap()
            .contains("PRIVATE_DIRECTORY")
    );
}

#[test]
fn handover_rejects_more_than_one_thousand_sources() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("source-limit.gvlib");
    let library = Library::create(&path, "Source Limit Chronicle").unwrap();
    let sql = Connection::open(&path).unwrap();
    let tx = sql.unchecked_transaction().unwrap();
    for index in 0..1001 {
        let hash = format!("{index:064x}");
        add_import(&tx, &hash, "synthetic.gv3", "{}");
    }
    tx.commit().unwrap();
    let error = library
        .chronicle_handover(false, false, false, ST)
        .unwrap_err();
    assert_eq!(error.code, "CHRONICLE_HANDOVER_SOURCE_LIMIT");
}
