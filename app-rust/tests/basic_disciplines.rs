use grapevine_workbench::{
    model::Result,
    pricing::{ClanRelation, MetTier, PricingProfile},
    purchase_preview::{
        ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID, AnimalismPower,
        AnimalismPurchasePreview, animalism_purchase_preview,
    },
};

fn preview(power_id: &str, relation: ClanRelation) -> Result<AnimalismPurchasePreview> {
    animalism_purchase_preview(
        "Vampire",
        Some(power_id),
        Some(PricingProfile::MetLawsOfTheNightRevised),
        Some(relation),
    )
}

#[test]
fn sixteen_basic_powers_have_stable_ids_families_order_and_title_pages() {
    let expected = [
        (
            AnimalismPower::FeralWhispers,
            ANIMALISM_FERAL_WHISPERS_ID,
            "Feral Whispers",
            "Animalism",
            134,
        ),
        (
            AnimalismPower::Beckoning,
            ANIMALISM_BECKONING_ID,
            "Beckoning",
            "Animalism",
            134,
        ),
        (
            AnimalismPower::HeightenedSenses,
            "met.lotn-revised.auspex.basic.heightened_senses",
            "Heightened Senses",
            "Auspex",
            137,
        ),
        (
            AnimalismPower::AuraPerception,
            "met.lotn-revised.auspex.basic.aura_perception",
            "Aura Perception",
            "Auspex",
            137,
        ),
        (
            AnimalismPower::Alacrity,
            "met.lotn-revised.celerity.basic.alacrity",
            "Alacrity",
            "Celerity",
            141,
        ),
        (
            AnimalismPower::Swiftness,
            "met.lotn-revised.celerity.basic.swiftness",
            "Swiftness",
            "Celerity",
            141,
        ),
        (
            AnimalismPower::Command,
            "met.lotn-revised.dominate.basic.command",
            "Command",
            "Dominate",
            146,
        ),
        (
            AnimalismPower::Mesmerism,
            "met.lotn-revised.dominate.basic.mesmerism",
            "Mesmerism",
            "Dominate",
            146,
        ),
        (
            AnimalismPower::Endurance,
            "met.lotn-revised.fortitude.basic.endurance",
            "Endurance",
            "Fortitude",
            149,
        ),
        (
            AnimalismPower::Mettle,
            "met.lotn-revised.fortitude.basic.mettle",
            "Mettle",
            "Fortitude",
            149,
        ),
        (
            AnimalismPower::CloakOfShadows,
            "met.lotn-revised.obfuscate.basic.cloak_of_shadows",
            "Cloak of Shadows",
            "Obfuscate",
            161,
        ),
        (
            AnimalismPower::UnseenPresence,
            "met.lotn-revised.obfuscate.basic.unseen_presence",
            "Unseen Presence",
            "Obfuscate",
            161,
        ),
        (
            AnimalismPower::Prowess,
            "met.lotn-revised.potence.basic.prowess",
            "Prowess",
            "Potence",
            166,
        ),
        (
            AnimalismPower::Might,
            "met.lotn-revised.potence.basic.might",
            "Might",
            "Potence",
            166,
        ),
        (
            AnimalismPower::Awe,
            "met.lotn-revised.presence.basic.awe",
            "Awe",
            "Presence",
            167,
        ),
        (
            AnimalismPower::DreadGaze,
            "met.lotn-revised.presence.basic.dread_gaze",
            "Dread Gaze",
            "Presence",
            167,
        ),
    ];
    assert_eq!(
        &AnimalismPower::ALL[..16],
        expected.map(|entry| entry.0).as_slice()
    );

    let ids = expected.map(|entry| entry.1);
    for (index, (power, id, name, family, printed_page)) in expected.into_iter().enumerate() {
        assert_eq!(power.stable_id(), id);
        assert_eq!(power.name(), name);
        assert_eq!(power.family(), family);
        assert_eq!(power.printed_page(), printed_page);
        assert_eq!(power.position(), index as u8 + 1);
        assert_eq!(power.position_in_basic_animalism(), index as u8 + 1);
        assert_eq!(
            power,
            serde_json::from_str(&serde_json::to_string(&power).unwrap()).unwrap()
        );

        for (relation, total) in [(ClanRelation::InClan, "3"), (ClanRelation::OutOfClan, "4")] {
            let result = preview(id, relation).unwrap();
            assert_eq!(result.power, power);
            assert_eq!(result.power_id, id);
            assert_eq!(result.name, name);
            assert_eq!(result.tier, MetTier::Basic);
            assert_eq!(result.position_in_basic_animalism, index as u8 + 1);
            assert_eq!(result.power_source.citation.printed_page, printed_page);
            assert_eq!(
                result.power_source.citation.pdf_page_1based,
                printed_page + 1
            );
            assert_eq!(result.learning_source.citation.printed_page, 133);
            assert_eq!(result.learning_source.citation.pdf_page_1based, 134);
            assert_eq!(result.price.base, "3");
            assert_eq!(
                result.price.affiliation_adjustment,
                if relation == ClanRelation::InClan {
                    "0"
                } else {
                    "1"
                }
            );
            assert_eq!(result.price.total, total);
            assert_eq!(result.price.citation.printed_page, 124);
            assert_eq!(result.price.citation.pdf_page_1based, 125);
            assert!(result.stated_power_prerequisites.is_empty());
            assert!(!result.purchase_enabled);
            assert!(!result.price.purchase_enabled);
            for gate in [
                "teacher_interpretation",
                "local_chronicle_adoption",
                "affiliation_evidence",
                "local_eligibility",
                "storyteller_approval",
                "reviewed_portuguese",
            ] {
                assert!(
                    result.unresolved_gates.iter().any(|entry| entry.id == gate),
                    "{family} missing {gate}"
                );
            }
        }
    }
    assert_eq!(
        ids.iter().collect::<std::collections::HashSet<_>>().len(),
        16
    );
}

#[test]
fn old_animalism_ids_and_serialized_enum_names_remain_unchanged() {
    assert_eq!(
        ANIMALISM_FERAL_WHISPERS_ID,
        "met.lotn-revised.animalism.basic.feral_whispers"
    );
    assert_eq!(
        ANIMALISM_BECKONING_ID,
        "met.lotn-revised.animalism.basic.beckoning"
    );
    assert_eq!(
        serde_json::to_string(&AnimalismPower::FeralWhispers).unwrap(),
        "\"FeralWhispers\""
    );
    assert_eq!(
        serde_json::to_string(&AnimalismPower::Beckoning).unwrap(),
        "\"Beckoning\""
    );
}

#[test]
fn auspex_teacher_list_omission_stays_an_additional_unresolved_gate() {
    let result = preview(
        "met.lotn-revised.auspex.basic.heightened_senses",
        ClanRelation::InClan,
    )
    .unwrap();
    let teacher = result
        .unresolved_gates
        .iter()
        .find(|gate| gate.id == "teacher_interpretation")
        .unwrap();
    assert!(
        teacher
            .explanation
            .contains("earlier cardinal-eight list includes Auspex")
    );
    assert!(teacher.explanation.contains("names seven families"));
    assert!(
        result
            .unresolved_gates
            .iter()
            .any(|gate| gate.id == "auspex_teacher_list_omission")
    );

    let celerity = preview(
        "met.lotn-revised.celerity.basic.alacrity",
        ClanRelation::InClan,
    )
    .unwrap();
    assert!(
        !celerity
            .unresolved_gates
            .iter()
            .any(|gate| gate.id == "auspex_teacher_list_omission")
    );
}

#[test]
fn use_conditions_do_not_become_power_purchase_prerequisites() {
    let swiftness = preview(
        "met.lotn-revised.celerity.basic.swiftness",
        ClanRelation::InClan,
    )
    .unwrap();
    assert!(swiftness.stated_power_prerequisites.is_empty());
    assert!(
        swiftness
            .prerequisite_note
            .contains("does not state that Alacrity must be purchased first")
    );

    let unseen = preview(
        "met.lotn-revised.obfuscate.basic.unseen_presence",
        ClanRelation::InClan,
    )
    .unwrap();
    assert!(unseen.stated_power_prerequisites.is_empty());
    assert!(
        unseen
            .prerequisite_note
            .contains("does not state that Cloak of Shadows must be purchased first")
    );
}

#[test]
fn missing_unknown_or_case_changed_ids_fail_closed() {
    assert_eq!(
        animalism_purchase_preview(
            "Vampire",
            None,
            Some(PricingProfile::MetLawsOfTheNightRevised),
            Some(ClanRelation::InClan)
        )
        .unwrap_err()
        .code,
        "ANIMALISM_POWER_REQUIRED"
    );
    for id in [
        "animalism.basic.unreviewed",
        "MET.lotn-revised.celerity.basic.alacrity",
        "met.lotn-revised.presence.basic.Dread_Gaze",
    ] {
        assert_eq!(
            animalism_purchase_preview(
                "Vampire",
                Some(id),
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(ClanRelation::InClan)
            )
            .unwrap_err()
            .code,
            "UNKNOWN_ANIMALISM_POWER"
        );
    }
}
