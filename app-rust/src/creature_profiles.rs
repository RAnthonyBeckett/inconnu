//! Shared, bounded creature configuration. Proposed defaults are not sourced rules.
use crate::{
    model::{Result, error},
    sheet,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    sync::OnceLock,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostBand {
    pub max_next: i64,
    pub cost: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreeTraitCosts {
    pub attribute: i64,
    pub ability: i64,
    pub background: i64,
    pub primary_power: i64,
    pub secondary_power: i64,
    pub specialization: i64,
    pub permanent_resource: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub kind: String,
    pub identity_fields: Vec<String>,
    pub primary_power_list: String,
    pub secondary_power_list: String,
    pub attribute_priorities: [i64; 3],
    pub ability_base: i64,
    pub background_base: i64,
    pub free_traits_base: i64,
    pub max_negative_credit: i64,
    pub max_flaw_credit: i64,
    pub basic_power_base: i64,
    pub trait_xp_costs: BTreeMap<String, Vec<CostBand>>,
    pub power_tier_costs: BTreeMap<String, i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_power_tier_costs: Option<BTreeMap<String, i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_trait_costs: Option<FreeTraitCosts>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub starting_resources: BTreeMap<String, i64>,
    #[serde(default)]
    pub cost_source: Option<crate::pricing::SourceCitation>,
    pub rules_reviewed: bool,
    pub rules_note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub version: u32,
    pub profiles: Vec<Profile>,
}
fn invalid() -> crate::model::AppError {
    error(
        "CREATURE_PROFILE_INVALID",
        "Creature configuration is invalid; no character or XP was changed.",
    )
}
impl Catalogue {
    pub fn from_json(text: &str) -> Result<Self> {
        if text.len() > 1024 * 1024 {
            return Err(invalid());
        }
        let value: Self = serde_json::from_str(text).map_err(|_| invalid())?;
        if value.version != 1 || value.profiles.is_empty() || value.profiles.len() > 64 {
            return Err(invalid());
        }
        let mut kinds = HashSet::new();
        for p in &value.profiles {
            if p.free_trait_costs.as_ref().is_some_and(|costs| {
                [
                    costs.attribute,
                    costs.ability,
                    costs.background,
                    costs.primary_power,
                    costs.secondary_power,
                    costs.specialization,
                    costs.permanent_resource,
                ]
                .iter()
                .any(|cost| !(0..=4096).contains(cost))
            }) || p.starting_resources.len() > 64
                || p.starting_resources.iter().any(|(key, value)| {
                    !sheet::RESOURCE_FIELDS.contains(&key.as_str())
                        || key.starts_with("Temp")
                        || !(0..=4096).contains(value)
                })
                || p.secondary_power_tier_costs.as_ref().is_some_and(|costs| {
                    costs.len() != 5
                        || [
                            "Basic",
                            "Intermediate",
                            "Advanced",
                            "in_affiliation_adj",
                            "out_of_affiliation_adj",
                        ]
                        .iter()
                        .any(|key| {
                            costs
                                .get(*key)
                                .is_none_or(|cost| !(0..=4096).contains(cost))
                        })
                })
            {
                return Err(invalid());
            }
            if p.cost_source.is_some() {
                p.checked_cost_source()?;
            }
            if p.rules_reviewed && p.kind != "Vampire" && p.cost_source.is_none() {
                return Err(invalid());
            }
            if p.primary_power_list == p.secondary_power_list
                || p.power_tier_costs.len() > 16
                || p.identity_fields.iter().collect::<HashSet<_>>().len() != p.identity_fields.len()
                || p.identity_fields
                    .iter()
                    .any(|field| !sheet::IDENTITY_FIELDS.contains(&field.as_str()))
                || p.trait_xp_costs.keys().any(|key| {
                    key.is_empty()
                        || key.len() > 64
                        || !key.bytes().all(|b| b.is_ascii_alphanumeric())
                })
            {
                return Err(invalid());
            }
            if !sheet::is_character_kind(&p.kind)
                || !kinds.insert(&p.kind)
                || !sheet::LIST_FIELDS.contains(&p.primary_power_list.as_str())
                || !sheet::LIST_FIELDS.contains(&p.secondary_power_list.as_str())
                || p.identity_fields.len() > 64
                || p.identity_fields.iter().any(|f| {
                    f.is_empty() || f.len() > 64 || !f.bytes().all(|b| b.is_ascii_alphanumeric())
                })
                || p.attribute_priorities
                    .iter()
                    .chain([
                        &p.ability_base,
                        &p.background_base,
                        &p.free_traits_base,
                        &p.max_negative_credit,
                        &p.max_flaw_credit,
                        &p.basic_power_base,
                    ])
                    .any(|n| !(0..=4096).contains(n))
                || p.rules_note.len() > 16000
                || p.trait_xp_costs.len() > 64
            {
                return Err(invalid());
            }
            for bands in p.trait_xp_costs.values() {
                if bands.is_empty() || bands.len() > 32 {
                    return Err(invalid());
                }
                let mut last = 0;
                for band in bands {
                    if band.max_next <= last
                        || band.max_next > i16::MAX as i64
                        || !(0..=4096).contains(&band.cost)
                    {
                        return Err(invalid());
                    }
                    last = band.max_next;
                }
            }
            if [
                "Basic",
                "Intermediate",
                "Advanced",
                "in_affiliation_adj",
                "out_of_affiliation_adj",
            ]
            .iter()
            .any(|key| {
                p.power_tier_costs
                    .get(*key)
                    .is_none_or(|n| !(0..=4096).contains(n))
            }) {
                return Err(invalid());
            }
        }
        Ok(value)
    }
    pub fn profile(&self, kind: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.kind == kind)
    }
}
impl Profile {
    pub fn checked_cost_source(&self) -> Result<crate::pricing::SourceCitation> {
        self.cost_source
            .clone()
            .filter(|source| {
                [&source.book, &source.edition, &source.system]
                    .iter()
                    .all(|s| {
                        !s.trim().is_empty() && s.len() <= 512 && !s.chars().any(char::is_control)
                    })
                    && (1..=10000).contains(&source.printed_page)
                    && (1..=10000).contains(&source.pdf_page_1based)
                    && source.original_pdf_sha256.len() == 64
                    && source
                        .original_pdf_sha256
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit())
            })
            .ok_or_else(|| {
                error(
                    "CREATURE_COST_SOURCE_REQUIRED",
                    "A valid reviewed cost-source citation is required.",
                )
            })
    }
    pub fn validate_snapshot(&self) -> Result<()> {
        let snapshot = Catalogue {
            version: 1,
            profiles: vec![self.clone()],
        };
        Catalogue::from_json(&serde_json::to_string(&snapshot)?).map(|_| ())
    }
    pub fn owns_power_list(&self, field: &str) -> bool {
        field == self.primary_power_list || field == self.secondary_power_list
    }
    pub fn trait_cost(&self, key: &str, current: i64) -> Result<i64> {
        if !self.rules_reviewed {
            return Err(error(
                "CREATURE_RULE_REVIEW_REQUIRED",
                "Review this creature's configured rules before quoting XP.",
            ));
        }
        let next = current
            .checked_add(1)
            .filter(|n| *n > 0)
            .ok_or_else(invalid)?;
        self.trait_xp_costs
            .get(key)
            .and_then(|bands| bands.iter().find(|b| next <= b.max_next))
            .map(|b| b.cost)
            .ok_or_else(|| {
                error(
                    "CREATURE_COST_UNSUPPORTED",
                    "No reviewed cost band covers this trait and count.",
                )
            })
    }
}
pub fn builtin() -> &'static Catalogue {
    static VALUE: OnceLock<Catalogue> = OnceLock::new();
    VALUE.get_or_init(|| {
        Catalogue::from_json(include_str!("../data/creature_profiles.json"))
            .expect("validated bundled creature profiles")
    })
}
pub fn profile(kind: &str) -> Option<&'static Profile> {
    builtin().profile(kind)
}
