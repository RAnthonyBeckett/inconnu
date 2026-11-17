use grapevine_workbench::{
    model::{BuildKind, Language, Role},
    searches::{Clause, Comparison, Field, Mode, Query},
    store::Library,
    trait_distribution::Request,
    trait_distribution_report,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{fs, path::Path};

const ST: Role = Role::Storyteller;

fn all_query() -> Query {
    Query {
        mode: Mode::All,
        clauses: Vec::new(),
    }
}

fn active_query() -> Query {
    Query {
        mode: Mode::All,
        clauses: vec![Clause {
            field: Field::Status,
            comparison: Comparison::Equal,
            negate: false,
            value: "Active".into(),
            trait_name: String::new(),
            number: 0,
        }],
    }
}

fn request() -> Request {
    Request {
        list: "InfluenceList".into(),
        name: " resources ".into(),
    }
}

fn imported(dir: &Path, file_name: &str, xml: &str) -> (Library, std::path::PathBuf) {
    let source = dir.join(file_name);
    fs::write(&source, xml).unwrap();
    let mut library = Library::create(&dir.join("distribution.gvlib"), "Synthetic").unwrap();
    let preview = library.preview_import(&source, "windows-1252", ST).unwrap();
    library.commit_import(&preview, 0, false, ST).unwrap();
    (library, source)
}

fn stored_records(library: &Library) -> Vec<(String, String)> {
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

fn row_id(library: &Library, name: &str) -> String {
    library
        .search_snapshot(&all_query(), false, ST)
        .unwrap()
        .rows
        .into_iter()
        .find(|row| row.name == name)
        .unwrap()
        .id
}

fn mutate_record(library: &Library, id: &str, update: impl FnOnce(&mut Value)) -> String {
    let connection = Connection::open(&library.path).unwrap();
    let encoded: String = connection
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    let mut data: Value = serde_json::from_str(&encoded).unwrap();
    update(&mut data);
    let encoded = serde_json::to_string(&data).unwrap();
    connection
        .execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![encoded, id],
        )
        .unwrap();
    encoded
}

#[test]
fn exact_distribution_respects_search_scope_and_keeps_report_private_and_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let (mut library, source) = imported(
        dir.path(),
        "distribution.gex",
        r#"<grapevine version="3"><player name="PRIVATE_PLAYER"/><vampire name="Echo" status="Active" player="PRIVATE_PLAYER" other="PRIVATE_SOURCE_EVIDENCE"><notes>PRIVATE_NOTES</notes><traitlist name="Influences"><trait name=" Resources " val="0002" note="PRIVATE_TRAIT_NOTE"/><trait name="resources" val="mystery"/></traitlist></vampire><mortal name="Echo" status="Active"><traitlist name="Influences"><trait name="RESOURCES" val="2"/></traitlist></mortal><wraith name="Else" status="Inactive"><traitlist name="Influences"><trait name="Resources" val="-1"/></traitlist></wraith></grapevine>"#,
    );
    let source_before = fs::read(&source).unwrap();
    let vampire_id = library
        .search_snapshot(&all_query(), false, ST)
        .unwrap()
        .rows
        .into_iter()
        .find(|row| row.family == "Vampire")
        .unwrap()
        .id;
    library
        .create_build(&vampire_id, BuildKind::Draft, 1, ST)
        .unwrap();

    let revision = library.revision().unwrap();
    let records_before = stored_records(&library);
    let audits_before = audit_count(&library);
    let all = library
        .trait_distribution_snapshot(&all_query(), false, &request(), ST)
        .unwrap();
    assert_eq!(all.search.rows.len(), 3); // The saved build is not a character row.
    assert!(all.search.rows.iter().all(|row| row.player.is_empty()));
    assert_eq!(all.report.characters, 3);
    assert_eq!(all.report.matched_characters, 3);
    assert_eq!(all.report.occurrences, 4);
    assert_eq!(all.report.known_occurrences, 2);
    assert_eq!(all.report.unknown_occurrences, 2);
    assert_eq!(all.report.ratings[0].value.as_deref(), Some("2"));
    assert_eq!(all.report.ratings[0].characters, 2);
    assert_eq!(all.report.ratings[0].occurrences, 2);
    assert_eq!(all.report.ratings[1].value, None);
    assert_eq!(all.report.ratings[1].characters, 2);
    assert_eq!(all.report.ratings[1].occurrences, 2);
    assert_eq!(all.report.members.len(), 3);
    assert_eq!(
        all.report
            .members
            .iter()
            .filter(|member| member.name == "Echo")
            .count(),
        2
    );
    assert!(
        all.report
            .members
            .iter()
            .any(|member| member.values == ["0002", "mystery"])
    );

    let active = library
        .trait_distribution_snapshot(&active_query(), false, &request(), ST)
        .unwrap();
    assert_eq!(active.search.rows.len(), 2);
    assert_eq!(active.report.characters, 2);
    assert_eq!(active.report.matched_characters, 2);
    assert_eq!(active.report.occurrences, 3);
    assert_eq!(active.report.unknown_occurrences, 1);

    let serialized_report = serde_json::to_string(&all.report).unwrap();
    for secret in [
        "PRIVATE_PLAYER",
        "PRIVATE_NOTES",
        "PRIVATE_TRAIT_NOTE",
        "PRIVATE_SOURCE_EVIDENCE",
    ] {
        assert!(!serialized_report.contains(secret));
    }
    for row in &all.search.rows {
        assert!(!serialized_report.contains(&row.id));
    }
    for language in [Language::English, Language::Portuguese] {
        let html = trait_distribution_report::html(&all, language).unwrap();
        for secret in [
            "PRIVATE_PLAYER",
            "PRIVATE_NOTES",
            "PRIVATE_TRAIT_NOTE",
            "PRIVATE_SOURCE_EVIDENCE",
        ] {
            assert!(!html.contains(secret), "{secret}");
        }
        for row in &all.search.rows {
            assert!(!html.contains(&row.id));
        }
    }
    assert!(
        library
            .trait_distribution_snapshot(&all_query(), false, &request(), Role::Player)
            .is_err()
    );
    assert_eq!(library.revision().unwrap(), revision);
    assert_eq!(audit_count(&library), audits_before);
    assert_eq!(stored_records(&library), records_before);
    assert_eq!(fs::read(&source).unwrap(), source_before);
}

#[test]
fn malformed_explicit_list_wins_over_legacy_fallback_and_omissions_are_counted() {
    let dir = tempfile::tempdir().unwrap();
    let (library, source) = imported(
        dir.path(),
        "malformed.gex",
        r#"<grapevine version="3"><vampire name="Malformed"><traitlist name="Influences"><trait name="Resources" val="1"/></traitlist></vampire><mortal name="Legacy"><traitlist name="Other"><trait name="Resources" val="2"/></traitlist></mortal><wraith name="Named"><traitlist name="Influences"><trait name="Resources" val="3"/></traitlist></wraith></grapevine>"#,
    );
    let malformed_id = row_id(&library, "Malformed");
    let legacy_id = row_id(&library, "Legacy");
    let named_id = row_id(&library, "Named");
    let legacy_list = json!({
        "Name": "Influences",
        "Traits": [{"Name": "Resources", "Value": "88", "Note": "PRIVATE_FALLBACK"}]
    });
    mutate_record(&library, &malformed_id, |data| {
        let private = data["private_fields"].as_object_mut().unwrap();
        private.insert("InfluenceList".into(), json!({"Traits": "malformed"}));
        private.insert("UnmappedTraitLists".into(), json!([legacy_list]));
    });
    mutate_record(&library, &legacy_id, |data| {
        for key in ["private_fields", "public_fields"] {
            data[key].as_object_mut().unwrap().remove("InfluenceList");
        }
        data["private_fields"].as_object_mut().unwrap().insert(
            "UnmappedTraitLists".into(),
            json!([{
                "Name": "Influences",
                "Traits": [{"Name": "Resources", "Value": "99"}]
            }]),
        );
    });
    mutate_record(&library, &named_id, |data| {
        data["private_fields"].as_object_mut().unwrap().insert(
            "InfluenceList".into(),
            json!({"Traits": [
                {"Name": "", "Value": "4"},
                {"Name": "Resources", "Value": "3"}
            ]}),
        );
    });
    let raw_records_before = stored_records(&library);
    let revision = library.revision().unwrap();
    let audits_before = audit_count(&library);

    let snapshot = library
        .trait_distribution_snapshot(&all_query(), false, &request(), ST)
        .unwrap();
    assert_eq!(snapshot.omitted_lists, 1);
    assert_eq!(snapshot.unnamed_entries, 1);
    assert_eq!(snapshot.report.characters, 3);
    assert_eq!(snapshot.report.missing_list_characters, 1);
    assert_eq!(snapshot.report.occurrences, 2);
    assert_eq!(
        snapshot
            .report
            .ratings
            .iter()
            .map(|rating| rating.value.as_deref())
            .collect::<Vec<_>>(),
        [Some("3"), Some("99")]
    );
    assert!(
        !serde_json::to_string(&snapshot.report)
            .unwrap()
            .contains("PRIVATE_FALLBACK")
    );
    assert_eq!(library.revision().unwrap(), revision);
    assert_eq!(audit_count(&library), audits_before);
    assert_eq!(stored_records(&library), raw_records_before);
    assert!(fs::read(source).is_ok());
}

#[test]
fn restored_library_reprojects_distribution_from_restored_source_records() {
    let dir = tempfile::tempdir().unwrap();
    let (mut library, source) = imported(
        dir.path(),
        "restore.gex",
        r#"<grapevine version="3"><vampire name="Mira"><traitlist name="Influences"><trait name="Resources" val="0002"/></traitlist></vampire></grapevine>"#,
    );
    let source_before = fs::read(&source).unwrap();
    let backup = dir.path().join("distribution.backup");
    library.backup(&backup, ST).unwrap();

    let id = row_id(&library, "Mira");
    mutate_record(&library, &id, |data| {
        let lists = data["private_fields"]
            .as_object_mut()
            .unwrap()
            .get_mut("UnmappedTraitLists")
            .unwrap();
        if let Some(rows) = lists.as_array_mut() {
            rows[0]["Traits"][0]["Value"] = json!("9");
        } else {
            lists["Traits"][0]["Value"] = json!("9");
        }
    });
    let changed = library
        .trait_distribution_snapshot(&all_query(), false, &request(), ST)
        .unwrap();
    assert_eq!(changed.report.ratings[0].value.as_deref(), Some("9"));

    let revision = library.revision().unwrap();
    library.restore(&backup, revision, ST).unwrap();
    let restored = library
        .trait_distribution_snapshot(&all_query(), false, &request(), ST)
        .unwrap();
    assert_eq!(restored.report.ratings[0].value.as_deref(), Some("2"));
    assert_eq!(restored.report.members[0].values, ["0002"]);
    assert_eq!(fs::read(&source).unwrap(), source_before);
}
