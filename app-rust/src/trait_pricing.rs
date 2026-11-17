use crate::model::{Result, error};
use crate::pricing::{PricingProfile, SourceCitation};
use serde::{Deserialize, Serialize};

const LIBRARY_PROMPT: &str =
    "[For more detailed info check your library for \"Laws of the Night (Revised).\"]";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Attribute,
    Ability,
    Background,
    HumanityPath,
    Virtue,
    Willpower,
    PermanentResource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub category: Category,
    pub current: i64,
    pub next: i64,
    pub cost: String,
    pub citation: SourceCitation,
    pub library_prompt: String,
    pub requires_review: bool,
    pub coverage: String,
}

/// Preview one printed Vampire MET Revised advancement price; grants and approvals are separate.
pub fn preview(
    kind: &str,
    profile: Option<PricingProfile>,
    category: Category,
    current: i64,
) -> Result<Quote> {
    if profile == Some(PricingProfile::ConfiguredCreature) {
        return configured_preview(kind, category, current);
    }
    if kind != "Vampire" {
        return Err(error(
            "UNSUPPORTED_PRICING_SCOPE",
            "This table is scoped to Vampire characters.",
        ));
    }
    match profile {
        None => {
            return Err(error(
                "PRICING_PROFILE_REQUIRED",
                "Select an applicable rules profile.",
            ));
        }
        Some(PricingProfile::MetLawsOfTheNightRevised) => {}
        Some(PricingProfile::ConfiguredCreature) => {
            return configured_preview(kind, category, current);
        }
    }
    if current < 0 {
        return Err(error(
            "INVALID_TRAIT_COUNT",
            "The current count must be nonnegative.",
        ));
    }
    if category == Category::Ability && current >= 10 {
        return Err(error(
            "ABILITY_TABLE_BAND_LIMIT",
            "No quote is available beyond the printed Ability band 1–10.",
        ));
    }
    if current >= i16::MAX as i64 {
        return Err(error(
            "TRAIT_COUNT_LIMIT",
            "The next count exceeds i16 storage.",
        ));
    }
    let next = current + 1;
    let (_legacy_cost, requires_review, price_scope) = match category {
        Category::Attribute => (1, false, "1 XP per new trait"),
        Category::Ability if next <= 5 => {
            (1, false, "1 XP for resulting counts 1–5; 2 XP for 6–10")
        }
        Category::Ability => (2, false, "1 XP for resulting counts 1–5; 2 XP for 6–10"),
        Category::Background => (
            1,
            true,
            "1 XP per trait; explicit Storyteller approval is required",
        ),
        Category::HumanityPath => (2, true, "2 XP per trait; Storyteller approval is required"),
        Category::Virtue => (3, true, "3 XP per trait; storyline support is required"),
        Category::Willpower => (3, false, "3 XP per trait"),
        Category::PermanentResource => {
            return Err(error(
                "CREATURE_COST_UNSUPPORTED",
                "Choose a configured creature permanent resource.",
            ));
        }
    };
    let xp = crate::creature_profiles::profile(kind)
        .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?
        .trait_cost(&format!("{category:?}"), current)?;
    let limits = if category == Category::Ability {
        "Only the printed 1–10 table band is stated; no broader character maximum is claimed."
    } else {
        "No game maximum is claimed; counts are bounded only by i16 storage."
    };
    Ok(Quote {
        category,
        current,
        next,
        cost: xp.to_string(),
        citation: SourceCitation {
            book: "Laws of the Night".into(),
            edition: "Revised".into(),
            system: "Mind’s Eye Theatre".into(),
            printed_page: 124,
            pdf_page_1based: 125,
            original_pdf_sha256: "f6eae9fda388c51e23bfe73338c867123655d5efa7957020857da7c370bac574"
                .into(),
        },
        library_prompt: LIBRARY_PROMPT.into(),
        requires_review,
        coverage: format!(
            "Printed price only: {price_scope}. {limits} No unlisted prerequisites or approval conclusions are provided. Applies to XP advancement, not creation budgets."
        ),
    })
}

pub fn preview_resource(
    kind: &str,
    profile: Option<PricingProfile>,
    field: &str,
    current: i64,
) -> Result<Quote> {
    if profile != Some(PricingProfile::ConfiguredCreature)
        || !crate::sheet::RESOURCE_FIELDS.contains(&field)
        || field.starts_with("Temp")
    {
        return Err(error(
            "CREATURE_COST_UNSUPPORTED",
            "Choose a configured permanent resource.",
        ));
    }
    let rules = crate::creature_profiles::profile(kind)
        .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
    let cost = rules.trait_cost(field, current)?;
    let citation = rules.checked_cost_source()?;
    Ok(Quote {
        category: Category::PermanentResource,
        current,
        next: current + 1,
        cost: cost.to_string(),
        library_prompt: format!(
            "[For more detailed info check your library for \"{}.\"]",
            citation.book
        ),
        citation,
        requires_review: true,
        coverage: "Configured permanent resource; explicit source/local review required.".into(),
    })
}
fn configured_preview(kind: &str, category: Category, current: i64) -> Result<Quote> {
    let rules = crate::creature_profiles::profile(kind)
        .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
    let cost = rules.trait_cost(&format!("{category:?}"), current)?;
    let citation = rules.checked_cost_source()?;
    Ok(Quote {
        category,
        current,
        next: current + 1,
        cost: cost.to_string(),
        library_prompt: format!(
            "[For more detailed info check your library for \"{}.\"]",
            citation.book
        ),
        citation,
        requires_review: true,
        coverage:
            "Source-reviewed configured creature cost; explicit local review remains required."
                .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn quote(category: Category, current: i64) -> Result<Quote> {
        preview(
            "Vampire",
            Some(PricingProfile::MetLawsOfTheNightRevised),
            category,
            current,
        )
    }

    #[test]
    fn six_prices_review_flags_and_citation() {
        for (category, cost, review) in [
            (Category::Attribute, "1", false),
            (Category::Ability, "1", false),
            (Category::Background, "1", true),
            (Category::HumanityPath, "2", true),
            (Category::Virtue, "3", true),
            (Category::Willpower, "3", false),
        ] {
            let q = quote(category, 0).unwrap();
            assert_eq!(
                (q.current, q.next, q.cost.as_str(), q.requires_review),
                (0, 1, cost, review)
            );
            assert!(q.coverage.contains("not creation budgets"));
            assert_eq!(
                (
                    q.citation.book.as_str(),
                    q.citation.edition.as_str(),
                    q.citation.system.as_str(),
                    q.citation.printed_page,
                    q.citation.pdf_page_1based
                ),
                (
                    "Laws of the Night",
                    "Revised",
                    "Mind’s Eye Theatre",
                    124,
                    125
                )
            );
            assert_eq!(
                q.citation.original_pdf_sha256,
                "f6eae9fda388c51e23bfe73338c867123655d5efa7957020857da7c370bac574"
            );
        }
    }

    #[test]
    fn ability_uses_resulting_count_band() {
        assert_eq!(quote(Category::Ability, 4).unwrap().cost, "1");
        assert_eq!(quote(Category::Ability, 5).unwrap().cost, "2");
        let q = quote(Category::Ability, 9).unwrap();
        assert_eq!((q.current, q.next, q.cost.as_str()), (9, 10, "2"));
        assert!(quote(Category::Ability, 10).is_err());
    }

    #[test]
    fn unsupported_kind_profile_and_counts_fail_closed() {
        assert!(
            preview(
                "Mortal",
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Category::Attribute,
                0
            )
            .is_err()
        );
        assert!(preview("Vampire", None, Category::Attribute, 0).is_err());
        assert!(quote(Category::Attribute, -1).is_err());
        assert!(quote(Category::Willpower, i16::MAX as i64).is_err());
    }
}
