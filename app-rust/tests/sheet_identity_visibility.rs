use grapevine_workbench::{
    importer,
    model::{Edit, Role},
    printing::{Options, PrintScope},
    sheet,
    store::Library,
};
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

const ST: Role = Role::Storyteller;

#[test]
fn identity_traits_and_resources_share_explicitly_and_can_be_hidden_again() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("identity.gvlib");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    let id = lib
        .create_character("Vampire", "New character", 0, ST)
        .unwrap();
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.identity
        .insert("Clan".into(), "Visible Clan [ST]PRIVATE_CLAN[/ST]".into());
    edit.identity.insert("Sect".into(), "PRIVATE_SECT".into());
    edit.visibility.insert("Sect".into(), false);
    let mut physical = sheet::empty_list("PhysicalList");
    physical["Traits"] =
        json!([{"Name":"Public trait", "Value":"Descriptive value", "Note":"PRIVATE_TRAIT_NOTE"}]);
    edit.lists.insert("PhysicalList".into(), physical);
    edit.visibility.insert("PhysicalList".into(), true);
    edit.resources.insert("Blood".into(), 7);
    edit.visibility.insert("Blood".into(), true);
    lib.save_character(
        &id,
        &Edit {
            name: "New character".into(),
            status: "Draft".into(),
            notes: "PRIVATE_ST_NOTE".into(),
        },
        &edit,
        1,
        ST,
    )
    .unwrap();
    drop(lib);

    let mut lib = Library::open(&path).unwrap();
    let public = lib.record(&id, Role::Player).unwrap();
    assert_eq!(public.fields["Clan"], "Visible Clan ");
    assert!(!public.fields.contains_key("Sect"));
    assert_eq!(public.fields["Blood"], 7);
    let document = lib
        .print_document(&id, &Options::default(), 2, Role::Player)
        .unwrap();
    let text = serde_json::to_string(&document).unwrap();
    assert!(text.contains("Visible Clan"));
    assert!(text.contains("Public trait"));
    for secret in [
        "PRIVATE_CLAN",
        "PRIVATE_SECT",
        "PRIVATE_TRAIT_NOTE",
        "PRIVATE_ST_NOTE",
    ] {
        assert!(!text.contains(secret));
    }
    let private = lib
        .print_document(
            &id,
            &Options {
                scope: PrintScope::PrivateStoryteller,
                include_notes: true,
                ..Options::default()
            },
            2,
            ST,
        )
        .unwrap();
    let text = serde_json::to_string(&private).unwrap();
    for secret in [
        "PRIVATE_CLAN",
        "PRIVATE_SECT",
        "PRIVATE_TRAIT_NOTE",
        "PRIVATE_ST_NOTE",
    ] {
        assert!(text.contains(secret));
    }

    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.visibility.insert("Clan".into(), false);
    edit.visibility.insert("Blood".into(), false);
    lib.save_sheet(&id, &edit, 2, ST).unwrap();
    let public = lib.record(&id, Role::Player).unwrap();
    assert!(!public.fields.contains_key("Clan"));
    assert!(!public.fields.contains_key("Blood"));
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    assert!(edit.identity["Clan"].contains("PRIVATE_CLAN"));
    assert!(!edit.visibility["Clan"]);
    edit.visibility.insert("Clan".into(), true);
    lib.save_sheet(&id, &edit, 3, ST).unwrap();
    assert_eq!(
        lib.record(&id, Role::Player).unwrap().fields["Clan"],
        "Visible Clan "
    );
}

#[test]
fn private_imported_identity_stays_private_until_an_explicit_share_choice() {
    let dir = TempDir::new().unwrap();
    let mut lib = Library::create(&dir.path().join("import.gvlib"), "Synthetic").unwrap();
    let mut input = importer::normalize(
        br#"<grapevine version="3"><vampire name="Imported"/></grapevine>"#,
        "synthetic.gex",
        "windows-1252",
    )
    .unwrap();
    let record = &mut input.records[0];
    let id = record.id.clone();
    record.private_fields.insert(
        "Clan".into(),
        json!("Imported clan [ST]PRIVATE_SOURCE[/ST]"),
    );
    record
        .private_fields
        .insert("Nature".into(), json!({"unknown":"opaque"}));
    lib.commit_import(&input, 0, false, ST).unwrap();
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    assert!(!edit.visibility["Clan"]);
    assert!(!edit.identity.contains_key("Nature"));
    edit.identity.get_mut("Clan").unwrap().push_str(" edited");
    lib.save_sheet(&id, &edit, 1, ST).unwrap();
    assert!(
        !lib.record(&id, Role::Player)
            .unwrap()
            .fields
            .contains_key("Clan")
    );
    edit.visibility.insert("Clan".into(), true);
    lib.save_sheet(&id, &edit, 2, ST).unwrap();
    assert_eq!(
        lib.record(&id, Role::Player).unwrap().fields["Clan"],
        "Imported clan  edited"
    );
    assert_eq!(
        lib.record(&id, ST).unwrap().fields["Nature"],
        json!({"unknown":"opaque"})
    );
    edit.identity.insert("Nature".into(), "Replacement".into());
    assert!(lib.save_sheet(&id, &edit, 3, ST).is_err());
    assert_eq!(lib.revision().unwrap(), 3);
}

#[test]
fn sharing_rejects_private_metadata_and_rolls_back_with_the_sheet_audit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("rollback.gvlib");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    let id = lib
        .create_character("Vampire", "New character", 0, ST)
        .unwrap();
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.visibility
        .insert("_AppPublishedReferenceReviews".into(), true);
    assert!(lib.save_sheet(&id, &edit, 1, ST).is_err());
    edit.visibility.remove("_AppPublishedReferenceReviews");
    edit.identity.insert("Clan".into(), "Changed".into());
    assert!(lib.save_sheet(&id, &edit, 1, Role::Player).is_err());
    assert!(lib.save_sheet(&id, &edit, 0, ST).is_err());
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_identity_audit BEFORE INSERT ON audit WHEN NEW.operation='edit_sheet' BEGIN SELECT RAISE(ABORT,'synthetic audit failure'); END;").unwrap();
    assert!(lib.save_sheet(&id, &edit, 1, ST).is_err());
    assert_eq!(lib.revision().unwrap(), 1);
    assert_eq!(lib.record(&id, Role::Player).unwrap().fields["Clan"], "");
}
