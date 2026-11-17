use grapevine_workbench::{
    importer,
    model::{Language, Role},
    plots,
    rumors::{Entry, Rumor},
    store::Library,
    story_work::{self, Draft, Kind},
};
use serde_json::Value;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

fn i16v(b: &mut Vec<u8>, n: i16) {
    b.extend(n.to_le_bytes());
}
fn i32v(b: &mut Vec<u8>, n: i32) {
    b.extend(n.to_le_bytes());
}
fn date(b: &mut Vec<u8>, d: f64) {
    b.extend(d.to_le_bytes());
}
fn strv(b: &mut Vec<u8>, s: &str) {
    i16v(b, s.len() as i16);
    b.extend(s.as_bytes());
}
fn effect(b: &mut Vec<u8>) {
    i16v(b, 1);
    i32v(b, 1);
    date(b, 25571.0);
    strv(b, "RETAINED_EFFECT");
    strv(b, "Subitem");
}

/// Minimal binary GEX v3 following the reader's actual section and nested field order.
fn binary_story() -> Vec<u8> {
    let mut b = Vec::new();
    strv(&mut b, "GVBE");
    date(&mut b, 3.0);
    for _ in 0..10 {
        i16v(&mut b, 0);
    }
    // Action: date, character, done, last modified, subactions.
    i16v(&mut b, 1);
    date(&mut b, 25569.5);
    strv(&mut b, "Archive Actor");
    i16v(&mut b, -1);
    date(&mut b, 25570.0);
    i16v(&mut b, 1);
    strv(&mut b, "Research");
    for value in [2, 1, 3, 0] {
        i16v(&mut b, value);
    }
    effect(&mut b);
    strv(&mut b, "Inspect <literal> item.");
    strv(&mut b, "Found a clue.");
    // Plot: date sentinels, narrator, outline, linked cast list, development.
    i16v(&mut b, 1);
    strv(&mut b, "Archive Plot");
    date(&mut b, 25569.75);
    date(&mut b, 0.0);
    strv(&mut b, "Narrator");
    date(&mut b, 25570.0);
    strv(&mut b, "Plan");
    strv(&mut b, "Key Characters");
    for v in [-1, 0, 0] {
        i16v(&mut b, v);
    }
    i32v(&mut b, 0);
    i16v(&mut b, 1);
    strv(&mut b, "Witness");
    strv(&mut b, "1");
    strv(&mut b, "PRIVATE_CAST");
    i16v(&mut b, 1);
    date(&mut b, 25570.5);
    effect(&mut b);
    strv(&mut b, "Event");
    // Influence rumor: nonempty MultiKey means no binary Query structure.
    i16v(&mut b, 1);
    strv(&mut b, "Archive Rumor");
    date(&mut b, 25570.0);
    i32v(&mut b, 1);
    strv(&mut b, "Influence");
    strv(&mut b, "Records");
    i16v(&mut b, -1);
    date(&mut b, 25570.0);
    i16v(&mut b, 1);
    i16v(&mut b, 2);
    strv(&mut b, "Rumor body");
    b
}

#[test]
fn real_binary_reader_dates_and_nested_effects_flow_into_private_editors() {
    let bytes = binary_story();
    let preview = importer::normalize(&bytes, "synthetic.gex", "windows-1252").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("story.gvlib");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    lib.commit_import(&preview, 0, false, ST).unwrap();
    assert_eq!(preview.records.len(), 3);
    for source in &preview.records {
        let id = &source.id;
        assert!(lib.record(id, PLAYER).is_err());
        match source.kind.as_str() {
            "Plot" => {
                let snap = lib.plot_snapshot(id, ST).unwrap();
                assert_eq!(snap.plot.start, "1970-01-01");
                assert_eq!(snap.plot.end, "");
                assert_eq!(snap.plot.developments[0].day, "1970-01-02");
                assert!(snap.retained);
                let report = plots::html(&snap, "1970-01-02", Language::English, false).unwrap();
                assert!(!report.contains("RETAINED_EFFECT") && !report.contains("PRIVATE_CAST"));
                lib.save_plot(id, &snap.plot, snap.revision, ST).unwrap();
            }
            "Action" => {
                let snap = lib.story_snapshot(id, ST).unwrap();
                let Draft::Action(a) = &snap.draft else {
                    panic!("Expected Action")
                };
                assert_eq!(a.day, "1970-01-01");
                assert_eq!(a.character, "Archive Actor");
                assert_eq!(a.slots[0].unused, "1");
                assert!(a.is_complete() && snap.retained);
                let report = story_work::html(&snap, Language::English).unwrap();
                assert!(report.contains("&lt;literal&gt;") && !report.contains("RETAINED_EFFECT"));
                lib.save_story(id, &snap.draft, snap.revision, ST).unwrap();
            }
            "Rumor" => {
                let snap = lib.story_snapshot(id, ST).unwrap();
                let Draft::Rumor(r) = &snap.draft else {
                    panic!("Expected Rumor")
                };
                assert_eq!(r.day, "1970-01-02");
                assert_eq!(r.category, 1);
                assert_eq!(r.entries[0].level, "2");
                assert!(!r.reviewed, "Imported Done does not equal local review");
                lib.save_story(id, &snap.draft, snap.revision, ST).unwrap();
            }
            other => panic!("Unexpected kind {other}"),
        }
        let sql = rusqlite::Connection::open(&path).unwrap();
        let data: String = sql
            .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
            .unwrap();
        let data: Value = serde_json::from_str(&data).unwrap();
        assert_eq!(data["original"], source.original);
        assert_eq!(data["id"], *id);
        let imported: Vec<u8> = sql
            .query_row("SELECT source_bytes FROM imports", [], |r| r.get(0))
            .unwrap();
        assert_eq!(imported, bytes);
    }
    assert!(lib.summaries(PLAYER).unwrap().is_empty());
}

#[test]
fn date_range_master_filters_saved_records_without_inferring_audience() {
    let xml=br#"<grapevine version="3"><rumor title="Ambiguous" date="10/11/2026" category="4" done="yes"><query name="PRIVATE_QUERY"/><subrumor><body>Legacy rumor</body></subrumor></rumor></grapevine>"#;
    let preview = importer::normalize(xml, "synthetic.gex", "windows-1252").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("story.gvlib");
    let backup = dir.path().join("story.backup");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    lib.commit_import(&preview, 0, false, ST).unwrap();
    let r = lib.story_snapshot(&preview.records[0].id, ST).unwrap();
    let Draft::Rumor(old) = r.draft else {
        panic!("Expected Rumor")
    };
    assert_eq!(old.day, "10/11/2026");
    assert_eq!(old.category, 4);
    assert_eq!(old.entries[0].level, "0");
    assert!(!old.reviewed && r.retained);
    let base = Rumor {
        title: "A rumor".into(),
        day: "2026-11-12".into(),
        category: 1,
        audience_label: "Group <literal>".into(),
        influence_list: "Influence".into(),
        influence_trait: "Records".into(),
        reviewed: false,
        entries: vec![Entry {
            level: "2".into(),
            body: "Private rumor.".into(),
        }],
    };
    let draft = Draft::Rumor(base.clone());
    assert!(lib.create_story(&draft, 1, PLAYER).is_err());
    let in_range = lib.create_story(&draft, 1, ST).unwrap();
    let earlier = Draft::Rumor(Rumor {
        day: "2026-11-01".into(),
        reviewed: true,
        title: "Earlier".into(),
        ..base.clone()
    });
    lib.create_story(&earlier, 2, ST).unwrap();
    let outside = Draft::Rumor(Rumor {
        day: "2026-12-01".into(),
        title: "Outside range".into(),
        ..base.clone()
    });
    lib.create_story(&outside, 3, ST).unwrap();
    assert!(
        lib.master_story(Kind::Rumor, "2026-11-01", "2026-11-30", false, PLAYER)
            .is_err()
    );
    let master = lib
        .master_story(Kind::Rumor, "2026-11-01", "2026-11-30", false, ST)
        .unwrap();
    assert_eq!(master.undated_omitted, 1);
    assert_eq!(master.records.len(), 2);
    assert_eq!(master.records[0].draft.label(), "Earlier");
    let html = story_work::master_html(&master, Language::Portuguese).unwrap();
    assert!(html.contains("Relatório geral de rumores") && html.contains("&lt;literal&gt;"));
    assert!(!html.contains("Outside range") && !html.contains("PRIVATE_QUERY"));
    let incomplete = lib
        .master_story(Kind::Rumor, "2026-11-01", "2026-11-30", true, ST)
        .unwrap();
    assert_eq!(incomplete.records.len(), 1);
    assert_eq!(incomplete.records[0].draft.label(), "A rumor");
    let invalid = Draft::Rumor(Rumor {
        reviewed: true,
        entries: vec![],
        ..base.clone()
    });
    assert_eq!(
        lib.save_story(&in_range, &invalid, 4, ST).unwrap_err().code,
        "RUMOR_REVIEW_INCOMPLETE"
    );
    assert_eq!(
        lib.save_story(&in_range, &draft, 3, ST).unwrap_err().code,
        "STALE_REVISION"
    );
    lib.backup(&backup, ST).unwrap();
    let updated = Draft::Rumor(Rumor {
        reviewed: true,
        ..base
    });
    lib.save_story(&in_range, &updated, 4, ST).unwrap();
    drop(lib);
    let mut lib = Library::open(&path).unwrap();
    assert!(lib.story_snapshot(&in_range, ST).unwrap().draft.ready());
    lib.restore(&backup, 5, ST).unwrap();
    assert!(!lib.story_snapshot(&in_range, ST).unwrap().draft.ready());
}
