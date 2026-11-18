use grapevine_workbench::{
    associations::{Kind, Link},
    house_rules::{
        Attribute, ChallengeModifierRule, HouseRulesPack, ModifierTarget, NamedChallenge,
        SourceKind,
    },
    mobile_bundle::{Bundle, FORMAT},
    model::{BuildKind, Role},
    printing::DocLanguage,
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    sheet,
    store::Library,
    world::Edit,
};
use serde_json::json;

#[test]
fn export_is_player_safe_and_nightly_state_is_separate() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.gex");
    std::fs::write(&source, r#"<grapevine version="3" stcommentstart="{ST}" stcommentend="{/ST}"><player name="Owner" id="p1" email="SECRET_EMAIL"/><vampire name="Player Sheet{ST}SECRET_CUSTOM_NAME{/ST}" status="Active" player="Owner" notes="SECRET_CHARACTER_NOTE"></vampire></grapevine>"#).unwrap();
    let mut library = Library::create(&directory.path().join("test.gvlib"), "Synthetic").unwrap();
    let st = Role::Storyteller;
    let preview = library.preview_import(&source, "windows-1252", st).unwrap();
    let id = preview
        .records
        .iter()
        .find(|r| r.kind == "Vampire")
        .unwrap()
        .id
        .clone();
    library.commit_import(&preview, 0, false, st).unwrap();
    let mut edit = library.sheet_edit(&id, st).unwrap();
    for (key, rows, public) in [
        (
            "SocialList",
            json!([{"Name":"Charming","Value":"14","Note":"SECRET_TRAIT_NOTE"}]),
            true,
        ),
        (
            "AbilityList",
            json!([{"Name":"Subterfuge","Value":"5"}]),
            true,
        ),
        (
            "DisciplineList",
            json!([{"Name":"Feral Whispers","Value":"basic","AppSourcePowerId":ANIMALISM_FERAL_WHISPERS_ID,"AppTeacherEvidence":"SECRET_TEACHER"}]),
            true,
        ),
        (
            "MeritList",
            json!([{"Name":"SECRET_PRIVATE_MERIT","Value":"2"}]),
            false,
        ),
    ] {
        let mut list = sheet::empty_list(key);
        list["Traits"] = rows;
        edit.lists.insert(key.into(), list);
        edit.visibility.insert(key.into(), public);
    }
    for (key, value) in [("Willpower", 7), ("TempWillpower", 2), ("Blood", 11)] {
        edit.resources.insert(key.into(), value);
        edit.visibility.insert(key.into(), key != "Blood");
    }
    library
        .save_sheet(&id, &edit, library.revision().unwrap(), st)
        .unwrap();
    let item = library
        .create_world(
            "Item",
            &Edit {
                name: "SECRET_PRIVATE_ITEM".into(),
                fields: [
                    ("Bonus".into(), json!(9)),
                    ("Notes".into(), json!("SECRET_ITEM_NOTE")),
                ]
                .into(),
            },
            library.revision().unwrap(),
            st,
        )
        .unwrap();
    library
        .save_associations(
            &id,
            &[Link {
                target_id: item,
                kind: Kind::Item,
                label_at_binding: "SECRET_PRIVATE_ITEM".into(),
                quantity: 1,
                share_challenge_bonus: false,
                note: "SECRET_LINK_NOTE".into(),
            }],
            library.revision().unwrap(),
            st,
        )
        .unwrap();
    let mut pack = HouseRulesPack {
        name: "Public Rules{ST}SECRET_PACK_NAME{/ST}".into(),
        version: "1[ST]SECRET_VERSION[/ST]".into(),
        named_challenges: vec![NamedChallenge {
            name: "Social test".into(),
            attribute: Attribute::Social,
            ability: Some("Subterfuge".into()),
        }],
        custom_modifiers: vec![ChallengeModifierRule {
            id: "private-rule".into(),
            label: "{ST}SECRET_RULE_LABEL{/ST}".into(),
            source_kind: SourceKind::Custom,
            match_name: "Social test".into(),
            target: ModifierTarget::NamedChallenge("Social test".into()),
            ability_override: None,
            delta: 3,
        }],
        ..HouseRulesPack::default()
    };
    pack.challenge_formula.include_ability_in_trait_call = true;
    pack.challenge_formula.include_linked_item_bonuses = true;
    let mut profile = library.chronicle_profile(st).unwrap().profile;
    profile.house_rules_pack = Some(pack);
    profile.owbn_bylaws_enabled = true;
    profile.allow_non_binding_sources = true;
    profile.private_notes = "SECRET_COORDINATOR".into();
    library
        .save_chronicle_profile(&profile, library.revision().unwrap(), st)
        .unwrap();
    let revision = library.revision().unwrap();
    let connection = rusqlite::Connection::open(&library.path).unwrap();
    let saved = || -> String {
        connection
            .query_row("SELECT data FROM records WHERE id=?1", [&id], |row| {
                row.get(0)
            })
            .unwrap()
    };
    let before = saved();
    let path = directory.path().join("player.inconnu-sheet.json");
    library
        .write_mobile_sheet(&id, DocLanguage::English, revision, &path)
        .unwrap();
    assert!(
        library
            .write_mobile_sheet(&id, DocLanguage::English, revision, &path)
            .is_err()
    );
    let text = std::fs::read_to_string(path).unwrap();
    for hidden in [
        "SECRET",
        "[ST]",
        "{ST}",
        "original",
        "private_fields",
        "AppSourcePowerId",
        "AppTeacherEvidence",
    ] {
        assert!(!text.contains(hidden), "leaked {hidden}");
    }
    let mut bundle: Bundle = serde_json::from_str(&text).unwrap();
    assert_eq!(bundle.format, FORMAT);
    assert_eq!(bundle.character.name, "Player Sheet");
    assert_eq!(bundle.character.permanent_resources["Willpower"], 7);
    assert!(!bundle.character.permanent_resources.contains_key("Blood"));
    assert!(
        !bundle
            .character
            .permanent_resources
            .contains_key("TempWillpower")
    );
    assert_eq!(
        bundle.tonight_play_state.current_temp_resources["TempWillpower"],
        2
    );
    assert_eq!(bundle.tonight_play_state.temp_social_lost, 0);
    assert!(bundle.tonight_play_state.current_health_damage.is_empty());
    assert!(
        bundle.chronicle_rules.owbn_bylaws_enabled
            && bundle.chronicle_rules.allow_non_binding_sources
    );
    assert_eq!(
        bundle
            .chronicle_rules
            .house_rules_pack
            .as_ref()
            .unwrap()
            .name,
        "Public Rules"
    );
    assert_eq!(bundle.challenge_pools.last().unwrap().callable_total, 19);
    assert!(bundle.challenge_pools.last().unwrap().modifiers.is_empty());
    assert!(
        bundle
            .simple_mechanics
            .iter()
            .any(|section| section.title.contains("Feral Whispers"))
    );
    assert!(
        bundle
            .source_references
            .iter()
            .any(|reference| reference.power == "Feral Whispers")
    );
    bundle.tonight_play_state.temp_social_lost = 4;
    assert_eq!(bundle.challenge_pools.last().unwrap().callable_total, 19);
    assert_eq!(saved(), before);
    assert_eq!(library.revision().unwrap(), revision);
    assert!(
        library
            .mobile_sheet(&id, DocLanguage::English, revision - 1)
            .is_err()
    );
    let build = library
        .create_build(&id, BuildKind::Test, revision, st)
        .unwrap();
    assert!(
        library
            .mobile_sheet(&build, DocLanguage::English, library.revision().unwrap())
            .is_err()
    );
}
