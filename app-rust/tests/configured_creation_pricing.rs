use grapevine_workbench::{
    creature_profiles::{self, Profile},
    model::Record,
    pricing::{self, ClanRelation, MetTier, PurchaseFamily, SourceCitation},
    sheet, vampire_creation as creation,
};
use serde_json::json;
use std::collections::BTreeMap;

fn reviewed(kind: &str) -> Profile {
    let mut p = creature_profiles::profile(kind).unwrap().clone();
    p.rules_reviewed = true;
    p.cost_source = Some(SourceCitation {
        book: "Synthetic fixture only".into(),
        edition: "Synthetic".into(),
        system: "MET fixture".into(),
        printed_page: 1,
        pdf_page_1based: 2,
        original_pdf_sha256: "a".repeat(64),
    });
    p.attribute_priorities = [1; 3];
    p.ability_base = 1;
    p.background_base = 1;
    p.basic_power_base = 1;
    p.secondary_power_base = None;
    p.free_traits_base = 30;
    p.free_trait_costs = Some(creature_profiles::FreeTraitCosts {
        attribute: 2,
        ability: 1,
        background: 1,
        primary_power: 3,
        secondary_power: 4,
        specialization: 1,
        permanent_resource: 2,
    });
    p.starting_resources = BTreeMap::from([("Willpower".into(), 3)]);
    p
}
fn record(kind: &str) -> Record {
    Record {
        id: uuid::Uuid::new_v4().to_string(),
        kind: kind.into(),
        name: "Synthetic".into(),
        player_id: None,
        legacy_id: None,
        public_fields: json!({"Name":"Synthetic","Status":"Draft"})
            .as_object()
            .unwrap()
            .clone(),
        private_fields: Default::default(),
        original: json!({"created_in_app":true}),
        source_sha256: String::new(),
        st_markers: None,
        build: None,
        xp_ledger: None,
    }
}
fn request(record: &Record, p: &Profile) -> creation::Request {
    let mut r = creation::initial_request(record);
    r.standard_pools = true;
    r.review_confirmed = true;
    r.actor = "Synthetic reviewer".into();
    r.day = "2026-11-17".into();
    r.reason = "Fixture math".into();
    r.policy_version = "Fixture v1".into();
    r.review_reference = "Synthetic eligibility".into();
    r.manual_power_reference = "Synthetic initial powers".into();
    for (key, value) in [
        ("PhysicalList", 3),
        ("SocialList", 1),
        ("MentalList", 1),
        ("AbilityList", 2),
        ("BackgroundList", 1),
        ("MeritList", 1),
    ] {
        let mut list = sheet::empty_list(key);
        list["Traits"] = json!([{"Name":"Synthetic","Value":value.to_string()}]);
        r.sheet.lists.insert(key.into(), list);
    }
    for (key, count) in [(&p.primary_power_list, 2), (&p.secondary_power_list, 1)] {
        let mut list = sheet::empty_list(key);
        list["Traits"] = json!(
            (0..count)
                .map(|n| json!({"Name":format!("Power{n}"),"Value":"Basic"}))
                .collect::<Vec<_>>()
        );
        r.sheet.lists.insert(key.clone(), list);
    }
    r.specializations = vec!["Synthetic".into()];
    r.budget.willpower_free = 1;
    r
}
#[test]
fn all_twelve_configured_kinds_price_excess_and_generate_only_reviewed_resources() {
    for base in &creature_profiles::builtin().profiles {
        if base.kind == "Vampire" {
            continue;
        }
        let profile = reviewed(&base.kind);
        let record = record(&base.kind);
        let request = request(&record, &profile);
        let before = serde_json::to_value(&record).unwrap();
        let q =
            creation::quote_configured_profile("fixture", 0, &record, &request, &profile).unwrap();
        assert!(q.ready, "{}: {:?}", base.kind, q.issues);
        let budget = q.configured_budget.unwrap();
        assert_eq!(budget.free_spent, Some(16));
        assert_eq!(budget.free_remaining, Some(14));
        assert_eq!(q.effective_sheet.resources["Willpower"], 4);
        assert_eq!(serde_json::to_value(record).unwrap(), before);
        assert_eq!(creature_profiles::profile(&base.kind), Some(base));
    }
}
#[test]
fn missing_costs_overdraw_unreviewed_rules_and_invalid_resources_fail_closed() {
    let mut profile = reviewed("Wraith");
    let record = record("Wraith");
    let mut request = request(&record, &profile);
    profile.free_trait_costs = None;
    let q = creation::quote_configured_profile("fixture", 0, &record, &request, &profile).unwrap();
    assert!(!q.ready);
    assert_eq!(q.configured_budget.unwrap().free_spent, None);
    profile = reviewed("Wraith");
    profile.free_traits_base = 1;
    assert!(
        !creation::quote_configured_profile("fixture", 0, &record, &request, &profile)
            .unwrap()
            .ready
    );
    profile = reviewed("Wraith");
    profile.rules_reviewed = false;
    assert!(
        !creation::quote_configured_profile("fixture", 0, &record, &request, &profile)
            .unwrap()
            .ready
    );
    profile = reviewed("Wraith");
    request.budget.willpower_free = -1;
    assert!(creation::quote_configured_profile("fixture", 0, &record, &request, &profile).is_err());
    request.budget.willpower_free = 0;
    request.sheet.resources.insert("Pathos".into(), 99);
    assert!(
        !creation::quote_configured_profile("fixture", 0, &record, &request, &profile)
            .unwrap()
            .ready
    );
    for (key, value) in [("TempWillpower", 1), ("Willpower", 4097), ("Unknown", 1)] {
        let mut invalid = profile.clone();
        invalid.starting_resources.insert(key.into(), value);
        assert!(invalid.validate_snapshot().is_err());
    }
}
#[test]
fn secondary_prices_can_fall_back_to_primary_and_defaults_preserve_old_wire() {
    let mut p = reviewed("Wraith");
    p.secondary_power_tier_costs = None;
    let fallback = pricing::configured_preview_for_list(
        "Wraith",
        PurchaseFamily::Other,
        MetTier::Basic,
        ClanRelation::InClan,
        &p,
        &p.secondary_power_list,
    )
    .unwrap();
    let primary = pricing::configured_preview(
        "Wraith",
        PurchaseFamily::Other,
        MetTier::Basic,
        ClanRelation::InClan,
        &p,
    )
    .unwrap();
    assert_eq!(fallback.total, primary.total);
    p.secondary_power_tier_costs = Some(
        [
            ("Basic", 2),
            ("Intermediate", 4),
            ("Advanced", 6),
            ("in_affiliation_adj", 0),
            ("out_of_affiliation_adj", 2),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect(),
    );
    let q = pricing::configured_preview_for_list(
        "Wraith",
        PurchaseFamily::Other,
        MetTier::Intermediate,
        ClanRelation::OutOfClan,
        &p,
        &p.secondary_power_list,
    )
    .unwrap();
    assert_eq!(q.total, "6");
    assert!(!q.purchase_enabled);
    assert!(
        pricing::configured_preview_for_list(
            "Wraith",
            PurchaseFamily::Other,
            MetTier::Basic,
            ClanRelation::InClan,
            &p,
            "DisciplineList"
        )
        .is_err()
    );
    let mut legacy_profile = creature_profiles::profile("Vampire").unwrap().clone();
    legacy_profile.secondary_power_tier_costs = None;
    let legacy = serde_json::to_value(legacy_profile).unwrap();
    for key in [
        "free_trait_costs",
        "starting_resources",
        "secondary_power_tier_costs",
    ] {
        assert!(legacy.get(key).is_none());
    }
    let again: Profile = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(serde_json::to_value(again).unwrap(), legacy);
}

#[test]
fn various_needs_a_local_free_trait_configuration_even_at_base() {
    let mut profile = reviewed("Various");
    profile.free_trait_costs = None;
    profile.attribute_priorities = [0; 3];
    profile.ability_base = 0;
    profile.background_base = 0;
    profile.basic_power_base = 0;
    let record = record("Various");
    let mut request = request(&record, &profile);
    for list in request.sheet.lists.values_mut() {
        list["Traits"] = json!([]);
    }
    request.specializations.clear();
    request.budget.willpower_free = 0;
    let quote =
        creation::quote_configured_profile("fixture", 0, &record, &request, &profile).unwrap();
    assert!(!quote.ready);
    assert!(
        quote
            .issues
            .iter()
            .any(|issue| issue.contains("explicit local Free Trait configuration"))
    );
}
