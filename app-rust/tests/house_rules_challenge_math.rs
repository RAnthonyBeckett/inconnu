use grapevine_workbench::{
    associations::{Kind, Link},
    challenge_math::{LinkedItem, calculate},
    house_rules::{
        Attribute, ChallengeFormula, ChallengeModifierRule, HouseRulesPack, MAX_BYTES,
        ModifierTarget, NamedChallenge, SourceKind,
    },
    model::{RecordView, Role, SheetEdit},
    printing::{Options, PrintScope},
    store::Library,
    world::Edit,
};
use serde_json::{Value, json};

fn fixture() -> (HouseRulesPack, NamedChallenge) {
    let challenge = NamedChallenge {
        name: "Chimerstry".into(),
        attribute: Attribute::Social,
        ability: Some("Subterfuge".into()),
    };
    let pack = HouseRulesPack {
        name: "Synthetic Green Bay formula".into(),
        challenge_formula: ChallengeFormula {
            include_ability_in_trait_call: true,
            include_linked_item_bonuses: true,
            include_merit_flaw_modifiers: true,
            max_trait_bid_cap: None,
        },
        named_challenges: vec![challenge.clone()],
        custom_modifiers: [
            (
                "a",
                "Synthetic Merit rule",
                SourceKind::Merit,
                "Synthetic Merit",
                3,
            ),
            (
                "b",
                "Synthetic Flaw rule",
                SourceKind::Flaw,
                "Synthetic Flaw",
                -1,
            ),
            (
                "c",
                "Synthetic Power rule",
                SourceKind::Power,
                "Chimerstry",
                7,
            ),
            (
                "d",
                "Synthetic Item rule",
                SourceKind::Item,
                "Public Mask",
                2,
            ),
        ]
        .into_iter()
        .map(
            |(id, label, source_kind, match_name, delta)| ChallengeModifierRule {
                id: id.into(),
                label: label.into(),
                source_kind,
                match_name: match_name.into(),
                target: ModifierTarget::NamedChallenge(challenge.name.clone()),
                delta,
                ability_override: None,
            },
        )
        .collect(),
        ..HouseRulesPack::default()
    };
    (pack, challenge)
}

fn list(key: &str, traits: Value) -> Value {
    let mut list = grapevine_workbench::sheet::empty_list(key);
    list["Traits"] = traits;
    list
}

fn view() -> RecordView {
    RecordView {
        build: None,
        id: "synthetic".into(),
        kind: "Vampire".into(),
        name: "Synthetic".into(),
        status: String::new(),
        player_name: String::new(),
        notes: String::new(),
        fields: [
            (
                "SocialList".into(),
                list("SocialList", json!([{"Name":"Charming","Value":"14"}])),
            ),
            (
                "AbilityList".into(),
                list("AbilityList", json!([{"Name":"Subterfuge","Value":"5"}])),
            ),
            (
                "MeritList".into(),
                list("MeritList", json!([{"Name":"Synthetic Merit","Value":"1"}])),
            ),
            (
                "FlawList".into(),
                list("FlawList", json!([{"Name":"Synthetic Flaw","Value":"1"}])),
            ),
            (
                "DisciplineList".into(),
                list(
                    "DisciplineList",
                    json!([{"Name":"Chimerstry","Value":"basic"}]),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    }
}

#[test]
fn green_bay_formula_lists_every_component_and_applies_the_lower_cap() {
    let (mut pack, challenge) = fixture();
    let view = view();
    let items = [LinkedItem {
        name: "Public Mask".into(),
        bonus: Some(5),
    }];
    let pool = calculate(&view, &items, Some(&pack), true, &challenge).unwrap();
    assert_eq!(
        (
            pool.attribute_traits,
            pool.ability_traits,
            pool.uncapped_total,
            pool.effective_cap,
            pool.callable_total
        ),
        (14, 5, 35, Some(30), 30)
    );
    assert_eq!(
        pool.modifiers
            .iter()
            .map(|(_, delta)| *delta)
            .collect::<Vec<_>>(),
        [3, -1, 7, 2, 5]
    );
    pack.challenge_formula.include_ability_in_trait_call = false;
    pack.challenge_formula.max_trait_bid_cap = Some(26);
    let pool = calculate(&view, &items, Some(&pack), false, &challenge).unwrap();
    assert_eq!(
        (
            pool.ability_traits,
            pool.uncapped_total,
            pool.effective_cap,
            pool.callable_total
        ),
        (0, 30, Some(26), 26)
    );
    assert_eq!(
        calculate(&view, &items, Some(&pack), true, &challenge)
            .unwrap()
            .effective_cap,
        Some(26)
    );
    let pool = calculate(&view, &items, None, false, &challenge).unwrap();
    assert_eq!(
        (
            pool.attribute_traits,
            pool.ability_traits,
            pool.callable_total
        ),
        (14, 0, 14)
    );
}

#[test]
fn pack_json_is_bounded_versioned_and_requires_local_power_authority() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("house-rules.json");
    let (pack, _) = fixture();
    pack.save_new(&path).unwrap();
    assert_eq!(HouseRulesPack::load(&path).unwrap(), pack);
    assert!(pack.save_new(&path).is_err());
    assert!(HouseRulesPack::from_json(&vec![b' '; MAX_BYTES + 1]).is_err());
    let mut invalid = serde_json::to_value(&pack).unwrap();
    invalid["challenge_formula"]["max_trait_bid_cap"] = json!(0);
    assert!(HouseRulesPack::from_json(&serde_json::to_vec(&invalid).unwrap()).is_err());
    invalid["challenge_formula"]["max_trait_bid_cap"] = json!(30);
    invalid["unexpected"] = json!(true);
    assert!(HouseRulesPack::from_json(&serde_json::to_vec(&invalid).unwrap()).is_err());
    let mut power = grapevine_workbench::power_catalogue::builtin().entries[0].clone();
    power.stable_id = "local.synthetic-power".into();
    power.legacy_power = None;
    power.legacy_preview = None;
    power.source_reviewed = false;
    let mut pack = pack;
    pack.custom_powers.push(power);
    assert!(pack.validate().is_err());
    pack.custom_powers[0].packet_authority =
        grapevine_workbench::owbn_bylaws::PacketAuthority::CustomHouseRule;
    pack.validate().unwrap();
}

#[test]
fn stored_pack_survives_restore_and_player_print_omits_private_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let mut library = Library::create(&directory.path().join("test.gvlib"), "Synthetic").unwrap();
    let st = Role::Storyteller;
    let player = Role::Player;
    let id = library
        .create_character("Vampire", "Synthetic", 0, st)
        .unwrap();
    let mut edit = SheetEdit::default();
    for (key, list) in view().fields {
        edit.visibility.insert(key.clone(), key != "MeritList");
        edit.lists.insert(key, list);
    }
    edit.lists.get_mut("SocialList").unwrap()["Traits"][0]["Note"] =
        json!("[ST]private-trait-token[/ST]");
    library.save_sheet(&id, &edit, 1, st).unwrap();
    let mut links = Vec::new();
    for (name, bonus, share) in [("Public Mask", 5, true), ("secret-item-token", 9, false)] {
        let item = library
            .create_world(
                "Item",
                &Edit {
                    name: name.into(),
                    fields: [
                        ("Bonus".into(), json!(bonus)),
                        ("Notes".into(), json!("secret-item-note")),
                    ]
                    .into(),
                },
                library.revision().unwrap(),
                st,
            )
            .unwrap();
        links.push(Link {
            target_id: item,
            kind: Kind::Item,
            label_at_binding: name.into(),
            quantity: 2,
            share_challenge_bonus: share,
            note: "secret-assignment-note".into(),
        });
    }
    library
        .save_associations(&id, &links, library.revision().unwrap(), st)
        .unwrap();
    let (pack, _) = fixture();
    let mut profile = library.chronicle_profile(st).unwrap().profile;
    profile.house_rules_pack = Some(pack.clone());
    library
        .save_chronicle_profile(&profile, library.revision().unwrap(), st)
        .unwrap();
    let backup = directory.path().join("backup.gvlib");
    library.backup(&backup, st).unwrap();
    profile.house_rules_pack = None;
    library
        .save_chronicle_profile(&profile, library.revision().unwrap(), st)
        .unwrap();
    library
        .restore(&backup, library.revision().unwrap(), st)
        .unwrap();
    assert_eq!(
        library
            .chronicle_profile(st)
            .unwrap()
            .profile
            .house_rules_pack,
        Some(pack.clone())
    );
    let public = library.public_chronicle_profile().unwrap();
    assert_eq!(public.profile.house_rules_pack, Some(pack.summary()));
    assert!(
        !serde_json::to_string(&public)
            .unwrap()
            .contains("Synthetic Merit rule")
    );
    let mut options = Options {
        include_challenge_math: true,
        ..Options::default()
    };
    let public = library
        .print_document(&id, &options, library.revision().unwrap(), player)
        .unwrap();
    let pools = &public.challenge_pools;
    assert_eq!(pools.len(), 4);
    assert_eq!(pools[3].uncapped_total, 32);
    let text = serde_json::to_string(&public).unwrap();
    for hidden in [
        "secret-item-token",
        "secret-item-note",
        "secret-assignment-note",
        "private-trait-token",
        "Synthetic Merit rule",
    ] {
        assert!(!text.contains(hidden), "leaked {hidden}");
    }
    options.scope = PrintScope::PrivateStoryteller;
    assert!(
        library
            .print_document(&id, &options, library.revision().unwrap(), player)
            .is_err()
    );
    let private = library
        .print_document(&id, &options, library.revision().unwrap(), st)
        .unwrap();
    assert_eq!(private.challenge_pools[3].uncapped_total, 44);
    assert!(
        private.challenge_pools[3]
            .modifiers
            .iter()
            .any(|(name, _)| name == "secret-item-token")
    );
}
