use grapevine_workbench::{diagnostics, importer, model::*, store::Library};
use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;

fn fixture(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("sample.gex");
    fs::write(&path,br#"<?xml version="1.0"?><grapevine version="3">
      <player name="Owner" id="p1" email="private@example.invalid"><notes>CONTACT_SECRET</notes></player>
      <vampire name="Alex[ST]NAME_SECRET[/ST]" id="c1" player="Owner" status="Active" strange="UNMAPPED_SECRET">
        <notes>ST_SECRET</notes><biography>PRIVATE_HISTORY</biography>
        <traitlist name="Disciplines"><trait name="Power" val="Elder" note="TRAIT_SECRET"/></traitlist>
        <mystery value="retain">UNKNOWN_VALUE</mystery>
      </vampire></grapevine>"#).unwrap();
    path
}

#[test]
fn xml_import_edit_reopen_preserves_fields_links_and_source() {
    let d = TempDir::new().unwrap();
    let source = fixture(d.path());
    let original = fs::read(&source).unwrap();
    let db = d.path().join("game.gvlib");
    let mut lib = Library::create(&db, "Test chronicle").unwrap();
    let preview = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    assert_eq!(preview.records.len(), 2);
    let expected = preview.records.clone();
    assert_eq!(
        preview
            .records
            .iter()
            .find(|r| r.kind == "Vampire")
            .unwrap()
            .public_fields["DisciplineList"]["Traits"][0]["Value"],
        "Elder"
    );
    lib.commit_import(&preview, 0, false, Role::Storyteller)
        .unwrap();
    let list = lib.summaries(Role::Storyteller).unwrap();
    let character = list
        .iter()
        .find(|r| r.kind == "Vampire")
        .unwrap()
        .id
        .clone();
    let player = list.iter().find(|r| r.kind == "Player").unwrap().id.clone();
    lib.save_edit(
        &player,
        &Edit {
            name: "Renamed owner".into(),
            status: "Active".into(),
            notes: "private".into(),
        },
        1,
        Role::Storyteller,
    )
    .unwrap();
    assert_eq!(
        lib.record(&character, Role::Storyteller)
            .unwrap()
            .player_name,
        "Renamed owner"
    );
    lib.save_edit(
        &character,
        &Edit {
            name: "Renamed Alex".into(),
            status: "Draft".into(),
            notes: "New ST notes".into(),
        },
        2,
        Role::Storyteller,
    )
    .unwrap();
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let view = lib.record(&character, Role::Storyteller).unwrap();
    assert_eq!(view.name, "Renamed Alex");
    assert_eq!(view.notes, "New ST notes");
    assert_eq!(view.fields["strange"], "UNMAPPED_SECRET");
    assert_eq!(view.fields["Notes"], "ST_SECRET");
    let conn = rusqlite::Connection::open(&db).unwrap();
    for prior in expected {
        let data: String = conn
            .query_row("SELECT data FROM records WHERE id=?1", [&prior.id], |r| {
                r.get(0)
            })
            .unwrap();
        let current: Record = serde_json::from_str(&data).unwrap();
        assert_eq!(current.original, prior.original);
        assert_eq!(current.source_sha256, prior.source_sha256);
        if current.kind == "Vampire" {
            assert_eq!(current.player_id.as_deref(), Some(player.as_str()));
        }
    }
    let saved_source: Vec<u8> = conn
        .query_row("SELECT source_bytes FROM imports", [], |r| r.get(0))
        .unwrap();
    assert_eq!(saved_source, original);
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn player_view_does_not_expose_private_or_raw_data_and_cannot_write() {
    let d = TempDir::new().unwrap();
    let source = fixture(d.path());
    let mut lib = Library::create(&d.path().join("game.gvlib"), "Private").unwrap();
    let preview = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    lib.commit_import(&preview, 0, false, Role::Storyteller)
        .unwrap();
    let summaries = lib.summaries(Role::Player).unwrap();
    let mut serialized = serde_json::to_string(&summaries).unwrap();
    for summary in summaries {
        serialized +=
            &serde_json::to_string(&lib.record(&summary.id, Role::Player).unwrap()).unwrap();
    }
    for secret in [
        "ST_SECRET",
        "CONTACT_SECRET",
        "private@example",
        "NAME_SECRET",
        "UNMAPPED_SECRET",
        "UNKNOWN_VALUE",
        "TRAIT_SECRET",
        "PRIVATE_HISTORY",
        "original",
        "source_bytes",
    ] {
        assert!(!serialized.contains(secret), "Leaked {secret}");
    }
    let id = lib.summaries(Role::Player).unwrap()[0].id.clone();
    assert_eq!(
        lib.save_edit(&id, &Edit::default(), 1, Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert_eq!(
        lib.preview_import(&source, "windows-1252", Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert_eq!(
        lib.backup(&d.path().join("denied"), Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
}

#[test]
fn repeated_conflicting_and_stale_imports_are_not_silent() {
    let d = TempDir::new().unwrap();
    let source = fixture(d.path());
    let mut lib = Library::create(&d.path().join("game.gvlib"), "Test").unwrap();
    let preview = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    lib.commit_import(&preview, 0, false, Role::Storyteller)
        .unwrap();
    assert_eq!(
        lib.preview_import(&source, "windows-1252", Role::Storyteller)
            .unwrap_err()
            .code,
        "ALREADY_IMPORTED"
    );
    fs::write(
        &source,
        fs::read_to_string(&source)
            .unwrap()
            .replace("ST_SECRET", "CHANGED_ST_SECRET"),
    )
    .unwrap();
    let changed = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    assert_eq!(changed.conflicts.len(), 2);
    assert_eq!(
        lib.commit_import(&changed, 1, false, Role::Storyteller)
            .unwrap_err()
            .code,
        "IMPORT_CONFLICT"
    );
    assert_eq!(lib.revision().unwrap(), 1);
    let result = lib
        .commit_import(&changed, 1, true, Role::Storyteller)
        .unwrap();
    assert_eq!((result.inserted, result.skipped), (0, 2));
    assert_eq!(lib.summaries(Role::Storyteller).unwrap().len(), 2);
    assert_eq!(
        lib.commit_import(&preview, 1, true, Role::Storyteller)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
}

#[test]
fn concurrent_save_failure_and_restore_preserve_committed_state() {
    let d = TempDir::new().unwrap();
    let path = d.path().join("game.gvlib");
    let mut lib = Library::create(&path, "Test").unwrap();
    let id = lib
        .create_character("Risen", "Original", 0, Role::Storyteller)
        .unwrap();
    let backup = d.path().join("snapshot.gvlib");
    lib.backup(&backup, Role::Storyteller).unwrap();
    let mut second = Library::open(&path).unwrap();
    lib.save_edit(
        &id,
        &Edit {
            name: "Changed".into(),
            status: "Draft".into(),
            notes: "secret".into(),
        },
        1,
        Role::Storyteller,
    )
    .unwrap();
    assert_eq!(
        second
            .save_edit(
                &id,
                &Edit {
                    name: "Stale".into(),
                    ..Default::default()
                },
                1,
                Role::Storyteller
            )
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_edit BEFORE UPDATE ON records BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(
        lib.save_edit(
            &id,
            &Edit {
                name: "Lost".into(),
                ..Default::default()
            },
            2,
            Role::Storyteller
        )
        .is_err()
    );
    assert_eq!(lib.revision().unwrap(), 2);
    assert_eq!(lib.record(&id, Role::Storyteller).unwrap().name, "Changed");
    conn.execute_batch("DROP TRIGGER fail_edit; CREATE TRIGGER fail_restore BEFORE INSERT ON records BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(lib.restore(&backup, 2, Role::Storyteller).is_err());
    assert_eq!(lib.revision().unwrap(), 2);
    assert_eq!(lib.record(&id, Role::Storyteller).unwrap().name, "Changed");
    conn.execute_batch("DROP TRIGGER fail_restore;").unwrap();
    let revision = lib.restore(&backup, 2, Role::Storyteller).unwrap();
    assert!(revision > 2);
    assert_eq!(lib.record(&id, Role::Storyteller).unwrap().name, "Original");
    assert_eq!(
        second
            .save_edit(
                &id,
                &Edit {
                    name: "Stale again".into(),
                    ..Default::default()
                },
                2,
                Role::Storyteller
            )
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert!(lib.backup(&backup, Role::Storyteller).is_err());
}

#[test]
fn future_schema_bad_backup_and_partial_files_are_rejected_without_writes() {
    let d = TempDir::new().unwrap();
    let path = d.path().join("game.gvlib");
    let mut lib = Library::create(&path, "Test").unwrap();
    lib.create_character("Vampire", "Unchanged", 0, Role::Storyteller)
        .unwrap();
    let bad = d.path().join("bad.gvlib");
    fs::write(&bad, b"bad backup").unwrap();
    assert!(lib.restore(&bad, 1, Role::Storyteller).is_err());
    assert_eq!(lib.revision().unwrap(), 1);
    let future = d.path().join("future.gvlib");
    lib.backup(&future, Role::Storyteller).unwrap();
    let conn = rusqlite::Connection::open(&future).unwrap();
    conn.pragma_update(None, "user_version", 999).unwrap();
    drop(conn);
    let bytes = fs::read(&future).unwrap();
    assert_eq!(
        Library::open(&future).err().unwrap().code,
        "UNSUPPORTED_SCHEMA"
    );
    assert_eq!(fs::read(&future).unwrap(), bytes);
    let partial = d.path().join("partial.gex");
    fs::write(&partial, b"\x04\0GVBE").unwrap();
    assert_eq!(
        lib.preview_import(&partial, "windows-1252", Role::Storyteller)
            .unwrap_err()
            .code,
        "INCOMPLETE_LEGACY_READ"
    );
    assert_eq!(lib.summaries(Role::Storyteller).unwrap().len(), 1);
    let other = d.path().join("other.gvlib");
    Library::create(&other, "Other").unwrap();
    assert_eq!(
        lib.restore(&other, 1, Role::Storyteller).unwrap_err().code,
        "WRONG_LIBRARY"
    );
}

#[test]
fn diagnostics_capture_failure_without_record_contents() {
    let d = TempDir::new().unwrap();
    diagnostics::write(d.path(), "save", "STORAGE_ERROR", true).unwrap();
    let text = fs::read_to_string(d.path().join("events.jsonl")).unwrap();
    let event: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(event["operation"], "save");
    assert_eq!(event["code"], "STORAGE_ERROR");
    assert!(event["backtrace"].is_string());
    assert!(!text.contains("ST_SECRET"));
}

#[test]
fn malformed_secret_markers_are_conservatively_redacted() {
    assert_eq!(
        importer::redact(&json!("visible [ST]hide forever")),
        "visible "
    );
    assert_eq!(importer::redact(&json!("ambiguous [/ST]")), "");
}

#[test]
fn custom_legacy_st_markers_are_honored_in_player_names_and_traits() {
    let d = TempDir::new().unwrap();
    let source = d.path().join("custom.gex");
    fs::write(&source,br#"<grapevine version="3" stcommentstart="{ST}" stcommentend="{/ST}"><vampire name="Visible{ST}HIDDEN_NAME{/ST}"><traitlist name="Disciplines"><trait name="Power{ST}HIDDEN_POWER{/ST}" val="3" note="HIDDEN_NOTE"/></traitlist></vampire></grapevine>"#).unwrap();
    let mut lib = Library::create(&d.path().join("custom.gvlib"), "Custom").unwrap();
    let p = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    lib.commit_import(&p, 0, false, Role::Storyteller).unwrap();
    let summaries = lib.summaries(Role::Player).unwrap();
    assert_eq!(summaries[0].name, "Visible");
    let view = lib.record(&summaries[0].id, Role::Player).unwrap();
    let text = serde_json::to_string(&view).unwrap();
    assert!(!text.contains("HIDDEN"));
}

#[test]
fn all_twelve_binary_character_types_survive_import_edit_and_reopen() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-game.gv3");
    let bytes = fs::read(&source).expect("retained synthetic fixture");
    let expected = importer::normalize(&bytes, "Synthetic Game.gv3", "windows-1252").unwrap();
    let d = TempDir::new().unwrap();
    let db = d.path().join("all-types.gvlib");
    let mut lib = Library::create(&db, "All types").unwrap();
    let preview = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    let result = lib
        .commit_import(&preview, 0, false, Role::Storyteller)
        .unwrap();
    assert_eq!(result.inserted, 22);
    let first = expected
        .records
        .iter()
        .find(|r| r.kind == "Vampire")
        .unwrap();
    lib.save_edit(
        &first.id,
        &Edit {
            name: "Edited Vampire".into(),
            status: "Draft".into(),
            notes: "Private app notes".into(),
        },
        1,
        Role::Storyteller,
    )
    .unwrap();
    drop(lib);
    let conn = rusqlite::Connection::open(&db).unwrap();
    for record in expected.records {
        let data: String = conn
            .query_row("SELECT data FROM records WHERE id=?1", [&record.id], |r| {
                r.get(0)
            })
            .unwrap();
        let got: Record = serde_json::from_str(&data).unwrap();
        assert_eq!(got.original, record.original);
        assert_eq!(
            got.private_fields.get("Experience"),
            record.private_fields.get("Experience")
        );
    }
    let lib = Library::open(&db).unwrap();
    assert_eq!(lib.summaries(Role::Storyteller).unwrap().len(), 22);
}

#[test]
fn real_exchange_files_when_explicitly_supplied() {
    let Some(source) = std::env::var_os("GRAPEVINE_LEGACY_TEST_SOURCE") else {
        eprintln!("Real-source comparison not requested; set GRAPEVINE_LEGACY_TEST_SOURCE.");
        return;
    };
    let root = Path::new(&source);
    let mut total = 0;
    for (name, count) in [
        ("New Game Items.gex", 47),
        ("Dark Ages Arsenal.gex", 33),
        ("Fetishes and Talens.gex", 43),
        ("Artifacts and Devices.gex", 34),
        ("Rotes.gex", 201),
    ] {
        let path = root.join(name);
        let bytes = fs::read(&path).unwrap();
        let expected = importer::normalize(&bytes, name, "windows-1252").unwrap();
        assert_eq!(expected.records.len(), count);
        let d = TempDir::new().unwrap();
        let db = d.path().join("real.gvlib");
        let mut lib = Library::create(&db, "Read-only source test").unwrap();
        let preview = lib
            .preview_import(&path, "windows-1252", Role::Storyteller)
            .unwrap();
        let imported = lib
            .commit_import(&preview, 0, false, Role::Storyteller)
            .unwrap();
        assert_eq!(imported.inserted, count);
        drop(lib);
        let conn = rusqlite::Connection::open(&db).unwrap();
        for original in expected.records {
            let data: String = conn
                .query_row(
                    "SELECT data FROM records WHERE id=?1",
                    [&original.id],
                    |r| r.get(0),
                )
                .unwrap();
            let stored: Record = serde_json::from_str(&data).unwrap();
            assert_eq!(stored.original, original.original);
            assert_eq!(stored.public_fields, original.public_fields);
            assert_eq!(stored.private_fields, original.private_fields);
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
        total += count;
    }
    assert_eq!(total, 358);
}

#[test]
fn edited_custom_markers_and_new_character_markers_remain_private() {
    let d = TempDir::new().unwrap();
    let source = d.path().join("custom.gex");
    fs::write(&source, br#"<grapevine version="3" stcommentstart="{ST}" stcommentend="{/ST}"><vampire name="Original" status="Draft"/></grapevine>"#).unwrap();
    let db = d.path().join("custom.gvlib");
    let mut lib = Library::create(&db, "Custom").unwrap();
    let p = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    let id = p.records[0].id.clone();
    lib.commit_import(&p, 0, false, Role::Storyteller).unwrap();
    lib.save_edit(
        &id,
        &Edit {
            name: "Visible{ST}EDIT_SECRET{/ST}".into(),
            status: "Active{ST}STATUS_SECRET{/ST}".into(),
            notes: "Private".into(),
        },
        1,
        Role::Storyteller,
    )
    .unwrap();
    let new_id = lib
        .create_character("Vampire", "New[ST]NEW_SECRET[/ST]", 2, Role::Storyteller)
        .unwrap();
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let player = lib.record(&id, Role::Player).unwrap();
    assert_eq!(player.name, "Visible");
    assert_eq!(player.status, "Active");
    assert!(
        !serde_json::to_string(&lib.summaries(Role::Player).unwrap())
            .unwrap()
            .contains("SECRET")
    );
    assert_eq!(lib.record(&new_id, Role::Player).unwrap().name, "New");
    assert!(
        lib.record(&id, Role::Storyteller)
            .unwrap()
            .name
            .contains("EDIT_SECRET")
    );
}

#[test]
fn legacy_date_values_survive_json_storage_without_one_ulp_drift() {
    // This bit pattern was exposed by real XP-history dates. No private record is a fixture.
    let value = f64::from_bits(0x40e4be903a375f32);
    let json = serde_json::json!({"ole_automation_days":value});
    let text = serde_json::to_string(&json).unwrap();
    let decoded: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        decoded["ole_automation_days"].as_f64().unwrap().to_bits(),
        value.to_bits()
    );

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-game.gv3");
    let mut bytes = fs::read(source).unwrap();
    let old = (-1.25_f64).to_le_bytes();
    let mut changed = 0;
    for offset in 0..bytes.len() - 7 {
        if bytes[offset..offset + 8] == old {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            changed += 1;
        }
    }
    assert!(changed > 1);
    let expected = importer::normalize(&bytes, "fractional-dates.gv3", "windows-1252").unwrap();
    let d = TempDir::new().unwrap();
    let source = d.path().join("fractional-dates.gv3");
    fs::write(&source, bytes).unwrap();
    let db = d.path().join("dates.gvlib");
    let mut lib = Library::create(&db, "Dates").unwrap();
    let p = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    lib.commit_import(&p, 0, false, Role::Storyteller).unwrap();
    drop(lib);
    let conn = rusqlite::Connection::open(db).unwrap();
    for record in expected.records {
        let text: String = conn
            .query_row("SELECT data FROM records WHERE id=?1", [&record.id], |r| {
                r.get(0)
            })
            .unwrap();
        let actual: Record = serde_json::from_str(&text).unwrap();
        assert_eq!(actual.original, record.original);
        assert_eq!(actual.private_fields, record.private_fields);
    }
}
