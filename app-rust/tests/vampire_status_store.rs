use grapevine_workbench::{
    boon_book::Edit,
    model::{BuildKind, Language, Role},
    operations::{Content, Request},
    store::Library,
    vampire_status,
};
use std::fs;
const ST: Role = Role::Storyteller;
#[test]
fn status_reports_use_live_saved_vampires_and_remove_private_boon_data_before_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("status.gex");
    fs::write(&source,br#"<grapevine version="3"><vampire name="Holder &lt;literal&gt;" status="Active" title="Synthetic title" group="Synthetic group"><notes>PRIVATE_CHARACTER_NOTE</notes><traitlist name="Status"><trait name="Recorded &lt;status&gt;" val="descriptive" note="PRIVATE_TRAIT_NOTE"/></traitlist><boon type="Minor" partner="PRIVATE_PARTNER" owed="false" date="04/05/2026"><description>PRIVATE_DESCRIPTION</description></boon></vampire><vampire name="Inactive" status="Inactive"/><wraith name="Excluded" status="Active"/></grapevine>"#).unwrap();
    let before = fs::read(&source).unwrap();
    let mut lib = Library::create(&dir.path().join("status.gvlib"), "Synthetic").unwrap();
    let preview = lib.preview_import(&source, "windows-1252", ST).unwrap();
    lib.commit_import(&preview, 0, false, ST).unwrap();
    let holder = lib
        .summaries(ST)
        .unwrap()
        .into_iter()
        .find(|r| r.name.starts_with("Holder"))
        .unwrap()
        .id;
    lib.create_build(&holder, BuildKind::Draft, 1, ST).unwrap();
    let report = lib
        .vampire_status_snapshot("Active", false, true, ST)
        .unwrap();
    assert_eq!(report.characters.len(), 1);
    assert_eq!(report.characters[0].statuses[0].value, "descriptive");
    assert!(report.characters[0].boons.is_empty());
    assert!(!report.include_descriptions);
    assert!(!format!("{report:?}").contains("PRIVATE_"));
    let boons = lib
        .vampire_status_snapshot("Active", true, false, ST)
        .unwrap();
    assert_eq!(boons.characters[0].boons.len(), 1);
    assert!(boons.characters[0].boons[0].description.is_empty());
    assert_eq!(boons.characters[0].boons[0].day, None);
    assert_eq!(boons.characters[0].boons[0].owed_by_holder, Some(false));
    for language in [Language::English, Language::Portuguese] {
        let html = vampire_status::html(&boons, language).unwrap();
        assert!(html.contains("&lt;literal&gt;") && html.contains("&lt;status&gt;"));
        assert!(
            !html.contains("PRIVATE_DESCRIPTION")
                && !html.contains("PRIVATE_TRAIT_NOTE")
                && !html.contains("PRIVATE_CHARACTER_NOTE")
                && !html.contains(&holder)
        );
    }
    let detailed = lib
        .vampire_status_snapshot("Active", true, true, ST)
        .unwrap();
    assert_eq!(
        detailed.characters[0].boons[0].description,
        "PRIVATE_DESCRIPTION"
    );
    assert!(
        lib.vampire_status_snapshot("", true, true, Role::Player)
            .is_err()
    );
    assert_eq!(lib.revision().unwrap(), 2);
    let edit = Edit {
        partner: "New partner".into(),
        kind: "Major".into(),
        owed_by_holder: Some(true),
        day: "2026-01-01".into(),
        description: "PRIVATE_NEW".into(),
    };
    let id = lib.add_boon(&holder, &edit, 2, ST).unwrap();
    let current = lib
        .operations_snapshot(
            &Request::VampireStatus {
                status: "Active".into(),
                include_boons: true,
                include_descriptions: false,
            },
            ST,
        )
        .unwrap();
    let Content::VampireStatus(current) = current.content else {
        panic!("status report")
    };
    assert_eq!(current.characters[0].boons.len(), 2);
    lib.remove_boon(&holder, &id, 3, ST).unwrap();
    assert_eq!(
        lib.vampire_status_snapshot("Active", true, false, ST)
            .unwrap()
            .characters[0]
            .boons
            .len(),
        1
    );
    assert_eq!(fs::read(source).unwrap(), before);
}
