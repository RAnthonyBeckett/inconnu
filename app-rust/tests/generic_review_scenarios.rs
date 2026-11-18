use grapevine_workbench::{
    creature_profiles::{self, Catalogue as ProfileCatalogue, Profile},
    model::Record,
    policy_templates::PolicyTemplate,
    power_catalogue::{self, Catalogue as PowerCatalogue, Definition},
    pricing::{ClanRelation, PricingProfile, PurchaseFamily, SourceCitation},
    purchase_reviews::{self, FIELD},
    purchases::{PurchaseRequest, TeacherRule},
    scenarios::{self, ScenarioChoice},
};
use serde_json::json;
use uuid::Uuid;

const SYNTHETIC_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const LIBRARY_ID: &str = "synthetic-library";

fn source(kind: &str) -> SourceCitation {
    SourceCitation {
        book: format!("Synthetic {kind} configuration"),
        edition: "Fixture edition".into(),
        system: format!("Synthetic {kind} rules"),
        printed_page: 1,
        pdf_page_1based: 2,
        original_pdf_sha256: SYNTHETIC_HASH.into(),
    }
}

fn configured_fixtures() -> (PowerCatalogue, ProfileCatalogue, Vec<(String, String)>) {
    let mut profiles: Vec<Profile> = creature_profiles::builtin().profiles.clone();
    let mut powers = Vec::new();
    let mut ids = Vec::new();
    for profile in &mut profiles {
        profile.rules_reviewed = true;
        profile.cost_source = Some(source(&profile.kind));
        profile.power_tier_costs.insert("Basic".into(), 4);
        profile.power_tier_costs.insert("Intermediate".into(), 6);
        profile.power_tier_costs.insert("Advanced".into(), 8);
        profile
            .power_tier_costs
            .insert("in_affiliation_adj".into(), 0);
        profile
            .power_tier_costs
            .insert("out_of_affiliation_adj".into(), 1);

        let slug = profile.kind.to_ascii_lowercase().replace(' ', "_");
        let id = format!("synthetic.{slug}.power");
        powers.push(Definition {
            packet_authority: Default::default(),
            tier_mapping_basis: None,
            source_level: None,
            published_source_id: None,
            level_kind: None,
            stable_id: id.clone(),
            template_scope: profile.kind.clone(),
            target_list: profile.primary_power_list.clone(),
            fixed_xp_cost: None,
            family: "Synthetic family".into(),
            branch: Some("Fixture branch".into()),
            name: format!("Synthetic {} power", profile.kind),
            tier: grapevine_workbench::pricing::MetTier::Basic,
            purchase_family: PurchaseFamily::Other,
            profile: PricingProfile::ConfiguredCreature,
            printed_page: 1,
            pdf_page_1based: 2,
            original_pdf_sha256: SYNTHETIC_HASH.into(),
            prerequisite_note: "Synthetic fixture only; no published rule asserted.".into(),
            source_reviewed: true,
            legacy_power: None,
            legacy_preview: None,
        });
        ids.push((profile.kind.clone(), id));
    }

    // A JSON-only Vampire entry exercises the shared catalogue path without an enum
    // variant. Its source identity and price profile come from the existing reviewed row.
    let mut json_only_vampire = power_catalogue::builtin()
        .entries
        .iter()
        .find(|entry| entry.legacy_power.is_some())
        .expect("bundled legacy power")
        .clone();
    json_only_vampire.stable_id = "synthetic.vampire.json_only".into();
    json_only_vampire.name = "Synthetic JSON-only Vampire power".into();
    json_only_vampire.family = "Synthetic Vampire family".into();
    json_only_vampire.prerequisite_note = "Synthetic fixture only.".into();
    json_only_vampire.legacy_power = None;
    json_only_vampire.legacy_preview = None;
    powers.push(json_only_vampire);

    let powers = PowerCatalogue::from_json(
        &serde_json::to_string(&PowerCatalogue {
            version: 1,
            entries: powers,
        })
        .unwrap(),
    )
    .unwrap();
    let profiles = ProfileCatalogue::from_json(
        &serde_json::to_string(&ProfileCatalogue {
            version: 1,
            profiles,
        })
        .unwrap(),
    )
    .unwrap();
    (powers, profiles, ids)
}

fn request(power_id: &str, profile: PricingProfile) -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: Uuid::new_v4().to_string(),
        power_id: power_id.into(),
        profile: Some(profile),
        relation: Some(ClanRelation::InClan),
        affiliation_name: "Synthetic group".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic next session".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic review scenario".into(),
        policy_version: "Synthetic policy v1".into(),
        policy_reference: "Synthetic policy reference".into(),
        affiliation_evidence: "Synthetic affiliation evidence".into(),
        approval_reference: "Synthetic approval".into(),
        eligibility_evidence: "Synthetic eligibility evidence".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Synthetic teacher evidence".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    }
}

fn record(kind: &str) -> Record {
    Record {
        id: Uuid::new_v4().to_string(),
        kind: kind.into(),
        name: format!("Synthetic {kind}"),
        player_id: None,
        legacy_id: None,
        public_fields: serde_json::Map::new(),
        private_fields: serde_json::Map::new(),
        original: json!({"fixture": true}),
        source_sha256: "b".repeat(64),
        st_markers: None,
        build: None,
        xp_ledger: None,
    }
}

fn choice(power_id: &str) -> ScenarioChoice {
    ScenarioChoice {
        power_id: power_id.into(),
        relation: Some(ClanRelation::InClan),
    }
}

#[test]
fn configured_scenarios_and_private_reviews_cover_all_thirteen_kinds() {
    let (powers, profiles, ids) = configured_fixtures();
    assert_eq!(ids.len(), 13);

    for (kind, power_id) in ids {
        let before = serde_json::to_value(record(&kind)).unwrap();
        let scenario = scenarios::preview_from_catalogues(
            &kind,
            Some("3"),
            Some(PricingProfile::ConfiguredCreature),
            &[choice(&power_id)],
            &[],
            &powers,
            &profiles,
        )
        .unwrap();
        assert_eq!(scenario.total_spend, "4", "{kind}");
        assert_eq!(scenario.projected.as_deref(), Some("-1"), "{kind}");
        assert!(!scenario.purchase_enabled);
        assert!(!scenario.items[0].purchase_enabled);
        assert_eq!(
            scenario.items[0].target_list(),
            profiles.profile(&kind).unwrap().primary_power_list.as_str()
        );
        assert_eq!(
            scenario.items[0]
                .catalogue_binding
                .as_ref()
                .map(|binding| binding.template_scope.as_str()),
            Some(kind.as_str())
        );

        let source_record = record(&kind);
        let input = request(&power_id, PricingProfile::ConfiguredCreature);
        let review = purchase_reviews::make_from_catalogues(
            LIBRARY_ID,
            &source_record,
            &input,
            "Synthetic review",
            &powers,
            &profiles,
        )
        .unwrap();
        assert!(review.configured_snapshot_hash.is_some(), "{kind}");
        let stored = purchase_reviews::append(LIBRARY_ID, &source_record, &review).unwrap();
        assert!(stored.private_fields.contains_key(FIELD));
        assert!(!stored.public_fields.contains_key(FIELD));

        let mut reusable = request("unselected.placeholder", PricingProfile::ConfiguredCreature);
        purchase_reviews::reuse_from_catalogues(
            LIBRARY_ID,
            &stored,
            &review.id,
            &mut reusable,
            &powers,
            &profiles,
        )
        .unwrap();
        assert_eq!(reusable.power_id, power_id);
        assert_eq!(reusable.profile, Some(PricingProfile::ConfiguredCreature));
        assert_eq!(reusable.review_id.as_deref(), Some(review.id.as_str()));

        // The scenario operation above receives only a character kind and never writes it.
        assert_eq!(before["kind"].as_str(), Some(kind.as_str()));
    }
}

#[test]
fn json_only_vampire_definition_uses_shared_preview_and_legacy_review_shape() {
    let (powers, profiles, _) = configured_fixtures();
    let id = "synthetic.vampire.json_only";
    let scenario = scenarios::preview_from_catalogues(
        "Vampire",
        None,
        Some(PricingProfile::MetLawsOfTheNightRevised),
        &[choice(id)],
        &[],
        &powers,
        &profiles,
    )
    .unwrap();
    assert_eq!(scenario.items[0].power_id, id);
    assert!(scenario.items[0].catalogue_binding.is_some());
    assert!(!scenario.purchase_enabled);

    let vampire = record("Vampire");
    let review = purchase_reviews::make_from_catalogues(
        LIBRARY_ID,
        &vampire,
        &request(id, PricingProfile::MetLawsOfTheNightRevised),
        "Synthetic legacy-profile review",
        &powers,
        &profiles,
    )
    .unwrap();
    assert!(review.configured_snapshot_hash.is_none());
    let serialized = serde_json::to_value(&review).unwrap();
    assert!(serialized.get("configured_snapshot_hash").is_none());
    assert_eq!(
        review.policy.profile,
        PricingProfile::MetLawsOfTheNightRevised
    );
}

#[test]
fn configured_review_reuse_fails_closed_after_profile_or_power_change() {
    let (powers, profiles, ids) = configured_fixtures();
    let (kind, power_id) = ids.iter().find(|(kind, _)| kind == "Werewolf").unwrap();
    let source_record = record(kind);
    let review = purchase_reviews::make_from_catalogues(
        LIBRARY_ID,
        &source_record,
        &request(power_id, PricingProfile::ConfiguredCreature),
        "Synthetic review",
        &powers,
        &profiles,
    )
    .unwrap();
    let stored = purchase_reviews::append(LIBRARY_ID, &source_record, &review).unwrap();

    let mut changed_profiles = profiles.clone();
    let changed = changed_profiles
        .profiles
        .iter_mut()
        .find(|profile| profile.kind == *kind)
        .unwrap();
    changed.power_tier_costs.insert("Basic".into(), 5);
    let changed_profiles =
        ProfileCatalogue::from_json(&serde_json::to_string(&changed_profiles).unwrap()).unwrap();
    let mut untouched = request("placeholder", PricingProfile::ConfiguredCreature);
    let before = untouched.clone();
    let error = purchase_reviews::reuse_from_catalogues(
        LIBRARY_ID,
        &stored,
        &review.id,
        &mut untouched,
        &powers,
        &changed_profiles,
    )
    .unwrap_err();
    assert_eq!(error.code, "REVIEW_CONFIG_STALE");
    assert_eq!(untouched, before);

    let mut changed_powers = powers.clone();
    changed_powers
        .entries
        .iter_mut()
        .find(|definition| definition.stable_id == *power_id)
        .unwrap()
        .prerequisite_note = "Changed synthetic definition".into();
    let changed_powers =
        PowerCatalogue::from_json(&serde_json::to_string(&changed_powers).unwrap()).unwrap();
    let mut linked_request = request(power_id, PricingProfile::ConfiguredCreature);
    linked_request.review_id = Some(review.id.clone());
    let error = purchase_reviews::verify_from_catalogues(
        LIBRARY_ID,
        &stored,
        &linked_request,
        &changed_powers,
        &profiles,
    )
    .unwrap_err();
    assert_eq!(error.code, "REVIEW_CONFIG_STALE");
    // Receipt validation uses the original declarations even when current configuration
    // is unavailable or the live character has changed; it must not reprice history.
    let mut historical_record = stored.clone();
    historical_record.name.push_str(" after later edit");
    purchase_reviews::verify_declarations(LIBRARY_ID, &historical_record, &linked_request).unwrap();
}

#[test]
fn unreviewed_profile_cannot_power_a_configured_preview_or_review() {
    let (powers, _, ids) = configured_fixtures();
    let (kind, power_id) = ids.iter().find(|(kind, _)| kind == "Werewolf").unwrap();
    let mut profiles = creature_profiles::builtin().profiles.clone();
    let rules = profiles
        .iter_mut()
        .find(|profile| profile.kind == *kind)
        .unwrap();
    rules.rules_reviewed = false;
    rules.cost_source = None;
    let profiles = ProfileCatalogue::from_json(
        &serde_json::to_string(&ProfileCatalogue {
            version: 1,
            profiles,
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        scenarios::preview_from_catalogues(
            kind,
            None,
            Some(PricingProfile::ConfiguredCreature),
            &[choice(power_id)],
            &[],
            &powers,
            &profiles,
        )
        .unwrap_err()
        .code,
        "CREATURE_RULE_REVIEW_REQUIRED"
    );
    assert_eq!(
        purchase_reviews::make_from_catalogues(
            LIBRARY_ID,
            &record(kind),
            &request(power_id, PricingProfile::ConfiguredCreature),
            "Synthetic review",
            &powers,
            &profiles,
        )
        .unwrap_err()
        .code,
        "CREATURE_RULE_REVIEW_REQUIRED"
    );
}

#[test]
fn policy_template_legacy_entry_points_remain_vampire_scoped() {
    let source = request("synthetic", PricingProfile::ConfiguredCreature);
    assert!(PolicyTemplate::from_request("Legacy API", &source).is_err());
    let mut target = request("target", PricingProfile::MetLawsOfTheNightRevised);
    let configured = PolicyTemplate {
        format_version: 1,
        name: "Synthetic configured policy".into(),
        profile: PricingProfile::ConfiguredCreature,
        policy_version: "Synthetic policy v1".into(),
        policy_reference: "Synthetic policy reference".into(),
        teacher_rule: TeacherRule::Required,
        standard_cost_confirmed: true,
    };
    assert!(configured.apply(&mut target).is_err());
}
