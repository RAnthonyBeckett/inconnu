use grapevine_workbench::{
    actions::Action,
    combined_story::Request,
    model::{BuildKind, Edit as RecordEdit, Role},
    rumor_audience,
    rumors::{Entry, Rumor},
    store::Library,
    story_work::Draft,
    world,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

fn action(label: &str, day: &str) -> Action {
    Action {
        character: label.into(),
        day: day.into(),
        slots: Vec::new(),
    }
}

fn create_rumor(library: &mut Library, title: &str, day: &str, body: &str) -> String {
    let revision = library.revision().unwrap();
    library
        .create_story(
            &Draft::Rumor(Rumor {
                title: title.into(),
                day: day.into(),
                entries: vec![Entry {
                    level: "0".into(),
                    body: body.into(),
                }],
                ..Rumor::default()
            }),
            revision,
            ST,
        )
        .unwrap()
}

fn assign_rumor(library: &mut Library, rumor_id: &str, character_id: &str) {
    let revision = library.revision().unwrap();
    let ids = vec![character_id.to_owned()];
    let preview = library
        .preview_rumor_audience(rumor_id, &ids, revision, ST)
        .unwrap();
    library.commit_rumor_audience(&preview, ST).unwrap();
}

fn read_record(path: &std::path::Path, id: &str) -> Value {
    let connection = Connection::open(path).unwrap();
    let encoded: String = connection
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    serde_json::from_str(&encoded).unwrap()
}

fn write_record(path: &std::path::Path, id: &str, update: impl FnOnce(&mut Value)) {
    let connection = Connection::open(path).unwrap();
    let mut record = read_record(path, id);
    update(&mut record);
    connection
        .execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![record.to_string(), id],
        )
        .unwrap();
}

fn raw_records(library: &Library) -> Vec<(String, String)> {
    let connection = Connection::open(&library.path).unwrap();
    let mut statement = connection
        .prepare("SELECT id,data FROM records ORDER BY id")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn audit_count(library: &Library) -> i64 {
    Connection::open(&library.path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
        .unwrap()
}

fn request(character_id: &str) -> Request {
    Request {
        character_id: character_id.into(),
        from: "2026-01-01".into(),
        through: "2026-01-03".into(),
    }
}

#[test]
fn combined_review_uses_exact_ids_and_keeps_dated_audience_and_legacy_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("combined.gvlib");
    let mut library = Library::create(&path, "Synthetic combined story").unwrap();
    let first = library.create_character("Vampire", "Twin", 0, ST).unwrap();
    let second = library.create_character("Vampire", "Twin", 1, ST).unwrap();

    let mut revision = library.revision().unwrap();
    let first_action = library
        .create_linked_action(&action("Twin", "2026-01-01"), Some(&first), revision, ST)
        .unwrap();
    revision = library.revision().unwrap();
    let second_action = library
        .create_linked_action(&action("Twin", "2026-01-02"), Some(&second), revision, ST)
        .unwrap();
    revision = library.revision().unwrap();
    let undated_action = library
        .create_linked_action(&action("Twin", "2026-01-02"), Some(&first), revision, ST)
        .unwrap();
    revision = library.revision().unwrap();
    let out_of_range_action = library
        .create_linked_action(&action("Twin", "2026-01-04"), Some(&first), revision, ST)
        .unwrap();
    revision = library.revision().unwrap();
    library
        .create_story(
            &Draft::Action(action("Unlinked", "2026-01-02")),
            revision,
            ST,
        )
        .unwrap();

    let first_rumor = create_rumor(
        &mut library,
        "Twin first rumor",
        "2026-01-01",
        "PRIVATE_STORY_BODY",
    );
    write_record(&path, &first_rumor, |record| {
        record["original"]["future_source_evidence"] = json!("RAW_FUTURE_SOURCE");
        record["private_fields"]["AppRumorV1"]["future_rumor_field"] = json!({"preserve": true});
        record["private_fields"][rumor_audience::FIELD] =
            json!({"recipients": [], "future_root": {"preserve": true}});
    });
    let second_rumor = create_rumor(
        &mut library,
        "Twin second rumor",
        "2026-01-02",
        "SECOND_CHARACTER_ONLY",
    );
    let unassigned_rumor = create_rumor(
        &mut library,
        "Unassigned rumor",
        "2026-01-03",
        "UNASSIGNED_BODY",
    );
    let out_of_range_rumor = create_rumor(
        &mut library,
        "Outside range rumor",
        "2026-01-04",
        "OUTSIDE_BODY",
    );
    let undated_rumor = create_rumor(&mut library, "Undated rumor", "2026-01-02", "UNDATED_BODY");
    assign_rumor(&mut library, &first_rumor, &first);
    assign_rumor(&mut library, &second_rumor, &second);
    assign_rumor(&mut library, &out_of_range_rumor, &first);
    write_record(&path, &first_rumor, |record| {
        record["private_fields"][rumor_audience::FIELD]["recipients"][0]["future_binding"] =
            json!("preserve");
    });
    write_record(&path, &undated_action, |record| {
        record["private_fields"]["AppActionV1"]["day"] = json!("");
    });
    write_record(&path, &undated_rumor, |record| {
        record["private_fields"]["AppRumorV1"]["day"] = json!("");
    });
    let revision = library.revision().unwrap();
    library
        .save_edit(
            &first,
            &RecordEdit {
                name: "Renamed Twin".into(),
                status: "Active".into(),
                notes: String::new(),
            },
            revision,
            ST,
        )
        .unwrap();

    let revision = library.revision().unwrap();
    let records_before = raw_records(&library);
    let audits_before = audit_count(&library);
    let query = request(&first);
    let snapshot = library.combined_story_snapshot(&query, ST).unwrap();
    assert_eq!(snapshot.request, query);
    assert_eq!(snapshot.actions.revision, revision);
    assert_eq!(snapshot.actions.character_id, first);
    assert_eq!(snapshot.actions.character_name, "Renamed Twin");
    assert_eq!(snapshot.actions.undated_omitted, 1);
    assert_eq!(snapshot.actions.unlinked_in_library, 1);
    assert_eq!(snapshot.actions.rows.len(), 1);
    assert_eq!(snapshot.actions.rows[0].action_id, first_action);
    assert_ne!(snapshot.actions.rows[0].action_id, second_action);
    assert_ne!(snapshot.actions.rows[0].action_id, out_of_range_action);
    let Draft::Action(action) = &snapshot.actions.rows[0].snapshot.draft else {
        panic!("the combined Action timeline should contain an Action")
    };
    assert_eq!(action.character, "Renamed Twin");

    assert_eq!(snapshot.undated_rumors, 1);
    assert_eq!(snapshot.unassigned_rumors_in_range, 1);
    assert_eq!(snapshot.rumors.len(), 1);
    assert_eq!(snapshot.rumors[0].record_id, first_rumor);
    let Draft::Rumor(rumor) = &snapshot.rumors[0].snapshot.draft else {
        panic!("the combined Rumor row should contain a Rumor")
    };
    assert_eq!(rumor.title, "Twin first rumor");
    assert_eq!(rumor.entries[0].body, "PRIVATE_STORY_BODY");
    assert_ne!(snapshot.rumors[0].record_id, second_rumor);
    assert_ne!(snapshot.rumors[0].record_id, unassigned_rumor);
    assert_ne!(snapshot.rumors[0].record_id, out_of_range_rumor);
    assert_ne!(snapshot.rumors[0].record_id, undated_rumor);

    let raw_after = raw_records(&library);
    assert_eq!(raw_after, records_before);
    let source_record = raw_after.iter().find(|(id, _)| id == &first_rumor).unwrap();
    let raw: Value = serde_json::from_str(&source_record.1).unwrap();
    assert_eq!(
        raw["original"]["future_source_evidence"],
        "RAW_FUTURE_SOURCE"
    );
    assert_eq!(
        raw["private_fields"]["AppRumorV1"]["future_rumor_field"],
        json!({"preserve": true})
    );
    assert_eq!(
        raw["private_fields"][rumor_audience::FIELD]["future_root"],
        json!({"preserve": true})
    );
    assert_eq!(
        raw["private_fields"][rumor_audience::FIELD]["recipients"][0]["future_binding"],
        "preserve"
    );
    assert_eq!(library.revision().unwrap(), revision);
    assert_eq!(audit_count(&library), audits_before);
}

#[test]
fn empty_ranges_work_but_player_build_and_wrong_kind_requests_are_denied() {
    let dir = tempfile::tempdir().unwrap();
    let mut library = Library::create(&dir.path().join("scope.gvlib"), "Scope").unwrap();
    let character = library.create_character("Vampire", "Mira", 0, ST).unwrap();
    let revision = library.revision().unwrap();
    let build = library
        .create_build(&character, BuildKind::Draft, revision, ST)
        .unwrap();
    let revision = library.revision().unwrap();
    let item = library
        .create_world(
            "Item",
            &world::Edit {
                name: "Local item".into(),
                fields: BTreeMap::new(),
            },
            revision,
            ST,
        )
        .unwrap();
    let revision = library.revision().unwrap();
    let player_record = library.create_player("Owner label", revision, ST).unwrap();
    let query = request(&character);

    let empty = library.combined_story_snapshot(&query, ST).unwrap();
    assert!(empty.actions.rows.is_empty());
    assert!(empty.rumors.is_empty());
    assert_eq!(empty.undated_rumors, 0);
    assert_eq!(empty.unassigned_rumors_in_range, 0);
    assert!(library.combined_story_snapshot(&query, PLAYER).is_err());
    assert!(
        library
            .combined_story_snapshot(&request(&build), ST)
            .is_err()
    );
    assert!(
        library
            .combined_story_snapshot(&request(&item), ST)
            .is_err()
    );
    assert!(
        library
            .combined_story_snapshot(&request(&player_record), ST)
            .is_err()
    );

    let mut invalid_dates = query;
    invalid_dates.from = "2026-01-04".into();
    invalid_dates.through = "2026-01-01".into();
    assert!(library.combined_story_snapshot(&invalid_dates, ST).is_err());
}

#[test]
fn combined_snapshot_reprojects_durable_links_after_backup_restore() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("restore.gvlib");
    let mut library = Library::create(&path, "Restore combined story").unwrap();
    let character = library.create_character("Mortal", "Mira", 0, ST).unwrap();
    let revision = library.revision().unwrap();
    library
        .create_linked_action(
            &action("Mira", "2026-01-02"),
            Some(&character),
            revision,
            ST,
        )
        .unwrap();
    let rumor = create_rumor(
        &mut library,
        "Restored rumor",
        "2026-01-02",
        "Restored text",
    );
    assign_rumor(&mut library, &rumor, &character);
    let backup = dir.path().join("combined.backup");
    library.backup(&backup, ST).unwrap();

    create_rumor(
        &mut library,
        "Not in backup",
        "2026-01-02",
        "Temporary extra",
    );
    let revision = library.revision().unwrap();
    library.restore(&backup, revision, ST).unwrap();
    let restored = library
        .combined_story_snapshot(&request(&character), ST)
        .unwrap();
    assert_eq!(restored.actions.rows.len(), 1);
    assert_eq!(restored.rumors.len(), 1);
    assert_eq!(restored.rumors[0].record_id, rumor);
}

#[test]
fn combined_limit_accepts_500_rows_and_rejects_501_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    let mut library = Library::create(&dir.path().join("boundary.gvlib"), "Boundary").unwrap();
    let character = library.create_character("Vampire", "Mira", 0, ST).unwrap();
    for _ in 0..499 {
        let revision = library.revision().unwrap();
        library
            .create_linked_action(
                &action("Mira", "2026-01-02"),
                Some(&character),
                revision,
                ST,
            )
            .unwrap();
    }

    let first = create_rumor(&mut library, "Included rumor", "2026-01-02", "One row");
    assign_rumor(&mut library, &first, &character);
    let request = request(&character);
    let exactly_500 = library.combined_story_snapshot(&request, ST).unwrap();
    assert_eq!(exactly_500.actions.rows.len(), 499);
    assert_eq!(exactly_500.rumors.len(), 1);
    assert_eq!(
        exactly_500.actions.rows.len() + exactly_500.rumors.len(),
        500
    );

    let second = create_rumor(
        &mut library,
        "Second included rumor",
        "2026-01-02",
        "One more",
    );
    assign_rumor(&mut library, &second, &character);
    assert_eq!(
        library
            .action_timeline(&character, &request.from, &request.through, ST)
            .unwrap()
            .rows
            .len(),
        499
    );
    assert_eq!(
        library
            .rumor_audience_queue(&request.from, &request.through, ST)
            .unwrap()
            .rows
            .len(),
        2
    );
    let revision = library.revision().unwrap();
    let audits = audit_count(&library);
    let error = library.combined_story_snapshot(&request, ST).unwrap_err();
    assert_eq!(error.code, "COMBINED_STORY_LIMIT");
    assert_eq!(library.revision().unwrap(), revision);
    assert_eq!(audit_count(&library), audits);
}
