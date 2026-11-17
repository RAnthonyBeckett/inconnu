use grapevine_workbench::{
    model::Role,
    printing::{DocLanguage, Options, PrintScope},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    sheet, simple_mechanics,
    store::Library,
};
use serde_json::json;
use tempfile::TempDir;
const ST: Role = Role::Storyteller;
fn fixture(mapped: bool, visible: bool) -> (TempDir, Library, String) {
    let dir = TempDir::new().unwrap();
    let mut lib = Library::create(&dir.path().join("mechanics.gvlib"), "Synthetic").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic <literal>", 0, ST)
        .unwrap();
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    let mut list = sheet::empty_list("DisciplineList");
    let mut row = json!({"Name":"Feral Whispers","Value":"basic","Note":"PRIVATE_MECHANICS_NOTE","AppAvailableFrom":"2031-12-31"});
    if mapped {
        row["AppSourcePowerId"] = json!(ANIMALISM_FERAL_WHISPERS_ID);
    }
    list["Traits"] = json!([row]);
    edit.lists.insert("DisciplineList".into(), list);
    edit.visibility.insert("DisciplineList".into(), visible);
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    (dir, lib, id)
}
#[test]
fn complete_batch_is_source_matched_and_no_activation_cost_is_an_xp_price() {
    let data = simple_mechanics::catalogue().unwrap();
    assert_eq!(data.entries.len(), 40);
    assert!(
        data.entries
            .iter()
            .all(|c| matches!(c.status.as_str(), "reviewed" | "qualified"))
    );
    let feral = data
        .entries
        .iter()
        .find(|c| c.id == ANIMALISM_FERAL_WHISPERS_ID)
        .unwrap();
    assert!(feral.en.expenditure.to_lowercase().contains("stated"));
    assert!(!feral.en.expenditure.contains("XP"));
    let beckoning = data
        .entries
        .iter()
        .find(|c| c.id.ends_with(".beckoning"))
        .unwrap();
    assert!(beckoning.en.expenditure.contains("Social Trait"));
    assert_eq!(beckoning.retest_basis, "not_applicable");
    let aegis = data
        .entries
        .iter()
        .find(|c| c.id.ends_with(".aegis"))
        .unwrap();
    assert!(aegis.en.expenditure.to_lowercase().contains("permanent"));
}
#[test]
fn opt_in_language_sources_availability_and_privacy_are_independent() {
    let (_dir, lib, id) = fixture(true, true);
    let rev = lib.revision().unwrap();
    let before = lib.record(&id, ST).unwrap();
    for (lang, prefix) in [
        (DocLanguage::English, "Simple mechanics:"),
        (DocLanguage::Portuguese, "Regras resumidas:"),
    ] {
        let doc = lib
            .print_document(
                &id,
                &Options {
                    language: lang,
                    include_simple_mechanics: true,
                    include_references: false,
                    ..Default::default()
                },
                rev,
                Role::Player,
            )
            .unwrap();
        let section = doc
            .sections
            .iter()
            .find(|s| s.title.starts_with(prefix))
            .unwrap();
        assert!(section.title.ends_with("Feral Whispers"));
        assert!(section.rows.iter().any(|r| r.value == "2031-12-31"));
        assert!(section.rows.iter().any(|r| r.value.contains("PDF135")));
        let html = grapevine_workbench::printing::html(&doc).unwrap();
        assert!(!html.contains("PRIVATE_MECHANICS_NOTE"));
        assert!(html.contains("&lt;literal&gt;"));
        let off = lib
            .print_document(
                &id,
                &Options {
                    language: lang,
                    ..Default::default()
                },
                rev,
                Role::Player,
            )
            .unwrap();
        assert!(!off.sections.iter().any(|s| s.title.starts_with(prefix)));
    }
    assert_eq!(lib.revision().unwrap(), rev);
    assert_eq!(
        serde_json::to_value(lib.record(&id, ST).unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
}
#[test]
fn hidden_unmapped_and_invalid_tier_powers_do_not_receive_reminders() {
    let (_dir, lib, id) = fixture(true, false);
    let options = Options {
        include_simple_mechanics: true,
        ..Default::default()
    };
    let doc = lib
        .print_document(&id, &options, lib.revision().unwrap(), Role::Player)
        .unwrap();
    assert!(
        !doc.sections
            .iter()
            .any(|s| s.title.starts_with("Simple mechanics:"))
    );
    let private = lib
        .print_document(
            &id,
            &Options {
                scope: PrintScope::PrivateStoryteller,
                ..options.clone()
            },
            lib.revision().unwrap(),
            ST,
        )
        .unwrap();
    assert!(
        private
            .sections
            .iter()
            .any(|s| s.title.starts_with("Simple mechanics:"))
    );
    assert!(
        lib.print_document(
            &id,
            &Options {
                scope: PrintScope::PrivateStoryteller,
                ..options.clone()
            },
            lib.revision().unwrap(),
            Role::Player
        )
        .is_err()
    );
    let (_dir, mut lib, id) = fixture(false, true);
    let doc = lib
        .print_document(&id, &options, lib.revision().unwrap(), Role::Player)
        .unwrap();
    assert!(
        !doc.sections
            .iter()
            .any(|s| s.title.starts_with("Simple mechanics:"))
    );
    assert!(doc.warnings.iter().any(|s| s.contains("unmapped")));
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    let row = &mut edit.lists.get_mut("DisciplineList").unwrap()["Traits"][0];
    row["AppSourcePowerId"] = json!(ANIMALISM_FERAL_WHISPERS_ID);
    row["Value"] = json!("intermediate");
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    let doc = lib
        .print_document(&id, &options, lib.revision().unwrap(), Role::Player)
        .unwrap();
    assert!(
        !doc.sections
            .iter()
            .any(|s| s.title.starts_with("Simple mechanics:"))
    );
}
