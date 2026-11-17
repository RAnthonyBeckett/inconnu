use grapevine_workbench::{
    creature_profiles, power_catalogue,
    pricing::{self, ClanRelation, MetTier, PurchaseFamily},
    sheet,
};
use serde_json::json;

#[test]
fn all_thirteen_profiles_are_explicit_and_unknown_defaults_never_become_approved_rules() {
    let profiles = creature_profiles::builtin();
    assert_eq!(profiles.profiles.len(), 13);
    for p in &profiles.profiles {
        assert!(sheet::is_character_kind(&p.kind));
        assert!(p.owns_power_list(&p.primary_power_list));
        if p.kind != "Vampire" {
            assert!(!p.rules_reviewed);
            assert_eq!(
                p.trait_cost("Attribute", 0).unwrap_err().code,
                "CREATURE_RULE_REVIEW_REQUIRED"
            );
        }
    }
    assert!(creature_profiles::profile("Player").is_none());
}

#[test]
fn the_same_json_schema_and_price_engine_accept_every_creature_without_new_power_enums() {
    for profile in &creature_profiles::builtin().profiles {
        let mut rules = profile.clone();
        rules.rules_reviewed = true;
        rules.cost_source = Some(pricing::SourceCitation {
            book: "Synthetic rules fixture".into(),
            edition: "Fixture".into(),
            system: "Test only".into(),
            printed_page: 1,
            pdf_page_1based: 2,
            original_pdf_sha256: "a".repeat(64),
        });
        let data = json!({"version":1,"entries":[{"stable_id":format!("synthetic.{}.first",profile.kind.to_ascii_lowercase()),"template_scope":profile.kind,"target_list":profile.primary_power_list,"family":"Synthetic family","branch":"Common","name":"Synthetic first power","tier":"Basic","purchase_family":"Other","profile":"ConfiguredCreature","printed_page":1,"pdf_page_1based":2,"original_pdf_sha256":"a".repeat(64),"prerequisite_note":"Fixture only; no published rule asserted.","source_reviewed":true}]});
        let catalogue = power_catalogue::Catalogue::from_json(&data.to_string()).unwrap();
        assert_eq!(catalogue.entries[0].template_scope, profile.kind);
        assert!(catalogue.entries[0].legacy_power.is_none());
        for (tier, cost) in [
            (MetTier::Basic, "4"),
            (MetTier::Intermediate, "7"),
            (MetTier::Advanced, "10"),
        ] {
            let price = pricing::configured_preview(
                &profile.kind,
                PurchaseFamily::Other,
                tier,
                ClanRelation::OutOfClan,
                &rules,
            )
            .unwrap();
            assert_eq!(price.total, cost);
            assert!(!price.purchase_enabled);
        }
        assert_eq!(rules.trait_cost("Ability", 4).unwrap(), 1);
        assert_eq!(rules.trait_cost("Ability", 5).unwrap(), 2);
        assert!(rules.trait_cost("Ability", 10).is_err());
    }
}

#[test]
fn malformed_duplicate_and_cross_template_configuration_fail_closed() {
    let mut value = serde_json::to_value(creature_profiles::builtin()).unwrap();
    value["profiles"][1]["kind"] = value["profiles"][0]["kind"].clone();
    assert!(creature_profiles::Catalogue::from_json(&value.to_string()).is_err());
    let mut powers = serde_json::to_value(power_catalogue::builtin()).unwrap();
    powers["entries"][0]["target_list"] = json!("GiftList");
    assert!(power_catalogue::Catalogue::from_json(&powers.to_string()).is_err());
    let mut value = serde_json::to_value(creature_profiles::builtin()).unwrap();
    value["profiles"][0]["trait_xp_costs"]["Ability"][1]["max_next"] = json!(1);
    assert!(creature_profiles::Catalogue::from_json(&value.to_string()).is_err());
}

#[test]
fn configured_power_purchase_and_original_cost_refund_share_one_path_for_all_thirteen_kinds() {
    use grapevine_workbench::{
        cancellations::{self, CancellationRequest},
        model::{Record, Role},
        pricing::PricingProfile,
        purchases::{self, PurchaseRequest, TeacherRule},
        store::Library,
        xp::{self, Baseline},
    };
    for profile in &creature_profiles::builtin().profiles {
        for target_list in [&profile.primary_power_list, &profile.secondary_power_list] {
            let directory = tempfile::TempDir::new().unwrap();
            let path = directory.path().join("synthetic.gvlib");
            let mut lib = Library::create(&path, "Synthetic rules test").unwrap();
            let id = lib
                .create_character(&profile.kind, "Synthetic character", 0, Role::Storyteller)
                .unwrap();
            lib.initialize_xp(
                &id,
                &Baseline {
                    day: "2026-01-01".into(),
                    earned: "20".into(),
                    unspent: "20".into(),
                    actor: "Synthetic ST".into(),
                    reason: "Fixture opening".into(),
                    evidence: "Fixture only".into(),
                },
                1,
                Role::Storyteller,
            )
            .unwrap();
            let conn = rusqlite::Connection::open(&path).unwrap();
            let data: String = conn
                .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
                .unwrap();
            let record: Record = serde_json::from_str(&data).unwrap();
            let original_hash = purchases::record_hash(&record).unwrap();
            let mut rules = serde_json::to_value(creature_profiles::builtin()).unwrap();
            let row = rules["profiles"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|p| p["kind"] == profile.kind)
                .unwrap();
            row["rules_reviewed"] = json!(true);
            row["cost_source"] = json!({"book":"Synthetic cost fixture","edition":"Fixture","system":"Tests only","printed_page":1,"pdf_page_1based":2,"original_pdf_sha256":"a".repeat(64)});
            let rules = creature_profiles::Catalogue::from_json(&rules.to_string()).unwrap();
            let power_id = format!("synthetic.{}.first", profile.kind.to_ascii_lowercase());
            let powers=power_catalogue::Catalogue::from_json(&json!({"version":1,"entries":[{"stable_id":power_id,"template_scope":profile.kind,"target_list":target_list,"family":"Synthetic family","branch":"Common","name":"Synthetic first power","tier":"Basic","purchase_family":"Other","profile":"ConfiguredCreature","printed_page":1,"pdf_page_1based":2,"original_pdf_sha256":"a".repeat(64),"prerequisite_note":"Synthetic fixture only","source_reviewed":true}]}).to_string()).unwrap();
            let request = PurchaseRequest {
                review_id: None,
                id: uuid::Uuid::new_v4().to_string(),
                power_id,
                profile: Some(PricingProfile::ConfiguredCreature),
                relation: Some(ClanRelation::OutOfClan),
                affiliation_name: "Synthetic affiliation".into(),
                day: "2026-01-02".into(),
                available_from: "2026-01-03".into(),
                next_session_reference: "Synthetic next session".into(),
                actor: "Synthetic ST".into(),
                reason: "Synthetic buy".into(),
                policy_version: "fixture-v1".into(),
                policy_reference: "Synthetic policy".into(),
                affiliation_evidence: "Synthetic affiliation".into(),
                approval_reference: "Synthetic approval".into(),
                eligibility_evidence: "Synthetic eligibility".into(),
                teacher_rule: Some(TeacherRule::Required),
                teacher_evidence: "Synthetic teacher".into(),
                standard_cost_confirmed: true,
            };
            let quote = purchases::quote_from_catalogue(
                "synthetic-library",
                2,
                &record,
                &request,
                &powers,
                &rules,
            )
            .unwrap();
            assert_eq!(quote.price.target_list(), target_list);
            assert_eq!(quote.price.price.total, "4");
            assert_eq!(purchases::record_hash(&record).unwrap(), original_hash);
            let after = purchases::apply(&record, &quote, 100).unwrap();
            assert_eq!(
                after.private_fields[target_list]["Traits"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            let receipt = purchases::receipt(&quote, 3);
            assert!(purchases::findings(&after, &receipt).is_empty());
            let cancel = CancellationRequest {
                id: uuid::Uuid::new_v4().to_string(),
                day: "2026-01-03".into(),
                actor: "Synthetic ST".into(),
                reason: "Synthetic refund".into(),
                evidence: "Synthetic correction".into(),
            };
            let cancel_quote =
                cancellations::quote("synthetic-library", 3, &after, &receipt, &cancel).unwrap();
            let restored = cancellations::apply(&after, &cancel_quote, &receipt, 101).unwrap();
            assert_eq!(
                xp::decimal(xp::replay(restored.xp_ledger.as_ref().unwrap()).unwrap().1),
                "20"
            );
            assert_eq!(
                restored.private_fields.get(target_list),
                record.private_fields.get(target_list)
            );
            assert_eq!(
                restored.public_fields.get(target_list),
                record.public_fields.get(target_list)
            );
        }
    }
}

#[test]
fn reviewed_profiles_require_bounded_valid_sources_and_unambiguous_lists() {
    let original = serde_json::to_value(creature_profiles::builtin()).unwrap();
    for bad_source in [
        json!({"book":"","edition":"Fixture","system":"Test","printed_page":1,"pdf_page_1based":2,"original_pdf_sha256":"a".repeat(64)}),
        json!({"book":"Fixture","edition":"Fixture","system":"Test","printed_page":0,"pdf_page_1based":2,"original_pdf_sha256":"a".repeat(64)}),
    ] {
        let mut data = original.clone();
        data["profiles"][1]["rules_reviewed"] = json!(true);
        data["profiles"][1]["cost_source"] = bad_source;
        assert!(creature_profiles::Catalogue::from_json(&data.to_string()).is_err());
    }
    let mut data = original.clone();
    data["profiles"][1]["rules_reviewed"] = json!(true);
    assert!(creature_profiles::Catalogue::from_json(&data.to_string()).is_err());
    let mut data = original;
    data["profiles"][1]["secondary_power_list"] = data["profiles"][1]["primary_power_list"].clone();
    assert!(creature_profiles::Catalogue::from_json(&data.to_string()).is_err());
}
