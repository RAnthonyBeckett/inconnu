use grapevine_workbench::pricing::{ClanRelation, PricingProfile};
use grapevine_workbench::purchase_preview::{ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID};
use grapevine_workbench::scenarios::{ScenarioChoice, ScenarioPreview, preview};

fn choice(power_id: &str, relation: Option<ClanRelation>) -> ScenarioChoice {
    ScenarioChoice {
        power_id: power_id.into(),
        relation,
    }
}

fn evaluate(
    available: Option<&str>,
    profile: Option<PricingProfile>,
    choices: &[ScenarioChoice],
    owned_power_ids: &[String],
) -> grapevine_workbench::model::Result<ScenarioPreview> {
    preview("Vampire", available, profile, choices, owned_power_ids)
}

fn profile() -> Option<PricingProfile> {
    Some(PricingProfile::MetLawsOfTheNightRevised)
}

#[test]
fn two_explicit_powers_sum_to_seven_and_report_one_xp_shortfall() {
    let choices = [
        choice(ANIMALISM_FERAL_WHISPERS_ID, Some(ClanRelation::InClan)),
        choice(ANIMALISM_BECKONING_ID, Some(ClanRelation::OutOfClan)),
    ];
    let result = evaluate(Some("6"), profile(), &choices, &[]).unwrap();

    assert_eq!(result.items.len(), 2);
    assert_eq!(result.items[0].price.base, "3");
    assert_eq!(result.items[0].price.affiliation_adjustment, "0");
    assert_eq!(result.items[0].price.total, "3");
    assert_eq!(result.items[1].price.base, "3");
    assert_eq!(result.items[1].price.affiliation_adjustment, "1");
    assert_eq!(result.items[1].price.total, "4");
    assert_eq!(result.total_spend, "7");
    assert_eq!(result.available.as_deref(), Some("6"));
    assert_eq!(result.projected.as_deref(), Some("-1"));
    assert_eq!(result.shortfall.as_deref(), Some("1"));
    assert!(
        result
            .items
            .iter()
            .all(|item| !item.unresolved_gates.is_empty())
    );
    assert!(result.items.iter().all(|item| !item.purchase_enabled));
    assert!(!result.purchase_enabled);
}

#[test]
fn unknown_balance_stays_unknown_and_empty_cart_spends_zero() {
    let one_power = [choice(
        ANIMALISM_FERAL_WHISPERS_ID,
        Some(ClanRelation::InClan),
    )];
    let unknown = evaluate(None, profile(), &one_power, &[]).unwrap();
    assert_eq!(unknown.total_spend, "3");
    assert_eq!(unknown.available, None);
    assert_eq!(unknown.projected, None);
    assert_eq!(unknown.shortfall, None);

    let empty = evaluate(Some("0.5"), None, &[], &[]).unwrap();
    assert!(empty.items.is_empty());
    assert_eq!(empty.total_spend, "0");
    assert_eq!(empty.available.as_deref(), Some("0.5"));
    assert_eq!(empty.projected.as_deref(), Some("0.5"));
    assert_eq!(empty.shortfall, None);
    assert!(!empty.purchase_enabled);
}

#[test]
fn six_decimal_balance_is_preserved_without_rounding() {
    let choices = [choice(
        ANIMALISM_FERAL_WHISPERS_ID,
        Some(ClanRelation::InClan),
    )];
    let result = evaluate(Some("2.999999"), profile(), &choices, &[]).unwrap();

    assert_eq!(result.available.as_deref(), Some("2.999999"));
    assert_eq!(result.projected.as_deref(), Some("-0.000001"));
    assert_eq!(result.shortfall.as_deref(), Some("0.000001"));
}

#[test]
fn duplicate_and_explicitly_owned_source_power_ids_are_rejected() {
    let duplicate = [
        choice(ANIMALISM_FERAL_WHISPERS_ID, Some(ClanRelation::InClan)),
        choice(ANIMALISM_FERAL_WHISPERS_ID, Some(ClanRelation::OutOfClan)),
    ];
    assert_eq!(
        evaluate(None, profile(), &duplicate, &[]).unwrap_err().code,
        "SCENARIO_DUPLICATE_POWER"
    );

    let selected = [choice(
        ANIMALISM_FERAL_WHISPERS_ID,
        Some(ClanRelation::InClan),
    )];
    let owned = vec![ANIMALISM_FERAL_WHISPERS_ID.to_string()];
    assert_eq!(
        evaluate(None, profile(), &selected, &owned)
            .unwrap_err()
            .code,
        "SCENARIO_POWER_ALREADY_OWNED"
    );
}

#[test]
fn unknown_power_missing_profile_or_affiliation_and_wrong_kind_fail_closed() {
    let unknown = [choice("imported:Beckoning", Some(ClanRelation::InClan))];
    assert_eq!(
        evaluate(None, profile(), &unknown, &[]).unwrap_err().code,
        "UNKNOWN_ANIMALISM_POWER"
    );

    let selected = [choice(
        ANIMALISM_FERAL_WHISPERS_ID,
        Some(ClanRelation::InClan),
    )];
    assert_eq!(
        evaluate(None, None, &selected, &[]).unwrap_err().code,
        "PRICING_PROFILE_REQUIRED"
    );

    let unknown_relation = [choice(ANIMALISM_FERAL_WHISPERS_ID, None)];
    assert_eq!(
        evaluate(None, profile(), &unknown_relation, &[])
            .unwrap_err()
            .code,
        "PRICING_AFFILIATION_UNKNOWN"
    );

    assert_eq!(
        preview("Werewolf", None, profile(), &[], &[])
            .unwrap_err()
            .code,
        "UNSUPPORTED_PRICING_SCOPE"
    );
}

#[test]
fn invalid_or_negative_available_xp_is_rejected() {
    let choices = [choice(
        ANIMALISM_FERAL_WHISPERS_ID,
        Some(ClanRelation::InClan),
    )];
    assert_eq!(
        evaluate(Some("1.0000001"), profile(), &choices, &[])
            .unwrap_err()
            .code,
        "INVALID_XP"
    );
    assert_eq!(
        evaluate(Some("-0.25"), profile(), &choices, &[])
            .unwrap_err()
            .code,
        "SCENARIO_AVAILABLE_XP_NEGATIVE"
    );
}
