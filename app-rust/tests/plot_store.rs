use grapevine_workbench::{
    importer,
    model::{Language, Role},
    plots::{self, Cast, Development, Plot, Status},
    store::Library,
};
use rusqlite::Connection;
use serde_json::Value;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

#[test]
fn saves_plot_atomically_and_restores_without_player_disclosure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chronicle.gvlib");
    let backup = dir.path().join("chronicle.backup");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    let draft = Plot {
        name: "A private mystery".into(),
        start: "2026-10-10".into(),
        end: "2026-10-20".into(),
        developments: vec![Development {
            day: "2026-10-11".into(),
            event: "The door opened.".into(),
        }],
        ..Plot::default()
    };
    assert_eq!(
        lib.create_plot(&draft, 0, PLAYER).unwrap_err().code,
        "ACCESS_DENIED"
    );
    let id = lib.create_plot(&draft, 0, ST).unwrap();
    assert_eq!(lib.revision().unwrap(), 1);
    assert!(lib.summaries(PLAYER).unwrap().iter().all(|s| s.id != id));
    assert!(lib.record(&id, PLAYER).is_err());
    assert!(lib.plot_snapshot(&id, PLAYER).is_err());
    assert!(lib.save_plot(&id, &draft, 1, PLAYER).is_err());
    lib.backup(&backup, ST).unwrap();
    let mut invalid = draft.clone();
    invalid.end = "2026-02-30".into();
    assert_eq!(
        lib.save_plot(&id, &invalid, 1, ST).unwrap_err().code,
        "PLOT_DATE_INVALID"
    );
    let mut updated = draft.clone();
    updated.outline = "Changed".into();
    assert_eq!(
        lib.save_plot(&id, &updated, 0, ST).unwrap_err().code,
        "STALE_REVISION"
    );
    assert_eq!(lib.plot_snapshot(&id, ST).unwrap().plot, draft);
    assert_eq!(lib.save_plot(&id, &updated, 1, ST).unwrap(), 2);
    drop(lib);
    let mut lib = Library::open(&path).unwrap();
    assert_eq!(lib.plot_snapshot(&id, ST).unwrap().plot, updated);
    let audit: Vec<String> = {
        let conn = Connection::open(&path).unwrap();
        let mut s = conn
            .prepare("SELECT operation FROM audit WHERE record_id=?1 ORDER BY revision")
            .unwrap();
        s.query_map([&id], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    assert_eq!(audit, ["create_plot", "edit_plot"]);
    lib.restore(&backup, 2, ST).unwrap();
    assert_eq!(lib.plot_snapshot(&id, ST).unwrap().plot, draft);
}

#[test]
fn imports_xml_chronology_and_preserves_uninterpreted_original_after_edit() {
    let bytes=br#"<?xml version="1.0"?><grapevine version="3"><plot name="Archive" id="p-7" start="10/14/2026" end="10/11/2026" narrator="Story Guide" lastmodified="10/17/2026"><outline><![CDATA[Past <future>]]></outline><traitlist name="Key Characters"><trait name="One" val="Witness" note="PRIVATE_CAST"/></traitlist><development date="17/10/2026"><event><![CDATA[First <event>]]></event><linklist><mystery preserve="true"/></linklist></development><unknown value="retained"/></plot></grapevine>"#;
    let preview = importer::normalize(bytes, "synthetic.gex", "windows-1252").unwrap();
    let r = preview.records.iter().find(|r| r.kind == "Plot").unwrap();
    let id = r.id.clone();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chronicle.gvlib");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    lib.commit_import(&preview, 0, false, ST).unwrap();
    let mut snap = lib.plot_snapshot(&id, ST).unwrap();
    assert!(snap.legacy && snap.retained);
    assert_eq!(snap.plot.start, "2026-10-14");
    assert_eq!(snap.plot.end, "10/11/2026");
    assert_eq!(snap.plot.outline, "Past <future>");
    assert_eq!(snap.plot.narrator, "Story Guide");
    assert_eq!(snap.plot.cast[0].value, "Witness");
    assert_eq!(snap.plot.developments[0].day, "2026-10-17");
    assert_eq!(snap.plot.developments[0].event, "First <event>");
    assert!(plots::html(&snap, "2026-10-18", Language::English, false).is_err());
    snap.plot.end = "2026-10-30".into();
    snap.plot.developments[0].event = "Revised".into();
    lib.save_plot(&id, &snap.plot, snap.revision, ST).unwrap();
    let c = Connection::open(&path).unwrap();
    let data: String = c
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let value: Value = serde_json::from_str(&data).unwrap();
    assert_eq!(value["original"], r.original);
    assert_eq!(value["legacy_id"], r.legacy_id.as_deref().unwrap());
    let raw: Vec<u8> = c
        .query_row("SELECT source_bytes FROM imports", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw, bytes);
    assert_eq!(
        lib.plot_snapshot(&id, ST).unwrap().plot.developments[0].event,
        "Revised"
    );
}

#[test]
fn private_report_has_explicit_status_and_sorted_escaped_events() {
    let p = Plot {
        name: "Café <script>".into(),
        start: "2026-10-05".into(),
        end: "2026-10-09".into(),
        cast: vec![Cast {
            label: "Guest".into(),
            value: "Witness".into(),
            note: "SECRET_NOTE".into(),
        }],
        developments: vec![
            Development {
                day: "2026-10-09".into(),
                event: "Later".into(),
            },
            Development {
                day: "2026-10-05".into(),
                event: "Earlier <script> & text".into(),
            },
        ],
        ..Default::default()
    };
    assert_eq!(plots::status(&p, "2026-10-04").unwrap(), Status::Pending);
    assert_eq!(plots::status(&p, "2026-10-09").unwrap(), Status::Active);
    assert_eq!(plots::status(&p, "2026-10-10").unwrap(), Status::Finished);
    let snap = plots::Snapshot {
        revision: 7,
        plot: p,
        legacy: false,
        retained: false,
    };
    let en = plots::html(&snap, "2026-10-09", Language::English, false).unwrap();
    let pt = plots::html(&snap, "2026-10-09", Language::Portuguese, true).unwrap();
    assert!(en.contains("Library revision 7") && en.contains("Private Storyteller"));
    assert!(en.contains("Café &lt;script&gt;") && !en.contains("<script>"));
    assert!(en.find("Earlier").unwrap() < en.find("Later").unwrap());
    assert!(
        !en.contains("SECRET_NOTE")
            && pt.contains("SECRET_NOTE")
            && pt.contains("Relatório privado")
    );
}
