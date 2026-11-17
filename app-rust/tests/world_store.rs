use grapevine_workbench::{model::Role, store::Library, world::Edit};
use serde_json::{Value, json};
use tempfile::TempDir;

fn edit(name: &str, fields: Value) -> Edit {
    Edit {
        name: name.into(),
        fields: serde_json::from_value(fields).unwrap(),
    }
}

#[test]
fn world_records_create_edit_reopen_and_reject_stale_writes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("world.gvlib");
    let mut lib = Library::create(&path, "Synthetic world records").unwrap();
    let fields = json!({
        "ItemType": "Relic",
        "DamageAmount": 3,
        "AbilityList": {
            "Name": "Abilities",
            "Traits": [{"Name":"Investigation","Value":"2","custom_row":"retained"}],
            "custom_list": {"source":"synthetic"}
        }
    });
    let id = lib
        .create_world("Item", &edit("  Brass Key  ", fields), 0, Role::Storyteller)
        .unwrap();
    assert_eq!(lib.revision().unwrap(), 1);
    let mut loaded = lib.world_edit(&id, Role::Storyteller).unwrap();
    assert_eq!(loaded.name, "Brass Key");
    assert_eq!(
        loaded.fields["AbilityList"]["Traits"][0]["custom_row"],
        "retained"
    );
    assert_eq!(
        loaded.fields["AbilityList"]["custom_list"]["source"],
        "synthetic"
    );
    loaded.name = "Brass Key, restored".into();
    loaded.fields.insert("DamageAmount".into(), json!(4));
    assert_eq!(
        lib.save_world(&id, &loaded, 1, Role::Storyteller).unwrap(),
        2
    );
    assert_eq!(
        lib.save_world(&id, &loaded, 1, Role::Storyteller)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert_eq!(lib.revision().unwrap(), 2);
    drop(lib);

    let reopened = Library::open(&path).unwrap();
    let saved = reopened.world_edit(&id, Role::Storyteller).unwrap();
    assert_eq!(saved.name, "Brass Key, restored");
    assert_eq!(saved.fields["DamageAmount"], 4);
    assert_eq!(
        saved.fields["AbilityList"]["Traits"][0]["custom_row"],
        "retained"
    );
}

#[test]
fn world_edits_validate_fields_kinds_and_storyteller_access() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("world-validate.gvlib");
    let mut lib = Library::create(&path, "Synthetic validation").unwrap();
    let before = lib.revision().unwrap();
    let invalid = [
        ("Item", edit("bad", json!({"DamageAmount": 32768}))),
        ("Item", edit("bad", json!({"DamageAmount": "3"}))),
        ("Item", edit("bad", json!({"UnknownField": "value"}))),
        (
            "Location",
            edit("bad", json!({"LinkList": {"Traits": "wrong"}})),
        ),
        ("Rote", edit("bad", json!({"Level": 1.5}))),
        ("Item", edit("   ", json!({}))),
        ("Item", edit(&"x".repeat(4097), json!({}))),
    ];
    for (kind, draft) in invalid {
        assert!(
            lib.create_world(kind, &draft, before, Role::Storyteller)
                .is_err()
        );
    }
    assert!(
        lib.create_world(
            "Vampire",
            &edit("bad", json!({})),
            before,
            Role::Storyteller
        )
        .is_err()
    );
    assert_eq!(lib.revision().unwrap(), before);

    let id = lib
        .create_world(
            "Location",
            &edit("Local place", json!({"Owner":"free text"})),
            before,
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(
        lib.world_edit(&id, Role::Player).unwrap_err().code,
        "ACCESS_DENIED"
    );
    let player = lib
        .create_player(
            "Synthetic player",
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    let player_rev = lib.revision().unwrap();
    assert_eq!(
        lib.create_world(
            "Rote",
            &edit("Private", json!({})),
            player_rev,
            Role::Player
        )
        .unwrap_err()
        .code,
        "ACCESS_DENIED"
    );
    let draft = lib.world_edit(&id, Role::Storyteller).unwrap();
    assert_eq!(
        lib.save_world(&id, &draft, player_rev, Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert_eq!(lib.revision().unwrap(), player_rev);
    let character = lib
        .create_character(
            "Vampire",
            "Not a world record",
            player_rev,
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(
        lib.world_edit(&character, Role::Storyteller)
            .unwrap_err()
            .code,
        "WORLD_KIND_REQUIRED"
    );
    assert_eq!(
        lib.save_world(
            &character,
            &edit("Wrong kind", json!({})),
            lib.revision().unwrap(),
            Role::Storyteller
        )
        .unwrap_err()
        .code,
        "WORLD_KIND_REQUIRED"
    );
    assert!(!player.is_empty());
}

#[test]
fn rename_preserves_malformed_and_unmodeled_imported_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("world-preserve.gvlib");
    let mut lib = Library::create(&path, "Synthetic preservation").unwrap();
    let id = lib
        .create_world(
            "Location",
            &edit("Old name", json!({})),
            0,
            Role::Storyteller,
        )
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let raw: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |row| {
            row.get(0)
        })
        .unwrap();
    let mut record: Value = serde_json::from_str(&raw).unwrap();
    record["legacy_id"] = json!("legacy-location-9");
    record["source_sha256"] = json!("source-hash-preserved");
    record["st_markers"] = json!(["[st]", "[/st]"]);
    record["xp_ledger"] = json!({
        "baseline":{"day":"2026-10-01","earned":"4","unspent":"2","actor":"legacy","reason":"legacy baseline","evidence":"source"},
        "entries":[]
    });
    record["original"] = json!({"unmodeled":{"source":"keep"},"raw_xml":"opaque"});
    record["original"]["build_metadata"] = json!({"source_id":"source-build","source_revision":7});
    record["private_fields"]["Access"] = json!(["malformed imported shape"]);
    record["private_fields"]["Owner"] = json!("A free-text owner");
    record["private_fields"]["LastModified"] = json!("1999-01-02");
    record["private_fields"]["UnmodeledImportedField"] = json!({"nested":"keep"});
    record["private_fields"]["LinkList"] = json!({
        "Name":"Connections",
        "Traits":[{"Name":"Door","Value":"secret detail","unknown_row_key":"row survives"}],
        "unknown_list_key":{"value":"list survives"}
    });
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![serde_json::to_string(&record).unwrap(), id],
    )
    .unwrap();
    drop(conn);

    let mut draft = lib.world_edit(&id, Role::Storyteller).unwrap();
    assert!(
        !draft.fields.contains_key("Access"),
        "malformed known values stay out of the edit draft"
    );
    assert_eq!(
        draft.fields["LinkList"]["Traits"][0]["unknown_row_key"],
        "row survives"
    );
    draft.name = "Renamed location".into();
    let revision = lib.revision().unwrap();
    lib.save_world(&id, &draft, revision, Role::Storyteller)
        .unwrap();

    let conn = rusqlite::Connection::open(&path).unwrap();
    let saved: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |row| {
            row.get(0)
        })
        .unwrap();
    let saved: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved["name"], "Renamed location");
    assert_eq!(saved["legacy_id"], "legacy-location-9");
    assert_eq!(saved["source_sha256"], "source-hash-preserved");
    assert_eq!(saved["st_markers"], json!(["[st]", "[/st]"]));
    assert_eq!(
        saved["original"]["build_metadata"]["source_id"],
        "source-build"
    );
    assert_eq!(saved["xp_ledger"]["baseline"]["unspent"], "2");
    assert_eq!(saved["original"]["unmodeled"]["source"], "keep");
    assert_eq!(
        saved["private_fields"]["Access"],
        json!(["malformed imported shape"])
    );
    assert_eq!(saved["private_fields"]["LastModified"], "1999-01-02");
    assert_eq!(
        saved["private_fields"]["UnmodeledImportedField"]["nested"],
        "keep"
    );
    assert_eq!(
        saved["private_fields"]["LinkList"]["unknown_list_key"]["value"],
        "list survives"
    );
    assert_eq!(
        saved["private_fields"]["LinkList"]["Traits"][0]["unknown_row_key"],
        "row survives"
    );
}

#[test]
fn build_copies_are_read_only_for_world_editor() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("world-build.gvlib");
    let mut lib = Library::create(&path, "Synthetic build guard").unwrap();
    let id = lib
        .create_world("Rote", &edit("Source", json!({})), 0, Role::Storyteller)
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let raw: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut record: Value = serde_json::from_str(&raw).unwrap();
    record["build"] = json!({"source_id":"source","source_revision":1,"kind":"Draft"});
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![serde_json::to_string(&record).unwrap(), id],
    )
    .unwrap();
    drop(conn);
    assert_eq!(
        lib.world_edit(&id, Role::Storyteller).unwrap_err().code,
        "WORLD_BUILD_READ_ONLY"
    );
    let draft = edit("Edited", json!({}));
    assert_eq!(
        lib.save_world(&id, &draft, lib.revision().unwrap(), Role::Storyteller)
            .unwrap_err()
            .code,
        "WORLD_BUILD_READ_ONLY"
    );
}

#[test]
fn rote_projection_redacts_markers_and_notes_and_failed_audit_rolls_back() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("world-private.gvlib");
    let mut lib = Library::create(&path, "Synthetic privacy").unwrap();
    let id = lib.create_world("Rote", &edit("Public[st]DEFAULT_SECRET[/st]", json!({
        "SphereList":{"Name":"Spheres","Traits":[{"Name":"Mind[st]POWER_SECRET[/st]","Value":"2","Note":"ROW_SECRET","unknown":"HIDDEN_UNKNOWN"}]},
        "Description":"DESCRIPTION_SECRET"
    })), 0, Role::Storyteller).unwrap();
    let player = serde_json::to_string(&lib.record(&id, Role::Player).unwrap()).unwrap();
    assert!(!player.contains("SECRET") && !player.contains("HIDDEN_UNKNOWN"));
    let conn = rusqlite::Connection::open(&path).unwrap();
    let raw: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut record: Value = serde_json::from_str(&raw).unwrap();
    record["st_markers"] = json!(["<private>", "</private>"]);
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    let mut draft = lib.world_edit(&id, Role::Storyteller).unwrap();
    draft.name = "Visible<private>CUSTOM_SECRET</private>".into();
    lib.save_world(&id, &draft, 1, Role::Storyteller).unwrap();
    assert_eq!(lib.record(&id, Role::Player).unwrap().name, "Visible");
    assert_eq!(
        lib.world_edit(&id, Role::Storyteller).unwrap().name,
        draft.name
    );
    let backup = dir.path().join("world.gvbackup");
    lib.backup(&backup, Role::Storyteller).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_world_audit BEFORE INSERT ON audit WHEN NEW.operation='edit_world' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    let revision = lib.revision().unwrap();
    draft.name = "Must roll back".into();
    assert!(
        lib.save_world(&id, &draft, revision, Role::Storyteller)
            .is_err()
    );
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(lib.record(&id, Role::Player).unwrap().name, "Visible");
    conn.execute_batch("DROP TRIGGER fail_world_audit;")
        .unwrap();
    drop(conn);
    lib.save_world(&id, &draft, revision, Role::Storyteller)
        .unwrap();
    let revision = lib.revision().unwrap();
    lib.restore(&backup, revision, Role::Storyteller).unwrap();
    assert_eq!(lib.record(&id, Role::Player).unwrap().name, "Visible");
}

#[test]
fn imported_xml_world_edits_map_known_fields_and_keep_original_structures() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("xml-world.gvlib");
    let mut lib = Library::create(&path, "Synthetic XML world").unwrap();
    let bytes = br#"<?xml version="1.0"?><grapevine version="3">
      <item name="Relic" id="i1" type="Blade" subtype="Sword" level="2" bonus="3" damage="Lethal" amount="broken" conceal="Coat" lastmodified="2020-01-02" mystery="KEEP-ATTR">
        <traitlist name="Tempers" abc="no" atomic="yes" negative="no" display="2"><trait name="Sharp" val="2" note="ITEM-SECRET"/></traitlist>
        <traitlist name="Abilities" abc="yes"><trait name="Melee" val="x" note="ITEM-SECRET" extra="KEEP-ROW"/></traitlist>
        <traitlist name="Negatives"><trait name="Clumsy" val="1"/></traitlist>
        <traitlist name="Availability"><trait name="Market" val="Always"/></traitlist>
        <powers>Original effect</powers><appearance>Steel</appearance><notes>ITEM NOTE</notes>
        <traitlist name="Experimental"><trait name="Other" val="raw"/></traitlist><unknown>KEEP</unknown>
      </item>
      <location name="Hall" id="l1" type="Sanctum" level="2" owner="A label" access="Members" securitytraits="3" securityretests="4" gauntlet="4" affinity="Moon" totem="Owl" lastmodified="2020-01-02">
        <traitlist name="Links" abc="yes"><trait name="Library" val="1" note="LOCATION-SECRET"/></traitlist>
        <where>Old quarter</where><appearance>Stone tower</appearance><security>Wardens</security><umbra>Near Umbra</umbra><notes>LOCATION NOTE</notes>
      </location>
      <rote name="Example rote" id="r1" level="3" duration="One scene" lastmodified="2020-01-02">
        <description>Original description</description><grades>Novice</grades>
        <traitlist name="Spheres" abc="no"><trait name="Mind" val="2" note="ROTE-SECRET"/></traitlist>
      </rote>
    </grapevine>"#;
    let preview =
        grapevine_workbench::importer::normalize(bytes, "synthetic-world.gex", "windows-1252")
            .unwrap();
    let originals: Vec<_> = preview
        .records
        .iter()
        .map(|r| {
            (
                r.id.clone(),
                r.original.clone(),
                r.source_sha256.clone(),
                r.legacy_id.clone(),
            )
        })
        .collect();
    lib.commit_import(&preview, 0, false, Role::Storyteller)
        .unwrap();
    for (id, original, source_hash, legacy_id) in originals {
        let mut draft = lib.world_edit(&id, Role::Storyteller).unwrap();
        match draft.name.as_str() {
            "Relic" => {
                assert_eq!(draft.fields["ItemType"], "Blade");
                assert_eq!(draft.fields["ItemSubtype"], "Sword");
                assert_eq!(draft.fields["Level"], 2);
                assert_eq!(draft.fields["Bonus"], 3);
                assert_eq!(draft.fields["DamageType"], "Lethal");
                assert_eq!(draft.fields["Concealability"], "Coat");
                assert_eq!(draft.fields["Powers"], "Original effect");
                assert_eq!(draft.fields["Appearance"], "Steel");
                assert_eq!(draft.fields["Notes"], "ITEM NOTE");
                assert_eq!(draft.fields["TemperList"]["Alphabetized"], false);
                assert_eq!(draft.fields["AbilityList"]["Traits"][0]["Name"], "Melee");
                assert_eq!(draft.fields["AbilityList"]["Traits"][0]["Value"], "x");
                assert_eq!(draft.fields["NegativeList"]["Traits"][0]["Name"], "Clumsy");
                assert_eq!(draft.fields["Availability"]["Traits"][0]["Value"], "Always");
                assert!(!draft.fields.contains_key("DamageAmount"));
                draft.fields.insert("ItemSubtype".into(), json!("Knife"));
                draft.fields.get_mut("AbilityList").unwrap()["Traits"][0]["Value"] = json!("3");
            }
            "Hall" => {
                assert_eq!(draft.fields["LocType"], "Sanctum");
                assert_eq!(draft.fields["Level"], 2);
                assert_eq!(draft.fields["Owner"], "A label");
                assert_eq!(draft.fields["Access"], "Members");
                assert_eq!(draft.fields["SecTraits"], 3);
                assert_eq!(draft.fields["SecRetests"], 4);
                assert_eq!(draft.fields["Gauntlet"], 4);
                assert_eq!(draft.fields["Affinity"], "Moon");
                assert_eq!(draft.fields["Totem"], "Owl");
                assert_eq!(draft.fields["Where"], "Old quarter");
                assert_eq!(draft.fields["Appearance"], "Stone tower");
                assert_eq!(draft.fields["Security"], "Wardens");
                assert_eq!(draft.fields["Umbra"], "Near Umbra");
                assert_eq!(draft.fields["Notes"], "LOCATION NOTE");
                assert_eq!(draft.fields["LinkList"]["Traits"][0]["Name"], "Library");
            }
            "Example rote" => {
                assert_eq!(draft.fields["Level"], 3);
                assert_eq!(draft.fields["Duration"], "One scene");
                assert_eq!(draft.fields["Description"], "Original description");
                assert_eq!(draft.fields["Grades"], "Novice");
                assert_eq!(draft.fields["SphereList"]["Alphabetized"], false);
                assert_eq!(draft.fields["SphereList"]["Traits"][0]["Name"], "Mind");
            }
            other => panic!("unexpected world record {other}"),
        }
        draft.name.push_str(" edited");
        let revision = lib.revision().unwrap();
        lib.save_world(&id, &draft, revision, Role::Storyteller)
            .unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        let raw: String = conn
            .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
            .unwrap();
        let saved: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(saved["id"], id);
        assert_eq!(saved["original"], original);
        assert_eq!(saved["source_sha256"], source_hash);
        assert_eq!(saved["legacy_id"], serde_json::to_value(legacy_id).unwrap());
        assert_eq!(saved["player_id"], Value::Null);
        let source: Vec<u8> = conn
            .query_row("SELECT source_bytes FROM imports", [], |r| r.get(0))
            .unwrap();
        assert_eq!(source, bytes);
        if saved["kind"] == "Item" {
            assert_eq!(saved["private_fields"]["ItemSubtype"], "Knife");
            assert_eq!(saved["private_fields"]["DamageAmount"], "broken");
            assert_eq!(
                saved["private_fields"]["AbilityList"]["Traits"][0]["Value"],
                "3"
            );
            assert_eq!(
                saved["private_fields"]["UnmappedTraitLists"]["Name"],
                "Experimental"
            );
            assert_eq!(saved["private_fields"]["mystery"], "KEEP-ATTR");
            assert_eq!(saved["private_fields"]["unknown"], "KEEP");
            assert_eq!(saved["original"], original);
            let original_json = serde_json::to_string(&saved["original"]).unwrap();
            assert!(original_json.contains("KEEP-ROW"), "{original_json}");
        } else {
            assert_eq!(saved["original"], original);
        }
    }
    assert_eq!(lib.summaries(Role::Storyteller).unwrap().len(), 3);

    let character = grapevine_workbench::importer::normalize(
        br#"<grapevine version="3"><vampire name="Projection"><traitlist name="Physical"><trait name="Strength" val="3"/></traitlist></vampire></grapevine>"#,
        "synthetic-character.gex",
        "windows-1252",
    )
    .unwrap();
    assert_eq!(
        character.records[0].public_fields["PhysicalList"]["Traits"][0]["Name"],
        "Strength"
    );
}
