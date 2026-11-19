use grapevine_workbench::{
    cancellations::CancellationRequest,
    model::Role,
    owbn_bylaws::{PacketAuthority, Regulation, SelectedPower, TierMappingBasis},
    power_catalogue,
    pricing::{ClanRelation, PricingProfile},
    published_powers,
    purchases::{PurchaseRequest, TeacherRule},
    sheet,
    store::Library,
    xp::Baseline,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const ST: Role = Role::Storyteller;

fn power(id: &str) -> &'static power_catalogue::Definition {
    power_catalogue::builtin().lookup(id).unwrap()
}

#[test]
fn phase7a_publications_join_exact_source_pages_and_authority() {
    let published = published_powers::catalogue().unwrap();
    for definition in power_catalogue::builtin()
        .entries
        .iter()
        .filter(|p| p.stable_id.starts_with("phase7a-"))
    {
        let citation = definition.source_citation().unwrap();
        assert_eq!(citation.original_pdf_sha256, definition.original_pdf_sha256);
        assert_eq!(citation.pdf_page_1based, definition.pdf_page_1based);
        assert_eq!(citation.printed_page, definition.printed_page);
    }
    let stygian = power("phase7a-wr20-wraith.argos.weather-eye.1");
    assert_eq!((stygian.printed_page, stygian.pdf_page_1based), (151, 216));
    assert_eq!(
        stygian.tier_mapping_basis,
        Some(TierMappingBasis::ConvertedFrom20thLevel)
    );
    let packet = power("phase7a-risen-risen.serendipity.fortuitous-wandering.1");
    assert_eq!(packet.packet_authority, PacketAuthority::BindingPacket);
    assert_eq!(
        published.power(&packet.stable_id).unwrap().printed_page,
        None
    );
    assert_eq!(packet.fixed_xp_cost, Some(4));
    for (kind, count) in [
        (published_powers::Kind::Art, 90),
        (published_powers::Kind::Realm, 30),
    ] {
        assert_eq!(
            published
                .entries
                .iter()
                .filter(|p| p.book_id == "phase7a-c20-packet-changeling" && p.kind == kind)
                .count(),
            count
        );
    }
}

fn add_list(
    request: &mut grapevine_workbench::vampire_creation::Request,
    key: &str,
    traits: Value,
) {
    let mut list = sheet::empty_list(key);
    list["Traits"] = traits;
    request.sheet.lists.insert(key.into(), list);
}

fn input(id: &str) -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: uuid::Uuid::new_v4().to_string(),
        power_id: id.into(),
        profile: Some(PricingProfile::ConfiguredCreature),
        relation: Some(ClanRelation::InClan),
        affiliation_name: "Synthetic local affiliation".into(),
        day: "2026-11-18".into(),
        available_from: "2026-11-18".into(),
        next_session_reference: "Synthetic session".into(),
        actor: "Synthetic Storyteller".into(),
        reason: "Synthetic fixture".into(),
        policy_version: "fixture".into(),
        policy_reference: "fixture".into(),
        affiliation_evidence: "fixture".into(),
        approval_reference: "fixture".into(),
        eligibility_evidence: "fixture".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "fixture".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    }
}

#[test]
fn changeling_creation_and_original_cost_refunds_use_both_lists() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("test.gvlib"), "Synthetic").unwrap();
    let id = lib
        .create_character("Changeling", "Synthetic Changeling", 0, ST)
        .unwrap();
    let mut request = lib.vampire_creation_context(&id, ST).unwrap().request;
    request.standard_pools = true;
    request.review_confirmed = true;
    request.actor = "Synthetic Storyteller".into();
    request.day = "2026-11-18".into();
    request.reason = "Synthetic creation".into();
    request.policy_version = "fixture".into();
    request.review_reference = "fixture".into();
    request.manual_power_reference = "fixture".into();
    request
        .sheet
        .identity
        .insert("Seeming".into(), "Childling".into());
    for (key, value) in [
        ("PhysicalList", 8),
        ("SocialList", 5),
        ("MentalList", 3),
        ("AbilityList", 5),
        ("BackgroundList", 5),
    ] {
        add_list(
            &mut request,
            key,
            json!([{"Name":"Synthetic","Value":value.to_string()}]),
        );
    }
    add_list(
        &mut request,
        "ArtList",
        json!([
            {"Name":"Trick of the Light","Value":"basic"},
            {"Name":"Creeping Shadows","Value":"basic"},
            {"Name":"Protocol","Value":"basic"}
        ]),
    );
    add_list(
        &mut request,
        "RealmList",
        json!([
            {"Name":"True Friend","Value":"1"},
            {"Name":"Hearty Commoner","Value":"1"},
            {"Name":"Base Element","Value":"1"},
            {"Name":"Ornate Garb","Value":"1"},
            {"Name":"Chamber","Value":"1"}
        ]),
    );
    let quote = lib.quote_vampire_creation(&id, &request, 1, ST).unwrap();
    assert!(quote.ready, "{:?}", quote.issues);
    assert_eq!(
        quote.configured_budget.as_ref().unwrap().free_remaining,
        Some(4)
    );
    assert_eq!(quote.effective_sheet.resources["Glamour"], 5);
    assert_eq!(quote.effective_sheet.resources["Willpower"], 4);
    assert_eq!(quote.effective_sheet.resources["Banality"], 3);
    lib.commit_vampire_creation(&quote, ST).unwrap();
    lib.initialize_xp(
        &id,
        &Baseline {
            day: "2026-11-17".into(),
            earned: "20".into(),
            unspent: "20".into(),
            actor: "Synthetic Storyteller".into(),
            reason: "fixture".into(),
            evidence: "fixture".into(),
        },
        lib.revision().unwrap(),
        ST,
    )
    .unwrap();
    let mut chronicle = lib.chronicle_profile(ST).unwrap().profile;
    chronicle.owbn_bylaws_enabled = true;
    chronicle.allow_non_binding_sources = true;
    lib.save_chronicle_profile(&chronicle, lib.revision().unwrap(), ST)
        .unwrap();
    let chronos = input("phase7a-c20-packet-changeling.chronos.backward-glance.1");
    let denied = lib
        .quote_purchase(&id, &chronos, lib.revision().unwrap(), ST)
        .unwrap_err();
    assert_eq!(denied.code, "OWBN_REGULATION_EVIDENCE");
    for (power, cost, value) in [
        (
            "phase7a-c20-packet-changeling.wayfare.hopscotch.1",
            "3",
            "basic",
        ),
        ("phase7a-c20-packet-changeling.time.three-fold.1", "2", "1"),
    ] {
        let q = lib
            .quote_purchase(&id, &input(power), lib.revision().unwrap(), ST)
            .unwrap();
        assert_eq!(q.price.price.total, cost);
        assert_eq!(q.trait_added["Value"], value);
        let receipt = lib.commit_purchase(&q, ST).unwrap();
        let refund = lib
            .quote_cancellation(
                &receipt.quote.request.id,
                &CancellationRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    day: "2026-11-18".into(),
                    actor: "Synthetic Storyteller".into(),
                    reason: "fixture refund".into(),
                    evidence: "fixture".into(),
                },
                lib.revision().unwrap(),
                ST,
            )
            .unwrap();
        assert_eq!(refund.refund, cost);
        lib.commit_cancellation(&refund, ST).unwrap();
    }
}

#[test]
fn bylaws_remain_distinct_from_catalogue_inclusion_and_profile_readiness() {
    let policy = grapevine_workbench::owbn_bylaws::builtin();
    let mut sheet = grapevine_workbench::model::SheetEdit::default();
    sheet.identity.insert("Guild".into(), "Harbingers".into());
    sheet
        .identity
        .insert("Legion".into(), "Legion of the Damned".into());
    let rules = policy.matching("Wraith", &sheet, &[], false);
    assert!(rules.iter().any(|rule| rule.id == "wraith-harbingers"
        && rule.regulation(false) == Regulation::CoordinatorNotify));
    assert!(rules.iter().any(|rule| rule.id == "wraith-legion-damned"
        && rule.regulation(false) == Regulation::CoordinatorApproval));
    let rules = policy.matching("Risen", &sheet, &[], false);
    assert!(rules.iter().any(
        |rule| rule.id == "risen" && rule.regulation(false) == Regulation::CoordinatorApproval
    ));
    let spring = [SelectedPower {
        names: &["Renewal", "Spring"],
        level: Some(5),
    }];
    assert!(
        policy
            .matching("Changeling", &sheet, &spring, false)
            .iter()
            .any(|r| r.id == "changeling-spring-5")
    );
    let reviewed: BTreeSet<_> = grapevine_workbench::creature_profiles::builtin()
        .profiles
        .iter()
        .filter(|p| p.rules_reviewed)
        .map(|p| p.kind.as_str())
        .collect();
    assert_eq!(
        reviewed,
        BTreeSet::from([
            "Changeling",
            "Mage",
            "Mortal",
            "Risen",
            "Vampire",
            "Werewolf",
            "Wraith"
        ])
    );
}
