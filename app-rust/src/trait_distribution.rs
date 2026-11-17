//! Descriptive distributions of exact stored values for a named trait.
//!
//! These counts summarize local storage only. They do not infer rules, ratings,
//! legality, approval, or character quality.

use crate::{
    model::{Result, error},
    trait_statistics::Character,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_CHARACTERS: usize = 10_000;
const MAX_ENTRIES_PER_CHARACTER: usize = 2_000;
const MAX_MATCHED_OCCURRENCES: usize = 200_000;
const MAX_TEXT_BYTES: usize = 4_096;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub list: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub characters: usize,
    pub matched_characters: usize,
    pub missing_list_characters: usize,
    pub unmatched_characters: usize,
    pub occurrences: usize,
    pub known_occurrences: usize,
    pub unknown_occurrences: usize,
    pub ratings: Vec<Rating>,
    pub members: Vec<Member>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rating {
    /// Canonical unsigned decimal value; `None` groups unparseable values last.
    pub value: Option<String>,
    /// Number of input character rows represented at this rating.
    pub characters: usize,
    /// Number of matching entries at this rating, including duplicates.
    pub occurrences: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    pub family: String,
    /// Original selected-list values for matching entries, in source order.
    pub values: Vec<String>,
}

#[derive(Default)]
struct RatingCounts {
    characters: usize,
    occurrences: usize,
}

fn invalid() -> crate::model::AppError {
    error(
        "TRAIT_DISTRIBUTION_INVALID",
        "The selected trait distribution input is invalid or exceeds its limits.",
    )
}

fn valid_text(value: &str) -> bool {
    value.len() <= MAX_TEXT_BYTES && !value.contains('\0')
}

fn validate_request(request: &Request) -> Result<String> {
    if !crate::sheet::LIST_FIELDS.contains(&request.list.as_str())
        || request.name.trim().is_empty()
        || !valid_text(&request.name)
    {
        return Err(invalid());
    }
    Ok(request.name.trim().to_lowercase())
}

fn numeric_value(value: &str) -> Option<u128> {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.as_bytes().iter().all(u8::is_ascii_digit) {
        return None;
    }
    trimmed.parse().ok()
}

/// Builds a bounded distribution for the exact selected list and trait name.
///
/// Values are reported as stored. Only trimmed ASCII unsigned integers that fit
/// in `u128` enter numeric buckets; every other value is retained in one unknown
/// bucket and is never treated as zero.
pub fn build(characters: &[Character], request: &Request) -> Result<Report> {
    let normalized_name = validate_request(request)?;
    if characters.len() > MAX_CHARACTERS {
        return Err(invalid());
    }

    let mut report = Report {
        characters: characters.len(),
        matched_characters: 0,
        missing_list_characters: 0,
        unmatched_characters: 0,
        occurrences: 0,
        known_occurrences: 0,
        unknown_occurrences: 0,
        ratings: Vec::new(),
        members: Vec::new(),
    };
    let mut known_ratings = BTreeMap::<u128, RatingCounts>::new();
    let mut unknown_rating = RatingCounts::default();

    for character in characters {
        if !valid_text(&character.name)
            || character.name.trim().is_empty()
            || !valid_text(&character.family)
        {
            return Err(invalid());
        }

        let Some(entries) = character.lists.get(&request.list) else {
            report.missing_list_characters += 1;
            continue;
        };
        if entries.len() > MAX_ENTRIES_PER_CHARACTER {
            return Err(invalid());
        }

        let mut values = Vec::new();
        let mut character_ratings = BTreeSet::new();
        for entry in entries {
            if !valid_text(&entry.name) || entry.name.trim().is_empty() || !valid_text(&entry.value)
            {
                return Err(invalid());
            }
            if entry.name.trim().to_lowercase() != normalized_name {
                continue;
            }
            if report.occurrences >= MAX_MATCHED_OCCURRENCES {
                return Err(invalid());
            }

            report.occurrences += 1;
            values.push(entry.value.clone());
            if let Some(value) = numeric_value(&entry.value) {
                report.known_occurrences += 1;
                known_ratings.entry(value).or_default().occurrences += 1;
                character_ratings.insert(Some(value));
            } else {
                report.unknown_occurrences += 1;
                unknown_rating.occurrences += 1;
                character_ratings.insert(None);
            }
        }

        if values.is_empty() {
            report.unmatched_characters += 1;
            continue;
        }

        report.matched_characters += 1;
        for value in character_ratings {
            match value {
                Some(value) => known_ratings.entry(value).or_default().characters += 1,
                None => unknown_rating.characters += 1,
            }
        }
        report.members.push(Member {
            name: character.name.clone(),
            family: character.family.clone(),
            values,
        });
    }

    report.ratings = known_ratings
        .into_iter()
        .map(|(value, counts)| Rating {
            value: Some(value.to_string()),
            characters: counts.characters,
            occurrences: counts.occurrences,
        })
        .collect();
    if unknown_rating.occurrences > 0 {
        report.ratings.push(Rating {
            value: None,
            characters: unknown_rating.characters,
            occurrences: unknown_rating.occurrences,
        });
    }

    debug_assert_eq!(
        report.characters,
        report.matched_characters + report.missing_list_characters + report.unmatched_characters
    );
    debug_assert_eq!(
        report.occurrences,
        report.known_occurrences + report.unknown_occurrences
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trait_statistics::Entry;

    fn character(name: &str, family: &str, lists: &[(&str, &[(&str, &str)])]) -> Character {
        Character {
            name: name.into(),
            family: family.into(),
            lists: lists
                .iter()
                .map(|(list, entries)| {
                    (
                        (*list).into(),
                        entries
                            .iter()
                            .map(|(trait_name, value)| Entry {
                                name: (*trait_name).into(),
                                value: (*value).into(),
                            })
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    fn request(list: &str, name: &str) -> Request {
        Request {
            list: list.into(),
            name: name.into(),
        }
    }

    #[test]
    fn numeric_aliases_sort_canonically_and_unknown_values_stay_unknown() {
        let maximum = u128::MAX.to_string();
        let overflow = format!("{}0", maximum);
        let characters = [character(
            "Mira",
            "Mage",
            &[(
                "InfluenceList",
                &[
                    ("Resources", "000"),
                    ("resources", "0002"),
                    (" RESOURCES ", "2"),
                    ("Resources", &maximum),
                    ("Resources", &overflow),
                    ("Resources", "-1"),
                    ("Resources", "+2"),
                    ("Resources", "1.5"),
                    ("Resources", "٢"),
                    ("Resources", "  3 \t"),
                ],
            )],
        )];

        let report = build(&characters, &request("InfluenceList", " resources ")).unwrap();
        assert_eq!(report.characters, 1);
        assert_eq!(report.matched_characters, 1);
        assert_eq!(report.occurrences, 10);
        assert_eq!(report.known_occurrences, 5);
        assert_eq!(report.unknown_occurrences, 5);
        assert_eq!(
            report
                .ratings
                .iter()
                .map(|rating| rating.value.clone())
                .collect::<Vec<_>>(),
            [
                Some("0".into()),
                Some("2".into()),
                Some("3".into()),
                Some(maximum),
                None,
            ]
        );
        assert_eq!(report.ratings[1].characters, 1);
        assert_eq!(report.ratings[1].occurrences, 2);
        assert_eq!(report.ratings[4].characters, 1);
        assert_eq!(report.ratings[4].occurrences, 5);
        assert_eq!(report.members[0].values[1], "0002");
        assert_eq!(report.members[0].values[6], "+2");
    }

    #[test]
    fn unicode_case_and_trimmed_names_match_exactly() {
        let characters = [
            character("Mira", "Mage", &[("MeritList", &[("  ÉCLAIR ", "2")])]),
            character("Sera", "Mage", &[("MeritList", &[("éclair", "3")])]),
            character("Nia", "Mage", &[("MeritList", &[("Eclair", "4")])]),
        ];
        let report = build(&characters, &request("MeritList", " Éclair ")).unwrap();
        assert_eq!(report.matched_characters, 2);
        assert_eq!(report.unmatched_characters, 1);
        assert_eq!(report.occurrences, 2);
        assert_eq!(report.members[0].values, ["2"]);
        assert_eq!(report.members[1].values, ["3"]);
    }

    #[test]
    fn duplicate_names_and_values_keep_rows_distinct_and_count_each_occurrence() {
        let characters = [
            character(
                "Same name",
                "Mage",
                &[("InfluenceList", &[("Resources", "02"), ("Resources", "2")])],
            ),
            character(
                "Same name",
                "Vampire",
                &[("InfluenceList", &[("Resources", "2"), ("Resources", "4")])],
            ),
        ];
        let report = build(&characters, &request("InfluenceList", "Resources")).unwrap();
        assert_eq!(report.characters, 2);
        assert_eq!(report.matched_characters, 2);
        assert_eq!(report.occurrences, 4);
        assert_eq!(report.ratings[0].value.as_deref(), Some("2"));
        assert_eq!(report.ratings[0].characters, 2);
        assert_eq!(report.ratings[0].occurrences, 3);
        assert_eq!(report.ratings[1].value.as_deref(), Some("4"));
        assert_eq!(report.ratings[1].characters, 1);
        assert_eq!(report.members.len(), 2);
        assert_eq!(report.members[0].name, "Same name");
        assert_eq!(report.members[0].family, "Mage");
        assert_eq!(report.members[0].values, ["02", "2"]);
        assert_eq!(report.members[1].name, "Same name");
        assert_eq!(report.members[1].family, "Vampire");
        assert_eq!(report.members[1].values, ["2", "4"]);
    }

    #[test]
    fn missing_empty_and_unmatched_lists_are_separate_and_conserve_rows() {
        let characters = [
            character("Missing", "Mage", &[("MeritList", &[("Resources", "1")])]),
            character("Empty", "Mage", &[("InfluenceList", &[])]),
            character(
                "Unmatched",
                "Mage",
                &[("InfluenceList", &[("Status", "1")])],
            ),
            character(
                "Matched",
                "Mage",
                &[("InfluenceList", &[("Resources", "0")])],
            ),
        ];
        let report = build(&characters, &request("InfluenceList", "Resources")).unwrap();
        assert_eq!(report.characters, 4);
        assert_eq!(report.matched_characters, 1);
        assert_eq!(report.missing_list_characters, 1);
        assert_eq!(report.unmatched_characters, 2);
        assert_eq!(
            report.characters,
            report.matched_characters
                + report.missing_list_characters
                + report.unmatched_characters
        );
        assert_eq!(report.members[0].name, "Matched");

        let empty = build(&[], &request("InfluenceList", "Resources")).unwrap();
        assert_eq!(empty.characters, 0);
        assert_eq!(empty.matched_characters, 0);
        assert!(empty.ratings.is_empty());
        assert!(empty.members.is_empty());
    }

    #[test]
    fn only_selected_list_is_validated_and_input_is_not_mutated() {
        let mut ignored_entries = Vec::new();
        for _ in 0..2_001 {
            ignored_entries.push(Entry {
                name: String::new(),
                value: "\0ignored".into(),
            });
        }
        let mut character = character(
            "Mira",
            "Mage",
            &[("InfluenceList", &[("Resources", "0003")])],
        );
        character
            .lists
            .insert("UnselectedList".into(), ignored_entries);
        let original = character.clone();

        let report = build(&[character.clone()], &request("InfluenceList", "Resources")).unwrap();
        assert_eq!(report.ratings[0].value.as_deref(), Some("3"));
        assert_eq!(character, original);
    }

    #[test]
    fn rejects_unknown_lists_blank_names_and_invalid_selected_source_text() {
        let source = [character(
            "Mira",
            "Mage",
            &[("InfluenceList", &[("Resources", "1")])],
        )];
        assert!(build(&source, &request("influencelist", "Resources")).is_err());
        assert!(build(&source, &request("InfluenceList", "  ")).is_err());
        assert!(build(&source, &request("InfluenceList", &"x".repeat(4_097))).is_err());

        let mut bad_entry = source[0].clone();
        bad_entry.lists.get_mut("InfluenceList").unwrap()[0].name = "\0".into();
        assert!(build(&[bad_entry], &request("InfluenceList", "Resources")).is_err());

        let mut bad_family = source[0].clone();
        bad_family.family = "\0".into();
        assert!(build(&[bad_family], &request("InfluenceList", "Resources")).is_err());

        let mut too_many_entries = source[0].clone();
        too_many_entries
            .lists
            .get_mut("InfluenceList")
            .unwrap()
            .resize(
                MAX_ENTRIES_PER_CHARACTER + 1,
                Entry {
                    name: "Resources".into(),
                    value: "1".into(),
                },
            );
        assert!(build(&[too_many_entries], &request("InfluenceList", "Resources")).is_err());
    }

    #[test]
    fn enforces_character_and_matched_occurrence_limits() {
        let empty_character = character("Mira", "Mage", &[]);
        let too_many_characters = vec![empty_character.clone(); MAX_CHARACTERS + 1];
        assert!(build(&too_many_characters, &request("InfluenceList", "Resources")).is_err());

        let entries = (0..MAX_ENTRIES_PER_CHARACTER)
            .map(|_| Entry {
                name: "Resources".into(),
                value: "1".into(),
            })
            .collect::<Vec<_>>();
        let one_character = Character {
            name: "Mira".into(),
            family: "Mage".into(),
            lists: BTreeMap::from([("InfluenceList".into(), entries)]),
        };
        let too_many_occurrences = vec![one_character; 101];
        assert!(
            build(
                &too_many_occurrences,
                &request("InfluenceList", "Resources")
            )
            .is_err()
        );
    }
}
