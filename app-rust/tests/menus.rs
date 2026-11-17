use grapevine_workbench::{importer, menus};
use std::fs;
use tempfile::TempDir;
#[test]
fn content_not_extension_distinguishes_menu_game_and_binary_menu() {
    let bytes=br#"<grapevinemenus version="3" size="1"><menu name="Arcanoi"><item name="Shared" cost="3" note="common"/><item name="Shared" note="guild"/></menu></grapevinemenus>"#;
    let menu = menus::inspect(bytes, "misnamed.gv3").unwrap().unwrap();
    assert_eq!(menu.rows.len(), 3);
    assert_eq!(menu.rows[1].name, menu.rows[2].name);
    assert_eq!(menu.rows[2].attributes["note"], "guild");
    assert_eq!(menu.sha256.len(), 64);
    assert_eq!(menu.rows[1].path, vec!["Arcanoi", "Shared"]);
    assert!(
        menus::inspect(
            br#"<grapevine version="3"><vampire name="Synthetic"/></grapevine>"#,
            "misnamed.gvm"
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(
        menus::inspect(b"\x04\x00GVBManything", "menu.gv3")
            .unwrap_err()
            .code,
        "UNSUPPORTED_MENU_VERSION"
    );
    assert_eq!(
        menus::inspect(
            br#"<grapevinemenus version="3" size="2"><menu name="x"/></grapevinemenus>"#,
            "menu.gvm"
        )
        .unwrap_err()
        .code,
        "MENU_COUNT_MISMATCH"
    );
}
#[test]
fn references_are_retained_and_only_existing_siblings_suggested_without_writes() {
    let d = TempDir::new().unwrap();
    let source = d.path().join("game.weird");
    let bytes=br#"<grapevine version="3" menupath="C:\Old\Example Menu.gvm"><vampire name="Synthetic"/></grapevine>"#;
    fs::write(&source, bytes).unwrap();
    let p = importer::read_preview(&source, "windows-1252").unwrap();
    assert_eq!(p.source_format, "grapevine-xml");
    assert_eq!(
        p.menu_reference.as_deref(),
        Some(r"C:\Old\Example Menu.gvm")
    );
    assert!(p.menu_candidate.is_none());
    let menu = d.path().join("Example Menu.gvm");
    fs::write(&menu, b"source unchanged").unwrap();
    let p = importer::read_preview(&source, "windows-1252").unwrap();
    assert_eq!(p.menu_candidate.as_ref(), Some(&menu));
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(fs::read(&menu).unwrap(), b"source unchanged");
    assert!(menus::sibling_reference(&source, "..").is_none());
    assert!(menus::sibling_reference(&source, "").is_none());
}
#[test]
fn real_original_xml_menu_counts_when_explicit_source_available() {
    let Ok(source) = std::env::var("GRAPEVINE_LEGACY_TEST_SOURCE") else {
        return;
    };
    let path = std::path::Path::new(&source)
        .parent()
        .unwrap()
        .join("Grapevine Menus XML.gvm");
    let before = fs::read(&path).unwrap();
    let m = menus::read(&path).unwrap().unwrap();
    assert_eq!(m.rows.len(), 10233);
    assert_eq!(m.rows.iter().filter(|r| r.kind == "item").count(), 8700);
    assert_eq!(
        m.sha256,
        "e2f978879cabb017c551d95ed1e0f3004a40426134b434ec69128f2311e48035"
    );
    assert_eq!(before, fs::read(path).unwrap());
}
