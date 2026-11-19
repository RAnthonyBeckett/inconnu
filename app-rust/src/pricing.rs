//! Book-specific price previews. No automatic purchase or approval consumes these values.
use crate::model::{Result, error};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PricingProfile {
    MetLawsOfTheNightRevised,
    ConfiguredCreature,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetTier {
    Basic,
    Intermediate,
    Advanced,
}
impl MetTier {
    /// Existing sheet wire values; never infer a tier from a display label or order.
    pub const fn sheet_value(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Intermediate => "intermediate",
            Self::Advanced => "advanced",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClanRelation {
    InClan,
    OutOfClan,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PurchaseFamily {
    Discipline,
    Ritual,
    Other,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCitation {
    pub book: String,
    pub edition: String,
    pub system: String,
    pub printed_page: u32,
    pub pdf_page_1based: u32,
    pub original_pdf_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostPreview {
    pub rule_id: String,
    pub profile: PricingProfile,
    pub base: String,
    pub affiliation_adjustment: String,
    pub total: String,
    pub citation: SourceCitation,
    pub library_prompt: String,
    pub reviewed_portuguese_equivalent: Option<String>,
    pub purchase_enabled: bool,
    pub coverage: String,
}

/// Preview only the printed MET Revised Vampire discipline table. Affiliation is an
/// explicit input, not inferred from names or the current sheet. Unknown inputs fail.
/// This does not validate identity, prerequisites, teacher, history, exceptions, policy
/// adoption, approval, Portuguese review, balance, or a trait grant. No storage writes.
pub fn discipline_preview(
    character_kind: &str,
    family: PurchaseFamily,
    profile: Option<PricingProfile>,
    tier: Option<MetTier>,
    relation: Option<ClanRelation>,
) -> Result<CostPreview> {
    if character_kind != "Vampire" || family != PurchaseFamily::Discipline {
        return Err(error(
            "UNSUPPORTED_PRICING_SCOPE",
            "This table is a Vampire discipline preview; other templates and ritual/path-specific cases require their own reviewed rules.",
        ));
    }
    let profile = profile.ok_or_else(|| {
        error(
            "PRICING_PROFILE_REQUIRED",
            "Select an applicable rules profile; no default prices are assumed.",
        )
    })?;
    let tier = tier.ok_or_else(|| error("PRICING_TIER_UNKNOWN", "The MET tier must be established from source; menu position or tabletop dots do not establish it."))?;
    let relation = relation.ok_or_else(|| error("PRICING_AFFILIATION_UNKNOWN", "Establish affiliation at the proposed purchase before quoting; a base rate cannot be assumed."))?;
    let rules = crate::creature_profiles::profile(character_kind)
        .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
    let base = rules.power_tier_costs[match tier {
        MetTier::Basic => "Basic",
        MetTier::Intermediate => "Intermediate",
        MetTier::Advanced => "Advanced",
    }];
    let adjustment = rules.power_tier_costs[match relation {
        ClanRelation::InClan => "in_affiliation_adj",
        ClanRelation::OutOfClan => "out_of_affiliation_adj",
    }];
    Ok(CostPreview {
        rule_id: "met-lotn-revised.printed124.new-discipline.v1".into(),
        profile,
        base: base.to_string(), affiliation_adjustment: adjustment.to_string(), total: (base + adjustment).to_string(),
        citation: SourceCitation {
            book: "Laws of the Night".into(), edition: "Revised".into(), system: "Mind’s Eye Theatre".into(),
            printed_page:124, pdf_page_1based:125,
            original_pdf_sha256: "f6eae9fda388c51e23bfe73338c867123655d5efa7957020857da7c370bac574".into(),
        },
        library_prompt: "[For more detailed info check your library for \"Laws of the Night (Revised).\"]".into(),
        reviewed_portuguese_equivalent: None,
        purchase_enabled:false,
        coverage:"Printed-book price preview only. Actual OWbN/chronicle adoption, affiliation evidence, prerequisites, exceptions, approval and Portuguese review remain unresolved. No XP is debited and no trait is granted.".into(),
    })
}

/// Shared JSON rules path; unknown/proposed profiles never silently inherit Vampire costs.
pub fn configured_preview(
    kind: &str,
    family: PurchaseFamily,
    tier: MetTier,
    relation: ClanRelation,
    rules: &crate::creature_profiles::Profile,
) -> Result<CostPreview> {
    configured_preview_for_list(
        kind,
        family,
        tier,
        relation,
        rules,
        &rules.primary_power_list,
    )
}
pub fn configured_preview_for_list(
    kind: &str,
    family: PurchaseFamily,
    tier: MetTier,
    relation: ClanRelation,
    rules: &crate::creature_profiles::Profile,
    target_list: &str,
) -> Result<CostPreview> {
    configured_power_price(kind, family, tier, relation, rules, target_list, None)
}

pub(crate) fn configured_power_price(
    kind: &str,
    family: PurchaseFamily,
    tier: MetTier,
    relation: ClanRelation,
    rules: &crate::creature_profiles::Profile,
    target_list: &str,
    fixed_xp_cost: Option<i64>,
) -> Result<CostPreview> {
    rules.validate_snapshot()?;
    if !rules.owns_power_list(target_list) {
        return Err(error(
            "CREATURE_COST_UNSUPPORTED",
            "This power list does not belong to the configured profile.",
        ));
    }
    if kind != rules.kind || !crate::sheet::is_character_kind(kind) || !rules.rules_reviewed {
        return Err(error(
            "CREATURE_RULE_REVIEW_REQUIRED",
            "Review the creature rules, source and local adoption before quoting.",
        ));
    }
    let citation = rules.checked_cost_source()?;
    let tier_key = match tier {
        MetTier::Basic => "Basic",
        MetTier::Intermediate => "Intermediate",
        MetTier::Advanced => "Advanced",
    };
    let costs = if target_list == rules.secondary_power_list {
        rules
            .secondary_power_tier_costs
            .as_ref()
            .unwrap_or(&rules.power_tier_costs)
    } else {
        &rules.power_tier_costs
    };
    let (base, adjustment) = if let Some(fixed) = fixed_xp_cost {
        if !(1..=4096).contains(&fixed) {
            return Err(error(
                "CREATURE_COST_UNSUPPORTED",
                "Invalid fixed XP price.",
            ));
        }
        (fixed, 0)
    } else {
        let base = *costs
            .get(tier_key)
            .ok_or_else(|| error("CREATURE_COST_UNSUPPORTED", "No configured tier price."))?;
        let adjustment = *costs
            .get(match relation {
                ClanRelation::InClan => "in_affiliation_adj",
                ClanRelation::OutOfClan => "out_of_affiliation_adj",
            })
            .ok_or_else(|| {
                error(
                    "CREATURE_COST_UNSUPPORTED",
                    "No configured affiliation price.",
                )
            })?;
        let adjustment = if kind == "Mage"
            && target_list == "SphereList"
            && citation.edition == "MET_Laws_of_Ascension"
        {
            adjustment
                * match tier {
                    MetTier::Basic => 1,
                    MetTier::Intermediate => 2,
                    MetTier::Advanced => 3,
                }
        } else {
            adjustment
        };
        (base, adjustment)
    };
    let price_suffix = if fixed_xp_cost.is_some() {
        ".fixed"
    } else {
        ""
    };
    let list_suffix = if target_list == rules.secondary_power_list {
        ".secondary"
    } else {
        ""
    };
    Ok(CostPreview{rule_id:format!("configured.{}.{family:?}{list_suffix}{price_suffix}.v1",kind.to_ascii_lowercase()),profile:PricingProfile::ConfiguredCreature,base:base.to_string(),affiliation_adjustment:adjustment.to_string(),total:(base+adjustment).to_string(),library_prompt:format!("[For more detailed info check your library for \"{}.\"]",citation.book),citation,reviewed_portuguese_equivalent:None,purchase_enabled:false,coverage:"Configured source-reviewed creature price; local policy, eligibility, teacher and approval declarations are still required.".into()})
}
