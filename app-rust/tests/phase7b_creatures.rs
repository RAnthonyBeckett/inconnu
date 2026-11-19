use grapevine_workbench::{
    cancellations::CancellationRequest,
    model::Role,
    owbn_bylaws::{Regulation, RegulationEvidence},
    power_catalogue,
    pricing::{ClanRelation, PricingProfile},
    published_powers,
    purchases::{PurchaseRequest, TeacherRule},
    sheet,
    store::Library,
    vampire_creation::{Request, RisenStart},
    xp::Baseline,
};
use serde_json::{Value, json};

const ST: Role = Role::Storyteller;

fn list(request: &mut Request, key: &str, traits: Value) {
    let mut value = sheet::empty_list(key);
    value["Traits"] = traits;
    request.sheet.lists.insert(key.into(), value);
}

fn reviewed(lib: &Library, id: &str, kind: &str) -> Request {
    let mut request = lib.vampire_creation_context(id, ST).unwrap().request;
    let profile = grapevine_workbench::creature_profiles::profile(kind).unwrap();
    request.standard_pools = true;
    request.review_confirmed = true;
    request.actor = "Synthetic Storyteller".into();
    request.day = "2026-11-18".into();
    request.reason = "Synthetic character creation".into();
    request.policy_version = "fixture".into();
    request.review_reference = "fixture review".into();
    request.manual_power_reference = "fixture initial powers".into();
    request.current_resources_reference = "fixture starting resources".into();
    for (key, count) in [
        ("PhysicalList", profile.attribute_priorities[0] + 1),
        ("SocialList", profile.attribute_priorities[1]),
        ("MentalList", profile.attribute_priorities[2]),
        ("AbilityList", profile.ability_base),
        ("BackgroundList", profile.background_base),
    ] {
        list(
            &mut request,
            key,
            json!([{"Name": "Fixture", "Value": count.to_string()}]),
        );
    }
    if profile.basic_power_base == 3 {
        list(
            &mut request,
            &profile.primary_power_list,
            json!([
                {"Name":"Fixture A","Value":"basic"},
                {"Name":"Fixture B","Value":"basic"},
                {"Name":"Fixture C","Value":"basic"}
            ]),
        );
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
        affiliation_name: "Synthetic affinity".into(),
        day: "2026-11-18".into(),
        available_from: "2026-11-18".into(),
        next_session_reference: "fixture session".into(),
        actor: "Synthetic Storyteller".into(),
        reason: "fixture".into(),
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
fn phase7b_source_joins_resolve_and_packet_hunter_edges_remain_indexed() {
    let published = published_powers::catalogue().unwrap();
    for definition in power_catalogue::builtin()
        .entries
        .iter()
        .filter(|d| d.stable_id.starts_with("phase7b-"))
    {
        let citation = definition.source_citation().unwrap();
        assert_eq!(citation.original_pdf_sha256, definition.original_pdf_sha256);
        assert_eq!(citation.pdf_page_1based, definition.pdf_page_1based);
    }
    assert_eq!(
        published
            .entries
            .iter()
            .filter(|row| row.book_id == "phase7b-imbued")
            .count(),
        6
    );
}

#[test]
fn reviewed_phase7b_sheets_create_and_refund_purchases_in_both_lists() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("phase7b.gvlib"), "Synthetic").unwrap();
    for (kind, primary, secondary, primary_cost, secondary_cost) in [
        (
            "Wraith",
            "phase7a-wr20-wraith.argos.weather-eye.1",
            "phase7b-phase7a-wr20-wraith.shadow-thorns.deaths-sigil.1",
            "4",
            "1",
        ),
        (
            "Risen",
            "phase7a-risen-risen.serendipity.fortuitous-wandering.1",
            "phase7b-risen.met.lotn-revised.celerity.basic.alacrity",
            "4",
            "4",
        ),
        (
            "Werewolf",
            "phase7b-w20.homid.apecrafts-blessings.1",
            "phase7b-w20.rites-of-accord.rite-of-cleansing.1",
            "3",
            "2",
        ),
        (
            "Mage",
            "phase7b-ascension.time.time-apprentice.1",
            "phase7b-ascension.spirit.animate-the-dead.2",
            "4",
            "2",
        ),
        (
            "Mortal",
            "phase7b-hunt.animal-communication.communication.1",
            "phase7b-hunt.alchemy.ointment-of-maedb.2",
            "3",
            "2",
        ),
    ] {
        let id = lib
            .create_character(
                kind,
                &format!("Synthetic {kind}"),
                lib.revision().unwrap(),
                ST,
            )
            .unwrap();
        let mut request = reviewed(&lib, &id, kind);
        match kind {
            "Wraith" => {
                list(
                    &mut request,
                    "ThornList",
                    json!([{"Name":"Synthetic Thorn","Value":"2"}]),
                );
                request.sheet.resources.insert("Angst".into(), 2);
                let blocked = lib
                    .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
                    .unwrap();
                assert!(!blocked.ready);
                request.shadow_review = true;
            }
            "Risen" => {
                request.risen_start = Some(RisenStart {
                    from_wraith: true,
                    potency: 1,
                    review_confirmed: true,
                });
                request.sheet.resources.insert("Corpus".into(), 8);
                request.sheet.resources.insert("Pathos".into(), 11);
            }
            "Werewolf" => {
                for (key, value) in [
                    ("Breed", "Metis"),
                    ("Auspice", "Galliard"),
                    ("Tribe", "Children of Gaia"),
                ] {
                    request.sheet.identity.insert(key.into(), value.into());
                }
                request.budget.willpower_free = 1;
            }
            "Mage" => {
                request.mage_arete_free = 1;
                list(
                    &mut request,
                    "BackgroundList",
                    json!([
                        {"Name":"Avatar","Value":"2"},
                        {"Name":"Other","Value":"5"}
                    ]),
                );
                list(
                    &mut request,
                    "SphereList",
                    json!([
                        {"Name":"Spirit — Initiate","Value":"2"},
                        {"Name":"Correspondence — Apprentice","Value":"1"},
                        {"Name":"Entropy — Apprentice","Value":"1"},
                        {"Name":"Forces — Apprentice","Value":"1"},
                        {"Name":"Life — Apprentice","Value":"1"}
                    ]),
                );
            }
            _ => {}
        }
        let quote = lib
            .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
            .unwrap();
        assert!(quote.ready, "{kind} {:?}", quote.issues);
        let spent = match kind {
            "Werewolf" => 2,
            "Mage" => 3,
            _ => 1,
        };
        assert_eq!(
            quote.configured_budget.as_ref().unwrap().free_spent,
            Some(spent),
            "{kind}"
        );
        let resources = match kind {
            "Wraith" => [("Willpower", 3), ("Corpus", 10), ("Angst", 2)].as_slice(),
            "Risen" => [("Willpower", 6), ("Corpus", 8), ("Pathos", 11)].as_slice(),
            "Werewolf" => [("Willpower", 5), ("Gnosis", 3), ("Rage", 4)].as_slice(),
            "Mage" => [("Arete", 2), ("Quintessence", 2), ("Paradox", 0)].as_slice(),
            _ => [("Willpower", 3), ("Humanity", 4)].as_slice(),
        };
        lib.commit_vampire_creation(&quote, ST).unwrap();
        let committed = lib.sheet_edit(&id, ST).unwrap();
        for (key, value) in resources {
            assert_eq!(committed.resources.get(*key), Some(value), "{kind}");
        }
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
        for (power, cost) in [(primary, primary_cost), (secondary, secondary_cost)] {
            let mut request = purchase(power);
            if kind == "Wraith" && power == secondary {
                assert_eq!(
                    lib.quote_purchase(&id, &request, lib.revision().unwrap(), ST)
                        .unwrap_err()
                        .code,
                    "SHADOW_REVIEW_REQUIRED"
                );
                request.owbn_evidence.shadow_review_reference = "fixture Shadow award".into();
            }
            let quote = lib
                .quote_purchase(&id, &request, lib.revision().unwrap(), ST)
                .unwrap();
            assert_eq!(quote.price.price.total, cost, "{power}");
            let receipt = lib.commit_purchase(&quote, ST).unwrap();
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
}

#[test]
fn owbn_risen_creation_requires_specific_coordinator_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("policy.gvlib"), "Synthetic").unwrap();
    let id = lib
        .create_character("Risen", "Synthetic Risen", 0, ST)
        .unwrap();
    let mut profile = lib.chronicle_profile(ST).unwrap().profile;
    profile.owbn_bylaws_enabled = true;
    profile.allow_non_binding_sources = true;
    lib.save_chronicle_profile(&profile, lib.revision().unwrap(), ST)
        .unwrap();
    let mut request = reviewed(&lib, &id, "Risen");
    request.risen_start = Some(RisenStart {
        from_wraith: false,
        potency: 0,
        review_confirmed: true,
    });
    let quote = lib
        .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(!quote.ready);
    request.owbn_evidence.references.push(RegulationEvidence {
        rule_id: "risen".into(),
        regulation: Regulation::CoordinatorApproval,
        owbn_coordinator_reference: "Fixture Risen approval".into(),
    });
    let quote = lib
        .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(quote.ready, "{:?}", quote.issues);
}
