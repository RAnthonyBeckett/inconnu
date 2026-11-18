//! Callable trait totals from a scope-filtered sheet and resolved item inputs.
use crate::{
    house_rules::{
        Attribute, ChallengeFormula, HouseRulesPack, ModifierTarget, NamedChallenge, SourceKind,
        normalize,
    },
    model::{RecordView, Result, error},
    printing::{DocLanguage, Row, Section},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedItem {
    pub name: String,
    pub bonus: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breakdown {
    pub name: String,
    pub attribute: Attribute,
    pub ability: Option<String>,
    pub attribute_traits: i64,
    pub ability_traits: i64,
    pub modifiers: Vec<(String, i64)>,
    pub uncapped_total: i64,
    pub effective_cap: Option<i64>,
    pub callable_total: i64,
}

fn rows<'a>(view: &'a RecordView, list: &str) -> impl Iterator<Item = &'a Value> {
    view.fields
        .get(list)
        .and_then(|list| list["Traits"].as_array())
        .into_iter()
        .flatten()
}

fn has_trait(view: &RecordView, list: &str, key: &str) -> bool {
    rows(view, list).any(|row| {
        row["Name"]
            .as_str()
            .is_some_and(|name| normalize(name) == key)
    })
}

fn invalid_count() -> crate::model::AppError {
    error(
        "CHALLENGE_TRAITS_INVALID",
        "Review the sheet's trait counts before calculating a challenge pool.",
    )
}

fn count<'a>(rows: impl Iterator<Item = &'a Value>) -> Result<i64> {
    let mut total = 0_i64;
    for row in rows.filter(|row| {
        row["Name"]
            .as_str()
            .is_some_and(|name| !name.trim().is_empty())
    }) {
        let count = match &row["Value"] {
            Value::Null => 1,
            Value::String(value) if value.trim().is_empty() => 1,
            Value::String(value) => value.trim().parse().map_err(|_| invalid_count())?,
            value => value.as_i64().ok_or_else(invalid_count)?,
        };
        if !(0..=4096).contains(&count) {
            return Err(invalid_count());
        }
        total = total.checked_add(count).ok_or_else(invalid_count)?;
    }
    Ok(total)
}

pub fn calculate(
    view: &RecordView,
    items: &[LinkedItem],
    pack: Option<&HouseRulesPack>,
    owbn_bylaws_enabled: bool,
    challenge: &NamedChallenge,
) -> Result<Breakdown> {
    if let Some(pack) = pack {
        pack.validate()?;
    }
    calculate_validated(view, items, pack, owbn_bylaws_enabled, challenge)
}

fn calculate_validated(
    view: &RecordView,
    items: &[LinkedItem],
    pack: Option<&HouseRulesPack>,
    owbn_bylaws_enabled: bool,
    challenge: &NamedChallenge,
) -> Result<Breakdown> {
    let default_formula = ChallengeFormula::default();
    let formula = pack.map_or(&default_formula, |pack| &pack.challenge_formula);
    let name = normalize(&challenge.name);
    let attribute = challenge.attribute;
    let attribute_traits = count(rows(view, attribute.list()))?;
    let profile = crate::creature_profiles::profile(&view.kind);
    let has_power = |name: &str| {
        profile.is_some_and(|profile| {
            has_trait(view, &profile.primary_power_list, name)
                || has_trait(view, &profile.secondary_power_list, name)
        })
    };
    let mut modifiers = Vec::new();
    let mut ability_override: Option<&str> = None;
    let mut rules: Vec<_> = pack
        .into_iter()
        .flat_map(|pack| &pack.custom_modifiers)
        .collect();
    rules.sort_unstable_by_key(|rule| &rule.id);
    for rule in rules {
        let target_matches = match &rule.target {
            ModifierTarget::Physical => attribute == Attribute::Physical,
            ModifierTarget::Social => attribute == Attribute::Social,
            ModifierTarget::Mental => attribute == Attribute::Mental,
            ModifierTarget::NamedChallenge(target) => normalize(target) == name,
        };
        let key = normalize(&rule.match_name);
        if !target_matches
            || !match rule.source_kind {
                SourceKind::Merit => {
                    formula.include_merit_flaw_modifiers && has_trait(view, "MeritList", &key)
                }
                SourceKind::Flaw => {
                    formula.include_merit_flaw_modifiers && has_trait(view, "FlawList", &key)
                }
                SourceKind::Power => has_power(&key),
                SourceKind::Item => {
                    formula.include_linked_item_bonuses
                        && items.iter().any(|item| normalize(&item.name) == key)
                }
                SourceKind::Custom => key == name,
            }
        {
            continue;
        }
        if let Some(ability) = rule.ability_override.as_deref() {
            if ability_override.is_some_and(|current| normalize(current) != normalize(ability)) {
                return Err(error(
                    "CHALLENGE_ABILITY_CONFLICT",
                    "Matching challenge rules specify different Abilities.",
                ));
            }
            ability_override = Some(ability);
        }
        modifiers.push((rule.label.clone(), rule.delta));
    }
    if formula.include_linked_item_bonuses {
        let mut bonuses: Vec<_> = items
            .iter()
            .filter_map(|item| {
                item.bonus
                    .filter(|bonus| *bonus > 0)
                    .map(|bonus| (&item.name, bonus))
            })
            .collect();
        bonuses.sort_unstable();
        modifiers.extend(
            bonuses
                .into_iter()
                .map(|(name, bonus)| (name.clone(), bonus)),
        );
    }
    let ability = ability_override
        .or(challenge.ability.as_deref())
        .map(str::to_owned);
    let ability_traits = if let Some(ability) = ability
        .as_ref()
        .filter(|_| formula.include_ability_in_trait_call)
    {
        let key = normalize(ability);
        count(rows(view, "AbilityList").filter(|row| {
            row["Name"]
                .as_str()
                .is_some_and(|name| normalize(name) == key)
        }))?
    } else {
        0
    };
    let uncapped_total = modifiers.iter().try_fold(
        attribute_traits
            .checked_add(ability_traits)
            .ok_or_else(invalid_count)?,
        |total, (_, delta)| total.checked_add(*delta).ok_or_else(invalid_count),
    )?;
    let effective_cap = match (formula.max_trait_bid_cap, owbn_bylaws_enabled) {
        (Some(local), true) => Some(local.min(30)),
        (None, true) => Some(30),
        (local, false) => local,
    };
    let callable_total = effective_cap
        .map_or(uncapped_total, |cap| uncapped_total.min(cap))
        .max(0);
    Ok(Breakdown {
        name: challenge.name.clone(),
        attribute,
        ability,
        attribute_traits,
        ability_traits,
        modifiers,
        uncapped_total,
        effective_cap,
        callable_total,
    })
}

pub(crate) fn sheet_pools(
    view: &RecordView,
    items: &[LinkedItem],
    pack: Option<&HouseRulesPack>,
    owbn: bool,
) -> Result<Vec<Breakdown>> {
    Attribute::ALL
        .into_iter()
        .map(|attribute| NamedChallenge {
            name: attribute.label().into(),
            attribute,
            ability: None,
        })
        .chain(
            pack.into_iter()
                .flat_map(|pack| pack.named_challenges.iter().cloned()),
        )
        .map(|challenge| calculate_validated(view, items, pack, owbn, &challenge))
        .collect()
}

pub(crate) fn section(pools: &[Breakdown], language: DocLanguage) -> Section {
    let pt = language == DocLanguage::Portuguese;
    Section {
        title: if pt {
            "Paradas de desafio"
        } else {
            "Challenge Pools"
        }
        .into(),
        rows: pools
            .iter()
            .map(|pool| {
                let attribute = match (pool.attribute, pt) {
                    (Attribute::Physical, true) => "Físico",
                    (Attribute::Social, true) => "Social",
                    (Attribute::Mental, true) => "Mental",
                    (attribute, false) => attribute.label(),
                };
                let mut arithmetic = format!("{} {attribute}", pool.attribute_traits);
                if let Some(ability) = &pool.ability {
                    arithmetic.push_str(&format!(" + {} {ability}", pool.ability_traits));
                }
                for (label, delta) in &pool.modifiers {
                    arithmetic.push_str(&format!(" {delta:+} ({label})"));
                }
                arithmetic.push_str(&format!(" = {}", pool.uncapped_total));
                if let Some(cap) = pool.effective_cap {
                    let cap_label = if pt { "limite" } else { "cap" };
                    arithmetic.push_str(&format!(" ({cap_label} {cap})"));
                }
                let callable = if pt { "disponíveis" } else { "callable" };
                arithmetic.push_str(&format!(" → {} {callable}", pool.callable_total));
                Row {
                    name: if pool.name == pool.attribute.label() {
                        attribute.into()
                    } else {
                        pool.name.clone()
                    },
                    value: arithmetic,
                    note: None,
                }
            })
            .collect(),
    }
}
