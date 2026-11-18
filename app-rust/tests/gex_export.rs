use grapevine_workbench::{
    chronicle,
    gex_export::{self, Options, RisenLegacyExportMode, Target},
    importer,
    model::{Record, Role},
    printing::{DocLanguage, PrintScope},
    sheet,
    store::Library,
};
use serde_json::json;
use tempfile::tempdir;

fn record(kind: &str) -> Record {
    Record {
        id: "internal-id".into(), kind: kind.into(), name: "Érica “Azul” — ação & <teste>".into(),
        player_id: None, legacy_id: Some("legacy-123".into()),
        public_fields: json!({"Nature":"Curiosa", "Willpower":5, "PhysicalList":{"Name":"Physical","Traits":[{"Name":"Ágil","Value":"3","Note":"SECRET_TRAIT","AppReceipt":"SECRET_ROW_RECEIPT"}],"Alphabetized":true,"Atomic":false,"Negative":false,"Display":2},"DisciplineList":{"Name":"Disciplines","Traits":[{"Name":"Olhar[ST]SECRET_STANDARD[/ST] {S}SECRET_CUSTOM{/S}","Value":"Basic"}]},"EMail":"SECRET_CONTACT","Notes":"SECRET_PUBLIC_NOTE","AppAudit":"SECRET_AUDIT"}).as_object().unwrap().clone(),
        private_fields: json!({"Notes":"PRIVATE_NOTE", "AppAudit":"SECRET_PRIVATE_AUDIT","Clan":"PRIVATE_CLAN"}).as_object().unwrap().clone(),
        original: json!({"raw_xml":"SECRET_ORIGINAL"}), source_sha256:"SECRET_SOURCE".into(),
        st_markers:Some(("{S}".into(),"{/S}".into())), build:None, xp_ledger:None,
    }
}
fn roundtrip(xml: &str) -> Vec<Record> {
    let inspected = grapevine_legacy_reader::inspect(xml.as_bytes(), "windows-1252");
    assert!(inspected.succeeded(), "{:?}", inspected.diagnostics);
    importer::normalize(xml.as_bytes(), "synthetic.gex", "windows-1252")
        .unwrap()
        .records
}
#[test]
fn public_utf8_typography_lists_privacy_and_escaping_roundtrip() {
    let r = record("Vampire");
    let before = serde_json::to_value(&r).unwrap();
    let doc = gex_export::serialize(
        std::slice::from_ref(&r),
        None,
        &Options::default(),
        Role::Player,
    )
    .unwrap();
    assert!(!doc.xml.contains("SECRET"));
    assert!(!doc.xml.contains("PRIVATE"));
    assert!(
        doc.xml
            .contains("Érica &quot;Azul&quot; - ação &amp; &lt;teste&gt;")
    );
    assert!(!doc.xml.contains("internal-id"));
    assert!(doc.xml.contains("legacy-123"));
    let imported = roundtrip(&doc.xml);
    let sheet = sheet::extract(&imported[0]);
    assert_eq!(imported[0].name, "Érica \"Azul\" - ação & <teste>");
    assert_eq!(sheet.lists["PhysicalList"]["Traits"][0]["Value"], "3");
    assert_eq!(sheet.lists["PhysicalList"]["Display"], 2);
    assert_eq!(sheet.lists["PhysicalList"]["Alphabetized"], true);
    assert_eq!(sheet.resources["Willpower"], 5);
    assert_eq!(serde_json::to_value(r).unwrap(), before);
}
#[test]
fn typography_can_be_disabled_but_invalid_controls_are_always_removed() {
    let mut r = record("Vampire");
    r.name = "João “noite”\u{1}—\tlinha\n次".into();
    let opts = Options {
        normalize_typography: false,
        ..Options::default()
    };
    let doc = gex_export::serialize(&[r], None, &opts, Role::Player).unwrap();
    assert!(doc.xml.contains('“'));
    assert!(doc.xml.contains('—'));
    assert!(doc.xml.contains('次'));
    assert!(doc.warnings.iter().any(|w| w.contains("control")));
    assert_eq!(roundtrip(&doc.xml)[0].name, "João “noite”—\tlinha\n次");
}
#[test]
fn private_scope_is_authorized_and_reimported_custom_markers_remain_secret() {
    let mut r = record("Vampire");
    r.name = "Nome {s}CUSTOM_SECRET{/s}".into();
    let opts = Options {
        scope: PrintScope::PrivateStoryteller,
        ..Options::default()
    };
    assert_eq!(
        gex_export::serialize(std::slice::from_ref(&r), None, &opts, Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    let private = gex_export::serialize(&[r], None, &opts, Role::Storyteller).unwrap();
    assert!(private.xml.contains("PRIVATE_NOTE"));
    assert!(private.xml.contains("SECRET_TRAIT"));
    assert!(!private.xml.contains("SECRET_ORIGINAL"));
    assert!(!private.xml.contains("SECRET_PRIVATE_AUDIT"));
    assert!(private.xml.contains("[ST]CUSTOM_SECRET[/ST]"));
    let public = gex_export::serialize(
        &roundtrip(&private.xml),
        None,
        &Options::default(),
        Role::Player,
    )
    .unwrap();
    assert!(!public.xml.contains("SECRET"));
    assert!(!public.xml.contains("PRIVATE"));
}
#[test]
fn risen_modes_are_explicit_and_native_ethnos_preserves_risen_identity() {
    let mut r = record("Risen");
    r.public_fields.insert(
        "ArcanoiList".into(),
        json!({"Name":"Arcanoi","Traits":[{"Name":"Argos","Value":"2"}]}),
    );
    let blocked = gex_export::serialize(
        std::slice::from_ref(&r),
        None,
        &Options::default(),
        Role::Storyteller,
    )
    .unwrap_err();
    assert_eq!(blocked.code, "GEX_RISEN_MODE_REQUIRED");
    assert!(blocked.message.contains("Escolha"));
    for (mode, tag, kind) in [
        (RisenLegacyExportMode::ExportAsWraith, "wraith", "Risen"),
        (RisenLegacyExportMode::ExportAsVarious, "various", "Various"),
    ] {
        let opts = Options {
            risen: mode,
            ..Options::default()
        };
        let doc = gex_export::serialize(std::slice::from_ref(&r), None, &opts, Role::Storyteller)
            .unwrap();
        assert!(doc.xml.contains(&format!("<{tag}")));
        assert!(!doc.xml.contains("<risen"));
        assert!(doc.warnings.iter().any(|w| w.contains("no native Risen")));
        let records = roundtrip(&doc.xml);
        assert_eq!(records[0].kind, kind);
        let imported = sheet::extract(&records[0]);
        assert!(imported.lists.contains_key("ArcanoiList"));
        assert!(imported.lists.contains_key("DisciplineList"));
    }
}
#[test]
fn every_legacy_kind_and_primary_secondary_list_roundtrips_without_invented_tags() {
    for profile in &grapevine_workbench::creature_profiles::builtin().profiles {
        if profile.kind == "Risen" {
            continue;
        }
        let mut r = record(&profile.kind);
        r.public_fields.clear();
        for key in [&profile.primary_power_list, &profile.secondary_power_list] {
            let mut list = sheet::empty_list(key);
            list["Traits"] = json!([{"Name":"Synthetic","Value":"2x3"}]);
            r.public_fields.insert(key.clone(), list);
        }
        let doc = gex_export::serialize(&[r], None, &Options::default(), Role::Player).unwrap();
        let imported = roundtrip(&doc.xml);
        assert_eq!(imported[0].kind, profile.kind);
        let sheet = sheet::extract(&imported[0]);
        for key in [&profile.primary_power_list, &profile.secondary_power_list] {
            assert_eq!(
                sheet
                    .lists
                    .get(key)
                    .and_then(|l| l["Traits"][0]["Value"].as_str()),
                Some("2x3"),
                "{} {key}",
                profile.kind
            );
        }
    }
}
#[test]
fn chronicle_public_omits_contacts_private_records_and_private_profile_details() {
    let mut player = record("Player");
    player.name = "PRIVATE_PLAYER".into();
    let profile = chronicle::Profile {
        title: "Crônica [ST]SECRET_TITLE[/ST]".into(),
        email: "SECRET_EMAIL".into(),
        phone: "SECRET_PHONE".into(),
        private_notes: "SECRET_PROFILE".into(),
        description: "SECRET_DESCRIPTION".into(),
        ..Default::default()
    };
    let doc = gex_export::serialize(
        &[record("Vampire"), player, record("Plot")],
        Some(&profile),
        &Options::default(),
        Role::Storyteller,
    )
    .unwrap();
    assert_eq!(doc.record_count, 1);
    assert!(!doc.xml.contains("SECRET"));
    assert!(!doc.xml.contains("PRIVATE"));
    assert!(doc.xml.contains("chronicle=\"Crônica \""));
    assert_eq!(roundtrip(&doc.xml).len(), 1);
}
#[test]
fn ambiguity_controls_bounded_lists_and_root_attribute_correction() {
    let mut r = record("Vampire");
    r.name = "Prefix [S{S}hidden{/S}T]SECRET[/ST]".into();
    r.public_fields.insert("Aura".into(), json!("Pale"));
    r.public_fields.insert("AuraBonus".into(), json!("+2"));
    let doc = gex_export::serialize(
        std::slice::from_ref(&r),
        None,
        &Options::default(),
        Role::Player,
    )
    .unwrap();
    assert!(!doc.xml.contains("SECRET"));
    assert!(doc.xml.contains("aurabonus=\"+2\""));
    roundtrip(&doc.xml);
    r.public_fields["PhysicalList"]["Traits"] = json!(vec![json!({"Name":"X","Value":"1"}); 4097]);
    assert!(gex_export::serialize(&[r], None, &Options::default(), Role::Player).is_err());
}
#[test]
fn normalization_cannot_create_public_markers_or_change_identifiers() {
    for name in ["[S\u{0}T]SECRET[/S\u{0}T]", "{\u{1}S}SECRET{/\u{1}S}"] {
        let mut r = record("Vampire");
        r.name = name.into();
        r.legacy_id = Some("legacy—“Á”".into());
        let doc = gex_export::serialize(&[r], None, &Options::default(), Role::Player).unwrap();
        assert!(!doc.xml.contains("SECRET"));
        assert!(!doc.xml.contains("[ST]"));
        assert_eq!(
            roundtrip(&doc.xml)[0].legacy_id.as_deref(),
            Some("legacy—“Á”")
        );
    }
    assert!(importer::normalize(br#"<grapevine version="3"><vampire name="X" inconnuprivate="broken" clan="secret"/></grapevine>"#,"x.gex","utf-8").is_err());
}
#[test]
fn store_preview_and_write_bind_revision_role_and_safe_destination() {
    let dir = tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("synthetic.gvlib"), "Synthetic").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
        .unwrap();
    let rev = lib.revision().unwrap();
    let opts = Options::default();
    let preview = lib
        .gex_preview(Target::Character, Some(&id), &opts, rev, Role::Player)
        .unwrap();
    let destination = dir.path().join("transfer.gex");
    lib.write_gex(&preview, &destination, Role::Player).unwrap();
    assert_eq!(lib.revision().unwrap(), rev);
    assert!(lib.write_gex(&preview, &destination, Role::Player).is_err());
    assert_eq!(
        lib.gex_preview(Target::Chronicle, None, &opts, rev, Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    let private = Options {
        scope: PrintScope::PrivateStoryteller,
        language: DocLanguage::Portuguese,
        ..opts
    };
    let private_preview = lib
        .gex_preview(
            Target::Character,
            Some(&id),
            &private,
            rev,
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(
        lib.write_gex(
            &private_preview,
            &dir.path().join("private.gex"),
            Role::Player
        )
        .unwrap_err()
        .code,
        "ACCESS_DENIED"
    );
    lib.create_character("Wraith", "Changed", rev, Role::Storyteller)
        .unwrap();
    assert_eq!(
        lib.write_gex(&preview, &dir.path().join("stale.gex"), Role::Player)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert!(!dir.path().join("stale.gex").exists());
    let roster = lib
        .gex_preview(
            Target::Roster,
            None,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(roster.document.record_count, 2);
}
#[test]
fn private_nested_story_content_is_projected_without_unknown_payloads() {
    let xml = r#"<grapevine version="3"><action character="Mira" date="2026-11-17"><subaction name="Influence" level="2"><act>Investigate &amp; report</act><results>Found it</results><linklist><plot date="2026-11-17" name="Case" item="0"/></linklist><unreviewed>DROP_UNKNOWN</unreviewed></subaction></action><plot name="Case"><outline>Outline</outline><development date="2026-11-17"><event>Discovery</event></development></plot><rumor title="Whispers" date="2026-11-17"><subrumor level="1"><body>Rumor body</body></subrumor></rumor><query name="Search" table="Vampires"><clause field="Name" comp="contains" find="Mira"/></query></grapevine>"#;
    let records = roundtrip(xml);
    let options = Options {
        scope: PrintScope::PrivateStoryteller,
        ..Default::default()
    };
    let doc = gex_export::serialize(&records, None, &options, Role::Storyteller).unwrap();
    for kept in [
        "Investigate &amp; report",
        "Found it",
        "Discovery",
        "Rumor body",
        "<clause",
        "<linklist",
    ] {
        assert!(doc.xml.contains(kept), "{kept}");
    }
    assert!(!doc.xml.contains("DROP_UNKNOWN"));
    assert!(doc.warnings.iter().any(|w| w.contains("omitted")));
    assert_eq!(roundtrip(&doc.xml).len(), 4);
}
#[test]
fn typed_story_edits_export_instead_of_stale_retained_originals() {
    let mut action = record("Action");
    action.public_fields.clear();
    action.private_fields.clear();
    action.private_fields.insert("AppActionV1".into(),json!({"character":"Mira","day":"2026-11-17","slots":[{"name":"Contact","level":"1","unused":"0","total":"1","growth":"0","action":"Current action","result":"Current result"}]}));
    let options = Options {
        scope: PrintScope::PrivateStoryteller,
        ..Default::default()
    };
    let doc = gex_export::serialize(&[action], None, &options, Role::Storyteller).unwrap();
    assert!(doc.xml.contains("Current action"));
    assert!(doc.xml.contains("Current result"));
    assert!(!doc.xml.contains("SECRET_ORIGINAL"));
    roundtrip(&doc.xml);
}
#[test]
fn incoming_legacy_single_traits_and_additive_visibility_are_conservative() {
    let old = roundtrip(
        r#"<grapevine version="3"><vampire name="Legacy"><traitlist name="Influences"><trait name="Resources"/></traitlist></vampire></grapevine>"#,
    );
    assert!(old[0].private_fields.contains_key("UnmappedTraitLists"));
    assert_eq!(
        sheet::extract(&old[0]).lists["InfluenceList"]["Traits"][0]["Value"],
        "1"
    );
    let mut r = record("Vampire");
    r.public_fields.insert(
        "RitualList".into(),
        json!({"Name":"Rituals","Traits":[{"Name":"Synthetic","Value":"1"}]}),
    );
    let doc = gex_export::serialize(&[r], None, &Options::default(), Role::Player).unwrap();
    let imported = roundtrip(&doc.xml);
    assert!(imported[0].public_fields.contains_key("RitualList"));
    let again = gex_export::serialize(&imported, None, &Options::default(), Role::Player).unwrap();
    assert!(again.xml.contains("Synthetic"));
    let malicious = r#"<grapevine version="3"><vampire name="X" inconnupublic="[&quot;Notes&quot;]" notes="PRIVATE_NOTE"/></grapevine>"#;
    assert!(!roundtrip(malicious)[0].public_fields.contains_key("Notes"));
}
