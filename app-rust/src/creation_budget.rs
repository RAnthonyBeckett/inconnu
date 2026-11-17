//! Pure arithmetic preview for the published MET Revised Vampire creation pools.
use crate::model::{Result, error};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Input {
    pub attribute_totals: [i64; 3],
    pub priorities: [usize; 3],
    pub ability_total: i64,
    pub background_total: i64,
    pub basic_power_count: i64,
    pub sabbat: bool,
    pub negative_traits: [i64; 3],
    pub optional_derangement: bool,
    pub lower_morality: bool,
    pub flaw_credit: i64,
    pub merit_cost: i64,
    pub specialization_count: i64,
    pub virtue_allocated: [i64; 3],
    pub virtue_free: [i64; 3],
    pub conviction: bool,
    pub instinct: bool,
    pub morality_free: i64,
    pub willpower_free: i64,
}
impl Default for Input {
    fn default() -> Self {
        Self {
            attribute_totals: [0; 3],
            priorities: [0, 1, 2],
            ability_total: 0,
            background_total: 0,
            basic_power_count: 0,
            sabbat: false,
            negative_traits: [0; 3],
            optional_derangement: false,
            lower_morality: false,
            flaw_credit: 0,
            merit_cost: 0,
            specialization_count: 0,
            virtue_allocated: [0; 3],
            virtue_free: [0; 3],
            conviction: false,
            instinct: false,
            morality_free: 0,
            willpower_free: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    /// Physical, Social, Mental, Abilities, Backgrounds, Basic powers, Virtue allocation.
    pub base_remaining: [i64; 7],
    pub free_available: i64,
    pub free_spent: i64,
    pub free_remaining: i64,
    /// Final Conscience/Conviction, Self-Control/Instinct, Courage values.
    pub virtues: [i64; 3],
    pub morality: i64,
    pub complete: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}

/// Quote pool leftovers and Free Trait spending without changing a character sheet.
pub fn quote(input: &Input) -> Result<Quote> {
    let values = [
        input.attribute_totals[0],
        input.attribute_totals[1],
        input.attribute_totals[2],
        input.ability_total,
        input.background_total,
        input.basic_power_count,
        input.negative_traits[0],
        input.negative_traits[1],
        input.negative_traits[2],
        input.flaw_credit,
        input.merit_cost,
        input.specialization_count,
        input.virtue_allocated[0],
        input.virtue_allocated[1],
        input.virtue_allocated[2],
        input.virtue_free[0],
        input.virtue_free[1],
        input.virtue_free[2],
        input.morality_free,
        input.willpower_free,
    ];
    if values.iter().any(|v| !(0..=4096).contains(v)) {
        return Err(error(
            "INVALID_CREATION_INPUT",
            "Creation values must be between 0 and 4096.",
        ));
    }
    let mut ranks = input.priorities;
    ranks.sort_unstable();
    if ranks != [0, 1, 2] {
        return Err(error(
            "INVALID_ATTRIBUTE_PRIORITIES",
            "Attribute priorities must be a unique ordering of 0, 1, and 2.",
        ));
    }
    if input.flaw_credit > 7 || input.merit_cost > 7 {
        return Err(error(
            "CREATION_CREDIT_LIMIT",
            "Flaw credit and Merit cost are each limited to seven Traits.",
        ));
    }
    let allocated: i64 = input.virtue_allocated.iter().sum();
    if allocated > 7 {
        return Err(error(
            "VIRTUE_ALLOCATION_LIMIT",
            "Virtue allocation cannot exceed seven Traits.",
        ));
    }
    let negative_credit: i64 = input.negative_traits.iter().sum::<i64>()
        + if input.optional_derangement { 2 } else { 0 }
        + if input.lower_morality { 2 } else { 0 };
    if negative_credit > 5 {
        return Err(error(
            "NEGATIVE_TRAIT_CREDIT_LIMIT",
            "Negative Trait, optional derangement, and Morality-loss credits share a five-Trait maximum.",
        ));
    }

    let mut attribute_base = [0; 3];
    for i in 0..3 {
        attribute_base[i] = [7, 5, 3][input.priorities[i]];
    }
    let (background_base, basic_base) = if input.sabbat { (0, 4) } else { (5, 3) };
    let mut remaining = [0; 7];
    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    for (i, name) in ["Physical", "Social", "Mental"].iter().enumerate() {
        remaining[i] = (attribute_base[i] - input.attribute_totals[i]).max(0);
        if remaining[i] > 0 {
            issues.push(format!(
                "{name} Attributes are short by {} initial Trait(s).",
                remaining[i]
            ));
        }
        if input.negative_traits[i] > 3 {
            warnings.push(format!(
                "{name} Negative Traits exceed the recommended three in this Attribute category."
            ));
        }
    }
    remaining[3] = (5 - input.ability_total).max(0);
    remaining[4] = (background_base - input.background_total).max(0);
    remaining[5] = (basic_base - input.basic_power_count).max(0);
    for (name, n) in [
        ("Abilities", remaining[3]),
        ("Backgrounds", remaining[4]),
        ("Basic powers", remaining[5]),
    ] {
        if n > 0 {
            issues.push(format!("{name} are short by {n} initial Trait(s)."));
        }
    }
    remaining[6] = 7 - allocated;
    if remaining[6] > 0 {
        issues.push(format!(
            "Virtue allocation is short by {} Trait(s).",
            remaining[6]
        ));
    }
    if input.conviction && input.virtue_allocated[0] == 0 {
        issues.push("Conviction requires at least one allocated Virtue Trait.".into());
    }
    if input.instinct && input.virtue_allocated[1] == 0 {
        issues.push("Instinct requires at least one allocated Virtue Trait.".into());
    }

    let baseline = [
        if input.conviction { 0 } else { 1 },
        if input.instinct { 0 } else { 1 },
        1,
    ];
    let virtues =
        std::array::from_fn(|i| input.virtue_allocated[i] + baseline[i] + input.virtue_free[i]);
    for (i, value) in virtues.iter().enumerate() {
        if !(1..=5).contains(value) {
            issues.push(format!(
                "Final Virtue {} must be between one and five Traits.",
                i + 1
            ));
        }
    }
    let morality_penalty = if input.lower_morality { 1 } else { 0 };
    let morality = (virtues[0] + virtues[1] + 1) / 2 - morality_penalty + input.morality_free;
    if !(1..=5).contains(&morality) {
        issues.push("Final Morality must be between one and five Traits.".into());
    }

    let free_available = 5 + negative_credit + input.flaw_credit;
    let free_spent = input
        .attribute_totals
        .iter()
        .zip(attribute_base)
        .map(|(n, b)| (*n - b).max(0))
        .sum::<i64>()
        + (input.ability_total - 5).max(0)
        + (input.background_total - background_base).max(0)
        + (input.basic_power_count - basic_base).max(0) * 3
        + input.specialization_count
        + input.virtue_free.iter().sum::<i64>() * 2
        + input.morality_free * 3
        + input.willpower_free * 3
        + input.merit_cost;
    let free_remaining = free_available - free_spent;
    if free_remaining < 0 {
        issues.push(format!(
            "Free Trait spending exceeds the available pool by {} Trait(s).",
            -free_remaining
        ));
    }
    Ok(Quote {
        base_remaining: remaining,
        free_available,
        free_spent,
        free_remaining,
        virtues,
        morality,
        complete: issues.is_empty(),
        issues,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn balanced(sabbat: bool) -> Input {
        Input {
            attribute_totals: [7, 5, 3],
            priorities: [0, 1, 2],
            ability_total: 5,
            background_total: if sabbat { 0 } else { 5 },
            basic_power_count: if sabbat { 4 } else { 3 },
            sabbat,
            virtue_allocated: [3, 2, 2],
            ..Input::default()
        }
    }
    #[test]
    fn ordinary_and_sabbat_pools_balance() {
        for sabbat in [false, true] {
            let q = quote(&balanced(sabbat)).unwrap();
            assert!(q.complete, "{:?}", q.issues);
            assert_eq!(q.base_remaining, [0; 7]);
            assert_eq!((q.free_spent, q.free_remaining), (0, 5));
        }
    }
    #[test]
    fn allocated_virtues_add_free_baselines() {
        let q = quote(&balanced(false)).unwrap();
        assert_eq!(q.virtues, [4, 3, 3]);
        assert_eq!((q.morality, q.base_remaining[6]), (4, 0));
    }
    #[test]
    fn unused_pool_does_not_pay_another_pool() {
        let mut input = balanced(false);
        input.attribute_totals = [6, 5, 3];
        input.ability_total = 6;
        let q = quote(&input).unwrap();
        assert!(!q.complete);
        assert_eq!(
            (q.base_remaining[0], q.free_spent, q.free_remaining),
            (1, 1, 4)
        );
    }
    #[test]
    fn negative_credit_sources_share_five_trait_cap() {
        let mut input = balanced(false);
        input.negative_traits = [1, 0, 0];
        input.optional_derangement = true;
        input.lower_morality = true;
        assert_eq!(quote(&input).unwrap().free_available, 10);
        input.negative_traits = [2, 0, 0];
        assert!(quote(&input).is_err());
    }
    #[test]
    fn all_free_trait_prices_are_counted() {
        let mut input = balanced(false);
        input.attribute_totals = [8, 6, 4];
        input.ability_total = 6;
        input.background_total = 6;
        input.basic_power_count = 4;
        input.specialization_count = 1;
        input.virtue_free = [1, 0, 0];
        input.morality_free = 1;
        input.willpower_free = 1;
        input.merit_cost = 1;
        input.flaw_credit = 7;
        input.negative_traits = [4, 1, 0];
        let q = quote(&input).unwrap();
        assert_eq!(
            (q.free_available, q.free_spent, q.free_remaining),
            (17, 18, -1)
        );
        assert!(!q.complete);
        assert_eq!(q.warnings.len(), 1);
    }
    #[test]
    fn invalid_priorities_and_values_fail() {
        let mut input = balanced(false);
        input.priorities = [0, 0, 2];
        assert!(quote(&input).is_err());
        input = balanced(false);
        input.willpower_free = 4097;
        assert!(quote(&input).is_err());
        input = balanced(false);
        input.flaw_credit = 8;
        assert!(quote(&input).is_err());
    }
}
