use grapevine_workbench::{
    model::{Language, Role},
    searches::{Mode, Query},
    store::Library,
    trait_reports,
    trait_statistics::Kind,
};
use std::fs;
#[test]
fn selected_trait_reports_keep_original_values_distinct_characters_and_private_scope() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("traits.gex");
    fs::write(&source,br#"<grapevine version="3"><vampire name="Same" status="Active" player="PRIVATE_PLAYER"><notes>PRIVATE_NOTES</notes><traitlist name="Influences"><trait name="Police" val="2" note="PRIVATE_TRAIT_NOTE"/><trait name="Police" val="1"/></traitlist><traitlist name="Merits"><trait name="Acute" val="2"/></traitlist></vampire><mortal name="Other" status="Active"><traitlist name="Influences"><trait name="police" val="descriptive"/><trait name="" val="4"/></traitlist></mortal><wraith name="Same" status="Inactive"><traitlist name="Merits"><trait name="Acute" val="0"/></traitlist><traitlist name="Flaws"><trait name="Enemy" val="-1"/></traitlist></wraith></grapevine>"#).unwrap();
    let original = fs::read(&source).unwrap();
    let mut lib = Library::create(&dir.path().join("traits.gvlib"), "Synthetic").unwrap();
    let import = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    lib.commit_import(&import, 0, false, Role::Storyteller)
        .unwrap();
    let query = Query {
        mode: Mode::All,
        clauses: Vec::new(),
    };
    let influences = lib
        .trait_statistics_snapshot(&query, false, Kind::Influence, Role::Storyteller)
        .unwrap();
    assert_eq!(influences.search.rows.len(), 3);
    assert_eq!(influences.unnamed_entries, 1);
    assert_eq!(influences.omitted_lists, 0);
    let bucket = &influences.report.buckets[0];
    assert_eq!(bucket.characters, 2);
    assert_eq!(bucket.occurrences, 3);
    assert_eq!(bucket.known_numeric, 2);
    assert_eq!(bucket.unknown_numeric, 1);
    assert_eq!(bucket.known_sum, "3");
    assert_eq!(bucket.known_max.as_deref(), Some("2"));
    assert!(
        bucket
            .members
            .iter()
            .any(|member| member.values == ["descriptive"])
    );
    let merits = lib
        .trait_statistics_snapshot(&query, false, Kind::MeritsFlaws, Role::Storyteller)
        .unwrap();
    let acute = merits
        .report
        .buckets
        .iter()
        .find(|bucket| bucket.name == "Acute")
        .unwrap();
    assert_eq!(acute.characters, 2);
    assert_eq!(acute.members.len(), 2);
    assert!(acute.members.iter().all(|member| member.name == "Same"));
    let enemy = merits
        .report
        .buckets
        .iter()
        .find(|bucket| bucket.name == "Enemy")
        .unwrap();
    assert_eq!(enemy.known_numeric, 0);
    assert_eq!(enemy.unknown_numeric, 1);
    assert!(enemy.known_sum.is_empty());
    for snapshot in [&influences, &merits] {
        for language in [Language::English, Language::Portuguese] {
            let html = trait_reports::html(snapshot, language).unwrap();
            for secret in ["PRIVATE_PLAYER", "PRIVATE_NOTES", "PRIVATE_TRAIT_NOTE"] {
                assert!(!html.contains(secret));
            }
            for row in &snapshot.search.rows {
                assert!(!html.contains(&row.id));
            }
        }
    }
    assert_eq!(lib.revision().unwrap(), 1);
    assert_eq!(fs::read(&source).unwrap(), original);
    assert!(
        lib.trait_statistics_snapshot(&query, false, Kind::Influence, Role::Player)
            .is_err()
    );
    let id = influences
        .search
        .rows
        .iter()
        .find(|row| row.name == "Other")
        .unwrap()
        .id
        .clone();
    let sql = rusqlite::Connection::open(&lib.path).unwrap();
    let data: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut data: serde_json::Value = serde_json::from_str(&data).unwrap();
    data["private_fields"]["InfluenceList"] =
        serde_json::json!({"Traits":[{"Name":"Unsupported","Value":7}]});
    sql.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![data.to_string(), id],
    )
    .unwrap();
    assert_eq!(
        lib.trait_statistics_snapshot(&query, false, Kind::Influence, Role::Storyteller)
            .unwrap()
            .omitted_lists,
        1
    );
}
