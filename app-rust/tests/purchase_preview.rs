use grapevine_workbench::pricing::{ClanRelation, MetTier, PricingProfile};
use grapevine_workbench::purchase_preview::{
    ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID, AnimalismPower, AnimalismPurchasePreview,
    animalism_purchase_preview,
};

fn preview(
    power_id: &str,
    relation: ClanRelation,
) -> grapevine_workbench::model::Result<
    grapevine_workbench::purchase_preview::AnimalismPurchasePreview,
> {
    animalism_purchase_preview(
        "Vampire",
        Some(power_id),
        Some(PricingProfile::MetLawsOfTheNightRevised),
        Some(relation),
    )
}

#[test]
fn both_basic_powers_have_stable_identity_sources_and_explicit_cost_cases() {
    for (power, id, name, position) in [
        (
            AnimalismPower::FeralWhispers,
            ANIMALISM_FERAL_WHISPERS_ID,
            "Feral Whispers",
            1,
        ),
        (
            AnimalismPower::Beckoning,
            ANIMALISM_BECKONING_ID,
            "Beckoning",
            2,
        ),
    ] {
        for (relation, total) in [(ClanRelation::InClan, "3"), (ClanRelation::OutOfClan, "4")] {
            let result = preview(id, relation).unwrap();
            assert_eq!(result.power, power);
            assert_eq!(
                result.power_source.inspected_text_path,
                "source:met.lotn-revised.text"
            );
            assert_eq!(
                result.learning_source.inspected_text_path,
                "source:met.lotn-revised.text"
            );
            assert_eq!(result.power_id, id);
            assert_eq!(result.name, name);
            assert_eq!(result.tier, MetTier::Basic);
            assert_eq!(result.position_in_basic_animalism, position);
            assert_eq!(result.power_source.citation.printed_page, 134);
            assert_eq!(result.power_source.citation.pdf_page_1based, 135);
            assert_eq!(result.learning_source.citation.printed_page, 133);
            assert_eq!(result.learning_source.citation.pdf_page_1based, 134);
            assert_eq!(result.power_source.citation.book, "Laws of the Night");
            assert_eq!(result.power_source.citation.edition, "Revised");
            assert_eq!(
                result.power_source.inspected_text_sha256,
                "6e6cfc9577c7a4bb3005cacdfaa529092c1c4d517ba3dfcd1cbfa2767a352ba0"
            );
            assert_eq!(
                result.power_source.citation.original_pdf_sha256,
                result.price.citation.original_pdf_sha256
            );
            assert_eq!(
                result.learning_source.citation.original_pdf_sha256,
                result.price.citation.original_pdf_sha256
            );
            assert_eq!(result.price.total, total);
            assert_eq!(result.price.citation.printed_page, 124);
            assert_eq!(result.price.citation.pdf_page_1based, 125);
            assert!(!result.price.library_prompt.is_empty());
            assert!(!result.purchase_enabled);
            assert!(!result.price.purchase_enabled);
        }
    }
}

#[test]
fn missing_or_unknown_power_and_missing_pricing_inputs_fail_closed() {
    assert_eq!(
        animalism_purchase_preview(
            "Vampire",
            None,
            Some(PricingProfile::MetLawsOfTheNightRevised),
            Some(ClanRelation::InClan),
        )
        .unwrap_err()
        .code,
        "ANIMALISM_POWER_REQUIRED"
    );
    assert_eq!(
        animalism_purchase_preview(
            "Vampire",
            Some("animalism.basic.unreviewed"),
            Some(PricingProfile::MetLawsOfTheNightRevised),
            Some(ClanRelation::InClan),
        )
        .unwrap_err()
        .code,
        "UNKNOWN_ANIMALISM_POWER"
    );
    assert_eq!(
        animalism_purchase_preview(
            "Vampire",
            Some(ANIMALISM_FERAL_WHISPERS_ID),
            None,
            Some(ClanRelation::InClan),
        )
        .unwrap_err()
        .code,
        "PRICING_PROFILE_REQUIRED"
    );
    assert_eq!(
        animalism_purchase_preview(
            "Vampire",
            Some(ANIMALISM_FERAL_WHISPERS_ID),
            Some(PricingProfile::MetLawsOfTheNightRevised),
            None,
        )
        .unwrap_err()
        .code,
        "PRICING_AFFILIATION_UNKNOWN"
    );
    assert!(serde_json::from_str::<PricingProfile>("\"UnknownProfile\"").is_err());
    assert!(serde_json::from_str::<ClanRelation>("\"UnknownAffiliation\"").is_err());
}

#[test]
fn unsupported_character_kind_is_rejected_by_the_vampire_pricing_scope() {
    assert_eq!(
        animalism_purchase_preview(
            "Werewolf",
            Some(ANIMALISM_FERAL_WHISPERS_ID),
            Some(PricingProfile::MetLawsOfTheNightRevised),
            Some(ClanRelation::InClan),
        )
        .unwrap_err()
        .code,
        "UNSUPPORTED_PRICING_SCOPE"
    );
}

#[test]
fn no_ordering_prerequisite_or_teacher_decision_is_invented() {
    let result = preview(ANIMALISM_BECKONING_ID, ClanRelation::InClan).unwrap();
    assert!(result.stated_power_prerequisites.is_empty());
    assert!(
        result
            .prerequisite_note
            .contains("not treated as a required purchase sequence")
    );
    let teacher = result
        .unresolved_gates
        .iter()
        .find(|gate| gate.id == "teacher_interpretation")
        .unwrap();
    assert!(teacher.explanation.contains("need only Experience Traits"));
    assert!(teacher.explanation.contains("requires a willing teacher"));
    for gate in [
        "local_chronicle_adoption",
        "affiliation_evidence",
        "storyteller_approval",
        "reviewed_portuguese",
    ] {
        assert!(result.unresolved_gates.iter().any(|entry| entry.id == gate));
    }
    let roundtrip =
        serde_json::from_str::<AnimalismPurchasePreview>(&serde_json::to_string(&result).unwrap())
            .unwrap();
    assert_eq!(roundtrip, result);
}
