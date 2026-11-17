use grapevine_workbench::{
    importer,
    model::{Edit, Role},
    printing::{Options, PrintScope},
    sheet,
    store::Library,
};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn new_vampire_discipline_list_is_player_visible_and_private_notes_stay_hidden() {
    let dir = TempDir::new().unwrap();
    let mut library = Library::create(&dir.path().join("new-vampire.gvlib"), "Synthetic").unwrap();
    let id = library
        .create_character("Vampire", "New Vampire", 0, Role::Storyteller)
        .unwrap();

    let initial_player_view = library.record(&id, Role::Player).unwrap();
    for field in ["Clan", "Sect", "Nature", "Demeanor"] {
        assert_eq!(initial_player_view.fields[field], "");
    }
    let empty = library
        .record(&id, Role::Player)
        .unwrap()
        .fields
        .get("DisciplineList")
        .cloned()
        .unwrap();
    assert_eq!(empty, sheet::empty_list("DisciplineList"));

    let mut sheet_edit = library.sheet_edit(&id, Role::Storyteller).unwrap();
    sheet_edit.lists.get_mut("DisciplineList").unwrap()["Traits"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "Name": "Synthetic Discipline",
            "Value": "Synthetic value",
            "Note": "PRIVATE_DISCIPLINE_NOTE"
        }));
    library
        .save_sheet(&id, &sheet_edit, 1, Role::Storyteller)
        .unwrap();
    library
        .save_edit(
            &id,
            &Edit {
                name: "New Vampire".into(),
                status: "Draft".into(),
                notes: "PRIVATE_CHARACTER_NOTE".into(),
            },
            2,
            Role::Storyteller,
        )
        .unwrap();

    let player_view = library.record(&id, Role::Player).unwrap();
    assert!(player_view.notes.is_empty());
    assert_eq!(
        player_view.fields["DisciplineList"]["Traits"][0]["Name"],
        "Synthetic Discipline"
    );
    let public_document = library
        .print_document(&id, &Options::default(), 3, Role::Player)
        .unwrap();
    let public_text = serde_json::to_string(&public_document).unwrap();
    assert!(
        public_document
            .sections
            .iter()
            .flat_map(|section| &section.rows)
            .any(|row| row.name == "Synthetic Discipline" && row.value == "Synthetic value")
    );
    assert!(!public_text.contains("PRIVATE_DISCIPLINE_NOTE"));
    assert!(!public_text.contains("PRIVATE_CHARACTER_NOTE"));

    let private_document = library
        .print_document(
            &id,
            &Options {
                scope: PrintScope::PrivateStoryteller,
                include_notes: true,
                ..Options::default()
            },
            3,
            Role::Storyteller,
        )
        .unwrap();
    let private_text = serde_json::to_string(&private_document).unwrap();
    assert!(private_text.contains("PRIVATE_DISCIPLINE_NOTE"));
    assert!(private_text.contains("PRIVATE_CHARACTER_NOTE"));
}

#[test]
fn editing_an_imported_private_discipline_list_does_not_publish_it() {
    let dir = TempDir::new().unwrap();
    let mut library =
        Library::create(&dir.path().join("imported-vampire.gvlib"), "Synthetic").unwrap();
    let mut preview = importer::normalize(
        br#"<grapevine version="3"><vampire name="Imported Vampire"><traitlist name="Disciplines"><trait name="Imported Private Discipline" val="Imported value" note="PRIVATE_IMPORTED_NOTE"/></traitlist></vampire></grapevine>"#,
        "synthetic.gex",
        "windows-1252",
    )
    .unwrap();
    let imported_record = &mut preview.records[0];
    let imported_list = imported_record
        .public_fields
        .remove("DisciplineList")
        .expect("normalized synthetic import has a discipline list");
    imported_record
        .private_fields
        .entry("DisciplineList")
        .or_insert(imported_list);
    let id = preview.records[0].id.clone();
    library
        .commit_import(&preview, 0, false, Role::Storyteller)
        .unwrap();
    let mut sheet_edit = library.sheet_edit(&id, Role::Storyteller).unwrap();
    assert_eq!(
        sheet_edit.visibility.get("DisciplineList"),
        Some(&false),
        "extracted visibility: {:?}",
        sheet_edit.visibility
    );
    assert!(
        library
            .record(&id, Role::Player)
            .unwrap()
            .fields
            .get("DisciplineList")
            .is_none()
    );

    sheet_edit.lists.get_mut("DisciplineList").unwrap()["Traits"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "Name": "Edited Private Discipline",
            "Value": "Edited value",
            "Note": "PRIVATE_EDITED_NOTE"
        }));
    library
        .save_sheet(&id, &sheet_edit, 1, Role::Storyteller)
        .unwrap();

    let player_view = library.record(&id, Role::Player).unwrap();
    assert!(player_view.fields.get("DisciplineList").is_none());
    let public_document = library
        .print_document(&id, &Options::default(), 2, Role::Player)
        .unwrap();
    let public_text = serde_json::to_string(&public_document).unwrap();
    for private_value in [
        "Imported Private Discipline",
        "Edited Private Discipline",
        "PRIVATE_IMPORTED_NOTE",
        "PRIVATE_EDITED_NOTE",
    ] {
        assert!(!public_text.contains(private_value));
    }

    let private_document = library
        .print_document(
            &id,
            &Options {
                scope: PrintScope::PrivateStoryteller,
                include_notes: true,
                ..Options::default()
            },
            2,
            Role::Storyteller,
        )
        .unwrap();
    let private_text = serde_json::to_string(&private_document).unwrap();
    assert!(private_text.contains("Imported Private Discipline"));
    assert!(private_text.contains("Edited Private Discipline"));
    assert!(private_text.contains("PRIVATE_IMPORTED_NOTE"));
    assert!(private_text.contains("PRIVATE_EDITED_NOTE"));
}

#[test]
fn create_rejects_player_role_and_stale_revision_without_changing_records() {
    let dir = TempDir::new().unwrap();
    let mut library =
        Library::create(&dir.path().join("create-guards.gvlib"), "Synthetic").unwrap();

    let wrong_role = library
        .create_character("Vampire", "Denied", 0, Role::Player)
        .unwrap_err();
    assert_eq!(wrong_role.code, "ACCESS_DENIED");
    assert_eq!(library.revision().unwrap(), 0);

    library
        .create_character("Vampire", "Accepted", 0, Role::Storyteller)
        .unwrap();
    let stale = library
        .create_character("Vampire", "Stale", 0, Role::Storyteller)
        .unwrap_err();
    assert_eq!(stale.code, "STALE_REVISION");
    assert_eq!(library.revision().unwrap(), 1);
    assert_eq!(library.summaries(Role::Storyteller).unwrap().len(), 1);
}
