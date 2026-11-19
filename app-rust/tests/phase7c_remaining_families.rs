use grapevine_workbench::{
    cancellations::CancellationRequest,
    creature_profiles,
    model::Role,
    owbn_bylaws::{Regulation, RegulationEvidence},
    power_catalogue,
    pricing::{ClanRelation, PricingProfile},
    purchases::{PurchaseRequest, TeacherRule},
    sheet,
    store::Library,
    vampire_creation::Request,
    xp::Baseline,
};
use serde_json::{Value, json};

const ST: Role = Role::Storyteller;

fn list(request: &mut Request, key: &str, traits: Value) {
    let mut value = sheet::empty_list(key);
    value["Traits"] = traits;
    request.sheet.lists.insert(key.into(), value);
}

fn basic(request: &mut Request, key: &str, names: [&str; 3]) {
    list(
        request,
        key,
        json!(names.map(|name| json!({"Name": name, "Value": "basic"}))),
    );
}

fn creation(lib: &Library, id: &str, kind: &str) -> Request {
    let mut request = lib.vampire_creation_context(id, ST).unwrap().request;
    let profile = creature_profiles::profile(kind).unwrap();
    request.standard_pools = true;
    request.review_confirmed = true;
    request.actor = "Synthetic Storyteller".into();
    request.day = "2026-11-18".into();
    request.reason = "Synthetic starting review".into();
    request.policy_version = "fixture".into();
    request.review_reference = "Fixture creation review".into();
    request.manual_power_reference = "Fixture initial powers".into();
    request.current_resources_reference = "Fixture resources".into();
    let extra_attributes = if kind == "Mummy" { 3 } else { 1 };
    for (key, name, amount) in [
        (
            "PhysicalList",
            "Fixture Physical",
            profile.attribute_priorities[0] + extra_attributes,
        ),
        (
            "SocialList",
            "Fixture Social",
            profile.attribute_priorities[1],
        ),
        (
            "MentalList",
            "Fixture Mental",
            profile.attribute_priorities[2],
        ),
        ("AbilityList", "Fixture Ability", profile.ability_base),
        ("BackgroundList", "Rites", profile.background_base),
    ] {
        list(
            &mut request,
            key,
            json!([{"Name": name, "Value": amount.to_string()}]),
        );
    }
    let identity = |request: &mut Request, pairs: &[(&str, &str)]| {
        request
            .sheet
            .identity
            .extend(pairs.iter().map(|&(key, value)| (key.into(), value.into())));
    };
    match kind {
        "Fera" => {
            identity(
                &mut request,
                &[
                    ("Faction", "Gurahl"),
                    ("Breed", "Ursine"),
                    ("Auspice", "Arcas"),
                ],
            );
            basic(
                &mut request,
                "GiftList",
                ["Heightened Senses", "Open Seal", "Healing Tongue"],
            );
        }
        "KueiJin" => {
            request
                .sheet
                .resources
                .extend([("YinChi".into(), 2), ("YangChi".into(), 4)]);
            basic(
                &mut request,
                "DisciplineList",
                ["Black Wind", "Smoke without Fire", "Words of Smoke"],
            );
        }
        "Demon" => {
            identity(&mut request, &[("House", "Devils")]);
            request.sheet.resources.extend([
                ("Conscience".into(), 1),
                ("Conviction".into(), 2),
                ("Courage".into(), 3),
            ]);
            basic(
                &mut request,
                "LoreList",
                ["Manipulate Gravity", "Manipulate Adhesion", "Lamp of Faith"],
            );
        }
        "Mummy" => {
            identity(&mut request, &[("Faction", "Khri-habi")]);
            basic(
                &mut request,
                "HekauList",
                ["Alchemy 1", "Amulets 1", "Celestial 1"],
            );
            basic(
                &mut request,
                "SpellList",
                ["Analyze Material", "Ashen Shroud", "Becalm"],
            );
        }
        _ => unreachable!(),
    }
    request
}

fn purchase(id: &str) -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: uuid::Uuid::new_v4().to_string(),
        power_id: id.into(),
        profile: Some(PricingProfile::ConfiguredCreature),
        relation: Some(ClanRelation::InClan),
        affiliation_name: "Fixture affinity".into(),
        day: "2026-11-18".into(),
        available_from: "2026-11-18".into(),
        next_session_reference: "Fixture session".into(),
        actor: "Synthetic Storyteller".into(),
        reason: "Fixture purchase".into(),
        policy_version: "fixture".into(),
        policy_reference: "Fixture rules".into(),
        affiliation_evidence: "Fixture affiliation".into(),
        approval_reference: "Fixture approval".into(),
        eligibility_evidence: "Fixture eligibility".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Fixture teacher".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    }
}

fn initialize(lib: &mut Library, id: &str) {
    lib.initialize_xp(
        id,
        &Baseline {
            day: "2026-11-17".into(),
            earned: "20".into(),
            unspent: "20".into(),
            actor: "Synthetic Storyteller".into(),
            reason: "Fixture opening".into(),
            evidence: "Fixture opening".into(),
        },
        lib.revision().unwrap(),
        ST,
    )
    .unwrap();
}

fn buy_and_refund(lib: &mut Library, id: &str, power: &str, price: &str) {
    let mut request = purchase(power);
    if power == "phase7c-hekau.effigy.effigy-1.1" {
        request.relation = Some(ClanRelation::OutOfClan);
    }
    let quote = lib
        .quote_purchase(id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(quote.price.price.total, price, "{power}");
    let receipt = lib.commit_purchase(&quote, ST).unwrap();
    let refund = lib
        .quote_cancellation(
            &receipt.quote.request.id,
            &CancellationRequest {
                id: uuid::Uuid::new_v4().to_string(),
                day: "2026-11-18".into(),
                actor: "Synthetic Storyteller".into(),
                reason: "Fixture refund".into(),
                evidence: "Fixture refund".into(),
            },
            lib.revision().unwrap(),
            ST,
        )
        .unwrap();
    assert_eq!(refund.refund, price);
    lib.commit_cancellation(&refund, ST).unwrap();
    assert_eq!(lib.xp_view(id, ST).unwrap().unspent.as_deref(), Some("20"));
}

#[test]
fn reviewed_starting_templates_price_powers_and_refund_original_cost() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("phase7c.gvlib"), "Synthetic").unwrap();
    for (kind, primary, secondary, primary_cost, secondary_cost, resources) in [
        (
            "Fera",
            "phase7c-fera.gurahl.desperate-strength.1",
            "phase7c-fera.gurahl.rite-of-rending-the-gauntlet.1",
            "3",
            "2",
            [("Willpower", 6), ("Rage", 4), ("Gnosis", 5)],
        ),
        (
            "KueiJin",
            "phase7c-kuei.storm-shintai.conjure-the-duke-of-wind.1",
            "phase7c-kuei.kuei-jin-rites.binding-goods.1",
            "4",
            "2",
            [("Hun", 1), ("YinChi", 2), ("YangChi", 4)],
        ),
        (
            "Demon",
            "phase7c-fallen.lore-of-humanity.translate.1",
            "phase7c-fallen.lore-of-the-celestials.baptism-of-faith.2",
            "3",
            "4",
            [("Willpower", 5), ("Faith", 3), ("Torment", 4)],
        ),
        (
            "Mummy",
            "phase7c-hekau.effigy.effigy-1.1",
            "phase7c-hekau.alchemy.cloud-of-smoke.1",
            "4",
            "1",
            [("Willpower", 5), ("Balance", 1), ("Sekhem", 1)],
        ),
    ] {
        let id = lib
            .create_character(kind, kind, lib.revision().unwrap(), ST)
            .unwrap();
        let request = creation(&lib, &id, kind);
        let quote = lib
            .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
            .unwrap();
        assert!(quote.ready, "{kind}: {:?}", quote.issues);
        assert_eq!(
            quote.configured_budget.as_ref().unwrap().free_spent,
            Some(1),
            "{kind}"
        );
        lib.commit_vampire_creation(&quote, ST).unwrap();
        for (key, expected) in resources {
            assert_eq!(
                lib.sheet_edit(&id, ST).unwrap().resources[key],
                expected,
                "{kind}"
            );
        }
        initialize(&mut lib, &id);
        if kind == "Mummy" {
            let invalid = purchase("phase7c-hekau.alchemy.alchemy-2.2");
            assert_eq!(
                lib.quote_purchase(&id, &invalid, lib.revision().unwrap(), ST)
                    .unwrap_err()
                    .code,
                "MUMMY_HEKAU_PREREQUISITE"
            );
        }
        buy_and_refund(&mut lib, &id, primary, primary_cost);
        buy_and_refund(&mut lib, &id, secondary, secondary_cost);
    }
    let vampire = lib
        .create_character("Vampire", "Synthetic Vampire", lib.revision().unwrap(), ST)
        .unwrap();
    initialize(&mut lib, &vampire);
    for (power, price) in [
        ("phase7c-v20.thaumaturgy.bind-the-accusing-tongue.1", "2"),
        ("phase7c-v20.thaumaturgy.clinging-of-the-insect.3", "4"),
        ("phase7c-da20.necromancy.chill-of-oblivion.5", "6"),
        ("phase7c-lotn.thaumaturgy.bone-of-lies.3", "4"),
    ] {
        buy_and_refund(&mut lib, &vampire, power, price);
    }
}

#[test]
fn gurahl_requires_breed_review_and_specific_owbn_approval() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("phase7c-policy.gvlib"), "Synthetic").unwrap();
    let id = lib
        .create_character("Fera", "Synthetic Fera", 0, ST)
        .unwrap();
    let mut profile = lib.chronicle_profile(ST).unwrap().profile;
    profile.owbn_bylaws_enabled = true;
    profile.allow_non_binding_sources = true;
    lib.save_chronicle_profile(&profile, lib.revision().unwrap(), ST)
        .unwrap();
    let mut request = creation(&lib, &id, "Fera");
    let blocked = lib
        .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(!blocked.ready);
    request.owbn_evidence.references.push(RegulationEvidence {
        rule_id: "phase7c-fera-gurahl".into(),
        regulation: Regulation::CoordinatorApproval,
        owbn_coordinator_reference: "Fixture Gurahl approval".into(),
    });
    let ready = lib
        .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(ready.ready, "{:?}", ready.issues);
    request.current_resources_reference.clear();
    let blocked = lib
        .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(!blocked.ready);
}

#[test]
fn every_new_purchase_resolves_to_its_published_citation() {
    for definition in power_catalogue::builtin()
        .entries
        .iter()
        .filter(|entry| entry.stable_id.starts_with("phase7c-"))
    {
        let citation = definition.source_citation().unwrap();
        assert_eq!(citation.original_pdf_sha256, definition.original_pdf_sha256);
        assert_eq!(citation.pdf_page_1based, definition.pdf_page_1based);
        assert_eq!(citation.printed_page, definition.printed_page);
    }
}
