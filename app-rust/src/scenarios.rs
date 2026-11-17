//! Read-only what-if XP totals for reviewed Discipline power previews.
//!
//! This module quotes only source-qualified powers supported by
//! `purchase_preview`; it never reads or writes a character, ledger, or sheet.
use crate::model::{Result, error};
use crate::pricing::{ClanRelation, PricingProfile};
use crate::purchase_preview::AnimalismPurchasePreview;
use crate::xp;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioChoice {
    /// A reviewed source-qualified ID, not a name imported from a character sheet.
    pub power_id: String,
    /// Must be supplied explicitly for each selected power.
    pub relation: Option<ClanRelation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioPreview {
    /// Each entry retains its checked power and learning citations and unresolved gates.
    pub items: Vec<AnimalismPurchasePreview>,
    pub total_spend: String,
    pub available: Option<String>,
    pub projected: Option<String>,
    pub shortfall: Option<String>,
    /// A scenario never authorizes or applies a purchase.
    pub purchase_enabled: bool,
}

/// Calculate a what-if cart from explicit choices and an optional exact XP balance.
/// A missing balance stays unknown. No character, imported power name, clan, or
/// unspent XP baseline is inferred here.
pub fn preview(
    character_kind: &str,
    available: Option<&str>,
    profile: Option<PricingProfile>,
    choices: &[ScenarioChoice],
    owned_power_ids: &[String],
) -> Result<ScenarioPreview> {
    preview_inner(
        character_kind,
        available,
        profile,
        choices,
        owned_power_ids,
        crate::power_catalogue::builtin(),
        crate::creature_profiles::builtin(),
    )
}

/// Calculate a read-only scenario using explicitly supplied, validated catalogues.
/// This is also the pure test seam for future configured-creature entries; it does not
/// install an override or modify the bundled catalogues.
pub fn preview_from_catalogues(
    character_kind: &str,
    available: Option<&str>,
    profile: Option<PricingProfile>,
    choices: &[ScenarioChoice],
    owned_power_ids: &[String],
    powers: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<ScenarioPreview> {
    if choices.is_empty() && character_kind == "Vampire" {
        return preview_inner(
            character_kind,
            available,
            profile,
            choices,
            owned_power_ids,
            powers,
            profiles,
        );
    }
    let powers = crate::power_catalogue::Catalogue::from_json(&serde_json::to_string(powers)?)?;
    let profiles =
        crate::creature_profiles::Catalogue::from_json(&serde_json::to_string(profiles)?)?;
    preview_inner(
        character_kind,
        available,
        profile,
        choices,
        owned_power_ids,
        &powers,
        &profiles,
    )
}

fn preview_inner(
    character_kind: &str,
    available: Option<&str>,
    profile: Option<PricingProfile>,
    choices: &[ScenarioChoice],
    owned_power_ids: &[String],
    powers: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<ScenarioPreview> {
    if !crate::sheet::is_character_kind(character_kind) {
        return Err(error(
            "UNSUPPORTED_PRICING_SCOPE",
            "This scenario supports reviewed Vampire Discipline powers only.",
        ));
    }

    // Empty Vampire scenarios remain the legacy zero-cost preview. Any selected entry,
    // including a JSON-only Vampire definition, uses the shared catalogue preview.
    if choices.is_empty() && character_kind != "Vampire" {
        let matching_entry = powers
            .entries
            .iter()
            .any(|entry| entry.template_scope == character_kind);
        if !matching_entry {
            return Err(error(
                "UNSUPPORTED_PRICING_SCOPE",
                "This scenario supports reviewed Vampire Discipline powers only.",
            ));
        }
        let rules = profiles
            .profile(character_kind)
            .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
        if !rules.rules_reviewed {
            return Err(error(
                "CREATURE_RULE_REVIEW_REQUIRED",
                "Review the creature rules, source and local adoption before quoting.",
            ));
        }
        rules.checked_cost_source()?;
    }

    let available_units = available.map(xp::amount).transpose()?;
    if available_units.is_some_and(|units| units < 0) {
        return Err(error(
            "SCENARIO_AVAILABLE_XP_NEGATIVE",
            "Available XP must be zero or greater; enter an explicit balance or leave it unknown.",
        ));
    }

    let mut seen = HashSet::with_capacity(choices.len());
    let mut items = Vec::with_capacity(choices.len());
    let mut spend_units = 0_i64;
    for choice in choices {
        if !seen.insert(choice.power_id.as_str()) {
            return Err(error(
                "SCENARIO_DUPLICATE_POWER",
                "Choose each source-qualified Discipline power at most once.",
            ));
        }
        if owned_power_ids
            .iter()
            .any(|owned| owned == &choice.power_id)
        {
            return Err(error(
                "SCENARIO_POWER_ALREADY_OWNED",
                "A selected source-qualified power is explicitly listed as already owned.",
            ));
        }

        let definition = powers.lookup(&choice.power_id).ok_or_else(|| {
            error(
                "UNKNOWN_ANIMALISM_POWER",
                "The source-reviewed catalogue entry is unavailable.",
            )
        })?;
        let rules = profiles
            .profile(character_kind)
            .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
        let item = crate::purchase_preview::preview_from_definition(
            character_kind,
            definition,
            profile,
            choice.relation,
            rules,
        )?;
        let base_units = xp::amount(&item.price.base)?;
        let adjustment_units = xp::amount(&item.price.affiliation_adjustment)?;
        let item_total_units = xp::amount(&item.price.total)?;
        let components_units = base_units.checked_add(adjustment_units).ok_or_else(|| {
            error(
                "SCENARIO_XP_OVERFLOW",
                "Scenario cost exceeds exact XP limits.",
            )
        })?;
        if components_units != item_total_units {
            return Err(error(
                "SCENARIO_COST_MISMATCH",
                "The reviewed preview total does not equal its base plus affiliation adjustment.",
            ));
        }
        spend_units = spend_units.checked_add(item_total_units).ok_or_else(|| {
            error(
                "SCENARIO_XP_OVERFLOW",
                "Scenario cost exceeds exact XP limits.",
            )
        })?;
        items.push(item);
    }

    let projected_units = available_units
        .map(|balance| {
            balance
                .checked_sub(spend_units)
                .ok_or_else(|| error("SCENARIO_XP_OVERFLOW", "Projected XP exceeds exact limits."))
        })
        .transpose()?;
    let shortfall_units = projected_units
        .filter(|projected| *projected < 0)
        .map(|projected| {
            projected
                .checked_neg()
                .ok_or_else(|| error("SCENARIO_XP_OVERFLOW", "XP shortfall exceeds exact limits."))
        })
        .transpose()?;

    Ok(ScenarioPreview {
        items,
        total_spend: xp::decimal(spend_units),
        available: available_units.map(xp::decimal),
        projected: projected_units.map(xp::decimal),
        shortfall: shortfall_units.map(xp::decimal),
        purchase_enabled: false,
    })
}
