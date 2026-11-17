use grapevine_workbench::{menus, model::*, store::Library};
use std::fs;
use tempfile::TempDir;
fn strb(out: &mut Vec<u8>, s: &[u8]) {
    out.extend((s.len() as i16).to_le_bytes());
    out.extend(s);
}
fn binary() -> Vec<u8> {
    let mut b = vec![];
    strb(&mut b, b"GVBM");
    b.extend(3f64.to_le_bytes());
    strb(&mut b, b"Caf\xe9");
    b.extend(1i16.to_le_bytes());
    strb(&mut b, b"Arcanoi");
    b.extend(6i32.to_le_bytes());
    for v in [-1i16, 0, -1, 0] {
        b.extend(v.to_le_bytes());
    }
    b.extend(5i32.to_le_bytes());
    b.extend(4i16.to_le_bytes());
    for (name, cost, note) in [
        (b"Shared".as_slice(), b"3".as_slice(), b"common".as_slice()),
        (b"Shared", b"6", b"guild"),
        (b"Common Tree", b":", b"Shared"),
        (b"Guild Tree", b"+", b"Guild"),
    ] {
        strb(&mut b, name);
        strb(&mut b, cost);
        strb(&mut b, note);
    }
    b
}
fn xml(name: &str) -> menus::MenuPreview {
    let b = format!(
        "<grapevinemenus version=\"3\" size=\"1\"><menu name=\"Arcanoi\"><item name=\"{name}\" cost=\"3\"/></menu></grapevinemenus>"
    );
    menus::inspect(b.as_bytes(), "synthetic.gvm")
        .unwrap()
        .unwrap()
}
#[test]
fn binary_fields_duplicate_names_links_and_strict_bounds_are_preserved() {
    let mut b = binary();
    let m = menus::inspect(&b, "misnamed.gex").unwrap().unwrap();
    assert_eq!(m.source_format, "gvbm-v3");
    assert_eq!(m.description, "Café");
    assert_eq!(m.rows.len(), 5);
    assert_eq!(m.rows[1].name, m.rows[2].name);
    assert_eq!(m.rows[2].attributes["note"], "guild");
    assert_eq!(m.rows[3].kind, "submenu");
    assert_eq!(m.rows[3].attributes["link"], "Shared");
    assert_eq!(m.rows[4].kind, "include");
    assert_eq!(m.rows[0].attributes["category"], "6");
    for n in 6..b.len() {
        assert!(
            menus::inspect(&b[..n], "cut.gvm").is_err(),
            "Cut accepted at {n}"
        );
    }
    b.push(0);
    assert_eq!(
        menus::inspect(&b, "tail.gvm").unwrap_err().code,
        "MENU_TRAILING_DATA"
    );
    let mut bad = binary();
    bad[6..14].copy_from_slice(&f64::NAN.to_le_bytes());
    assert_eq!(
        menus::inspect(&bad, "bad.gvm").unwrap_err().code,
        "UNSUPPORTED_MENU_VERSION"
    );
}
#[test]
fn menu_save_select_reopen_backup_restore_and_player_denial() {
    let d = TempDir::new().unwrap();
    let path = d.path().join("menus.gvlib");
    let mut l = Library::create(&path, "Menus").unwrap();
    let a = xml("First");
    let b = xml("Second");
    let rev = l.save_select_menu(&a, 0, Role::Storyteller).unwrap();
    assert_eq!(rev, 1);
    assert_eq!(l.save_select_menu(&a, rev, Role::Storyteller).unwrap(), rev);
    assert!(l.save_select_menu(&b, 0, Role::Storyteller).is_err());
    assert_eq!(l.menu_catalogues(Role::Storyteller).unwrap().len(), 1);
    assert!(l.menu_catalogues(Role::Player).is_err());
    assert!(l.active_menu(Role::Player).is_err());
    assert!(l.save_select_menu(&b, rev, Role::Player).is_err());
    assert!(l.clear_selected_menu(rev, Role::Player).is_err());
    let backup = d.path().join("before.gvbackup");
    l.backup(&backup, Role::Storyteller).unwrap();
    l.save_select_menu(&b, rev, Role::Storyteller).unwrap();
    assert_eq!(
        l.active_menu(Role::Storyteller).unwrap().unwrap().sha256,
        b.sha256
    );
    assert!(l.summaries(Role::Storyteller).unwrap().is_empty());
    l.restore(&backup, l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(l.menu_catalogues(Role::Storyteller).unwrap().len(), 1);
    assert_eq!(l.active_menu(Role::Storyteller).unwrap().unwrap(), a);
    drop(l);
    let mut l = Library::open(&path).unwrap();
    assert_eq!(l.active_menu(Role::Storyteller).unwrap().unwrap(), a);
    l.clear_selected_menu(l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert!(l.active_menu(Role::Storyteller).unwrap().is_none());
    assert_eq!(l.menu_catalogues(Role::Storyteller).unwrap().len(), 1);
}
#[test]
fn save_failure_tamper_and_source_corruption_never_replace_committed_catalogue() {
    let d = TempDir::new().unwrap();
    let path = d.path().join("menus.gvlib");
    let mut l = Library::create(&path, "Menus").unwrap();
    let a = xml("First");
    let mut tampered = a.clone();
    tampered.rows[1].name = "Changed".into();
    assert_eq!(
        l.save_select_menu(&tampered, 0, Role::Storyteller)
            .unwrap_err()
            .code,
        "MENU_PREVIEW_CHANGED"
    );
    assert_eq!(l.revision().unwrap(), 0);
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch("CREATE TRIGGER fail_menu BEFORE INSERT ON audit WHEN NEW.operation='select_menu' BEGIN SELECT RAISE(FAIL,'synthetic'); END;").unwrap();
    assert!(l.save_select_menu(&a, 0, Role::Storyteller).is_err());
    assert_eq!(l.revision().unwrap(), 0);
    assert!(l.menu_catalogues(Role::Storyteller).unwrap().is_empty());
    c.execute_batch("DROP TRIGGER fail_menu").unwrap();
    l.save_select_menu(&a, 0, Role::Storyteller).unwrap();
    c.execute(
        "UPDATE imports SET source_bytes=?1 WHERE hash=?2",
        rusqlite::params![b"corrupt".to_vec(), a.sha256],
    )
    .unwrap();
    assert!(l.active_menu(Role::Storyteller).is_err());
    assert!(l.menu_catalogues(Role::Storyteller).is_err());
    assert_eq!(
        l.save_select_menu(&a, 1, Role::Storyteller)
            .unwrap_err()
            .code,
        "MENU_SOURCE_CONFLICT"
    );
    assert_eq!(l.revision().unwrap(), 1);
}
#[test]
fn originals_binary_sources_load_completely_when_opted_in() {
    let Ok(src) = std::env::var("GRAPEVINE_LEGACY_TEST_SOURCE") else {
        return;
    };
    let dir = std::path::Path::new(&src).parent().unwrap();
    for name in ["Grapevine Menus.gvm", "Dark Ages Menus.gvm"] {
        let p = dir.join(name);
        let before = fs::read(&p).unwrap();
        let m = menus::read(&p).unwrap().unwrap();
        assert_eq!(m.source_format, "gvbm-v3");
        assert!(m.rows.iter().any(|r| r.kind == "item"));
        assert_eq!(fs::read(p).unwrap(), before);
    }
}

#[test]
fn menu_choices_never_modify_character_sheet_or_xp() {
    let d = TempDir::new().unwrap();
    let path = d.path().join("character.gvlib");
    let mut l = Library::create(&path, "Test").unwrap();
    let id = l
        .create_character("Wraith", "Synthetic", 0, Role::Storyteller)
        .unwrap();
    let c = rusqlite::Connection::open(&path).unwrap();
    let raw = |c: &rusqlite::Connection| {
        c.query_row("SELECT data FROM records WHERE id=?1", [&id], |r| {
            r.get::<_, String>(0)
        })
        .unwrap()
    };
    let before = raw(&c);
    l.save_select_menu(&xml("Candidate"), l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    l.clear_selected_menu(l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(raw(&c), before);
    assert_eq!(l.summaries(Role::Player).unwrap().len(), 1);
}
#[test]
fn default_binary_and_xml_semantic_rows_match_in_original_order() {
    let Ok(src) = std::env::var("GRAPEVINE_LEGACY_TEST_SOURCE") else {
        return;
    };
    let dir = std::path::Path::new(&src).parent().unwrap();
    let binary = menus::read(&dir.join("Grapevine Menus.gvm"))
        .unwrap()
        .unwrap();
    let xml = menus::read(&dir.join("Grapevine Menus XML.gvm"))
        .unwrap()
        .unwrap();
    assert_eq!(binary.rows.len(), 10233);
    assert_eq!(binary.description.trim(), xml.description.trim());
    fn canonical(row: &menus::MenuRow) -> (String, Vec<String>, Vec<String>) {
        let a = &row.attributes;
        let fields = match row.kind.as_str() {
            "menu" => vec![
                a.get("category").map_or("1", String::as_str),
                a.get("abc").map_or("yes", String::as_str),
                a.get("negative").map_or("no", String::as_str),
                a.get("autonote").map_or("no", String::as_str),
                a.get("required").map_or("no", String::as_str),
                a.get("display").map_or("0", String::as_str),
            ],
            "item" => vec![
                a.get("cost").map_or("1", String::as_str),
                a.get("note").map_or("", String::as_str),
            ],
            _ => vec![a.get("link").map_or(row.name.as_str(), String::as_str)],
        };
        (
            row.kind.clone(),
            row.path.clone(),
            fields.into_iter().map(str::to_owned).collect(),
        )
    }
    assert!(
        binary
            .rows
            .iter()
            .map(canonical)
            .eq(xml.rows.iter().map(canonical)),
        "Original formats differ; payload omitted"
    );
}
