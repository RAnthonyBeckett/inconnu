//! Bounded, source-preserving aggregation for selected character trait lists.

use crate::model::{Result, error};
use std::collections::BTreeMap;

const MAX_CHARACTERS: usize = 10_000;
const MAX_ENTRIES_PER_CHARACTER: usize = 2_000;
const MAX_REPORT_OCCURRENCES: usize = 200_000;
const MAX_TEXT_BYTES: usize = 4_096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub name: String,
    pub family: String,
    pub lists: BTreeMap<String, Vec<Entry>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Influence,
    MeritsFlaws,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Number of input character rows, including separate same-name rows.
    pub characters: usize,
    pub buckets: Vec<Bucket>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bucket {
    /// Canonical source list key, such as `InfluenceList` or `MeritList`.
    pub list: String,
    /// Deterministic trimmed display name for the case-insensitive group.
    pub name: String,
    /// Number of input character rows represented in this bucket.
    pub characters: usize,
    /// Number of original entries in this bucket, including duplicates.
    pub occurrences: usize,
    /// Numeric counts are known-only storage statistics, not inferred ratings.
    pub known_numeric: usize,
    pub unknown_numeric: usize,
    /// Empty when there are no known numeric entries; otherwise an exact base-10 sum.
    pub known_sum: String,
    /// Maximum known unsigned integer value, or None if none parsed.
    pub known_max: Option<String>,
    /// One member per input character row, retaining each original value in order.
    pub members: Vec<Member>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub family: String,
    /// Original value strings, including duplicates and unparseable values.
    pub values: Vec<String>,
}

#[derive(Default)]
struct CharacterBucket {
    display_name: String,
    values: Vec<String>,
    known_numeric: usize,
    unknown_numeric: usize,
    known_sum: u128,
    known_max: Option<u128>,
}

#[derive(Default)]
struct BucketAccumulator {
    name: String,
    characters: usize,
    occurrences: usize,
    known_numeric: usize,
    unknown_numeric: usize,
    known_sum: u128,
    known_max: Option<u128>,
    members: Vec<Member>,
}

fn invalid() -> crate::model::AppError {
    error(
        "TRAIT_STATISTICS_INVALID",
        "The trait statistics input is invalid or exceeds its limits.",
    )
}

fn overflow() -> crate::model::AppError {
    error(
        "TRAIT_STATISTICS_OVERFLOW",
        "The known numeric total exceeds the supported storage range.",
    )
}

fn valid_text(value: &str) -> bool {
    value.len() <= MAX_TEXT_BYTES && !value.contains('\0')
}

pub(crate) fn selected_lists(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Influence => &["InfluenceList"],
        Kind::MeritsFlaws => &["MeritList", "FlawList"],
    }
}

fn numeric_value(value: &str) -> Option<u128> {
    if value.is_empty() || !value.as_bytes().iter().all(u8::is_ascii_digit) {
        return None;
    }
    value.parse().ok()
}

fn display_name_for_group(current: &str, candidate: &str) -> String {
    if current.is_empty() || candidate < current {
        candidate.to_owned()
    } else {
        current.to_owned()
    }
}

/// Builds exact known-only aggregates while retaining all selected source values.
pub fn build(characters: &[Character], kind: Kind) -> Result<Report> {
    if characters.len() > MAX_CHARACTERS {
        return Err(invalid());
    }

    let lists = selected_lists(kind);
    let mut total_occurrences = 0usize;
    let mut buckets: BTreeMap<(String, String), BucketAccumulator> = BTreeMap::new();

    for character in characters {
        if !valid_text(&character.name)
            || character.name.trim().is_empty()
            || !valid_text(&character.family)
        {
            return Err(invalid());
        }

        let mut selected_entry_count = 0usize;
        for list in lists {
            if let Some(entries) = character.lists.get(*list) {
                selected_entry_count = selected_entry_count
                    .checked_add(entries.len())
                    .ok_or_else(invalid)?;
            }
        }
        if selected_entry_count > MAX_ENTRIES_PER_CHARACTER {
            return Err(invalid());
        }
        total_occurrences = total_occurrences
            .checked_add(selected_entry_count)
            .filter(|count| *count <= MAX_REPORT_OCCURRENCES)
            .ok_or_else(invalid)?;

        let mut per_character: BTreeMap<(String, String), CharacterBucket> = BTreeMap::new();
        for list in lists {
            let Some(entries) = character.lists.get(*list) else {
                continue;
            };
            for entry in entries {
                if !valid_text(&entry.name)
                    || entry.name.trim().is_empty()
                    || !valid_text(&entry.value)
                {
                    return Err(invalid());
                }

                let display_name = entry.name.trim();
                let folded_name = display_name.to_lowercase();
                let key = ((*list).to_owned(), folded_name);
                let local = per_character.entry(key).or_default();
                local.display_name = display_name_for_group(&local.display_name, display_name);
                local.values.push(entry.value.clone());

                if let Some(value) = numeric_value(&entry.value) {
                    local.known_numeric += 1;
                    local.known_sum = local.known_sum.checked_add(value).ok_or_else(overflow)?;
                    local.known_max = Some(local.known_max.map_or(value, |max| max.max(value)));
                } else {
                    local.unknown_numeric += 1;
                }
            }
        }

        for (key, local) in per_character {
            let bucket = buckets.entry(key).or_default();
            bucket.name = display_name_for_group(&bucket.name, &local.display_name);
            bucket.characters += 1;
            bucket.occurrences += local.values.len();
            bucket.known_numeric += local.known_numeric;
            bucket.unknown_numeric += local.unknown_numeric;
            bucket.known_sum = bucket
                .known_sum
                .checked_add(local.known_sum)
                .ok_or_else(overflow)?;
            if let Some(value) = local.known_max {
                bucket.known_max = Some(bucket.known_max.map_or(value, |max| max.max(value)));
            }
            bucket.members.push(Member {
                name: character.name.clone(),
                family: character.family.clone(),
                values: local.values,
            });
        }
    }

    let buckets = buckets
        .into_iter()
        .map(|((list, _), bucket)| Bucket {
            list,
            name: bucket.name,
            characters: bucket.characters,
            occurrences: bucket.occurrences,
            known_numeric: bucket.known_numeric,
            unknown_numeric: bucket.unknown_numeric,
            known_sum: if bucket.known_numeric == 0 {
                String::new()
            } else {
                bucket.known_sum.to_string()
            },
            known_max: bucket.known_max.map(|value| value.to_string()),
            members: bucket.members,
        })
        .collect();

    Ok(Report {
        characters: characters.len(),
        buckets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn bucket<'a>(report: &'a Report, list: &str, name: &str) -> &'a Bucket {
        report
            .buckets
            .iter()
            .find(|bucket| bucket.list == list && bucket.name == name)
            .unwrap()
    }

    #[test]
    fn groups_case_insensitive_trimmed_unicode_names_deterministically() {
        let characters = [
            character("Mira", "Mage", &[("InfluenceList", &[("  ÉCLAIR ", "2")])]),
            character("Sera", "Mage", &[("InfluenceList", &[("éclair", "3")])]),
        ];

        let report = build(&characters, Kind::Influence).unwrap();
        assert_eq!(report.buckets.len(), 1);
        let group = bucket(&report, "InfluenceList", "ÉCLAIR");
        assert_eq!(group.characters, 2);
        assert_eq!(group.occurrences, 2);
        assert_eq!(group.known_sum, "5");
    }

    #[test]
    fn duplicate_entries_count_as_occurrences_but_one_character() {
        let characters = [
            character(
                "Same Name",
                "Mage",
                &[("MeritList", &[("Patient", "2"), ("patient", "3")])],
            ),
            character(
                "Same Name",
                "Vampire",
                &[("MeritList", &[("PATIENT", "4")])],
            ),
        ];

        let report = build(&characters, Kind::MeritsFlaws).unwrap();
        let group = bucket(&report, "MeritList", "PATIENT");
        assert_eq!(report.characters, 2);
        assert_eq!(group.characters, 2);
        assert_eq!(group.occurrences, 3);
        assert_eq!(group.members.len(), 2);
        assert_eq!(group.members[0].name, "Same Name");
        assert_eq!(group.members[0].values, ["2", "3"]);
        assert_eq!(group.members[1].name, "Same Name");
        assert_eq!(group.members[1].family, "Vampire");
        assert_eq!(group.members[1].values, ["4"]);
    }

    #[test]
    fn numeric_totals_and_maxima_use_known_unsigned_values_only() {
        let characters = [character(
            "Mira",
            "Mage",
            &[(
                "InfluenceList",
                &[
                    ("Resources", "2"),
                    ("Resources", "cleared"),
                    ("Resources", "-1"),
                    ("Resources", "1.5"),
                    ("Resources", ""),
                    ("Resources", "0"),
                ],
            )],
        )];

        let report = build(&characters, Kind::Influence).unwrap();
        let group = bucket(&report, "InfluenceList", "Resources");
        assert_eq!(group.occurrences, 6);
        assert_eq!(group.known_numeric, 2);
        assert_eq!(group.unknown_numeric, 4);
        assert_eq!(group.known_sum, "2");
        assert_eq!(group.known_max.as_deref(), Some("2"));
        assert_eq!(
            group.members[0].values,
            ["2", "cleared", "-1", "1.5", "", "0"]
        );
    }

    #[test]
    fn zero_and_large_unsigned_values_are_exact() {
        let largest = u128::MAX.to_string();
        let characters = [character(
            "Mira",
            "Mage",
            &[(
                "InfluenceList",
                &[("Resources", "0"), ("Resources", &largest)],
            )],
        )];

        let report = build(&characters, Kind::Influence).unwrap();
        let group = bucket(&report, "InfluenceList", "Resources");
        assert_eq!(group.known_numeric, 2);
        assert_eq!(group.known_sum, largest);
        assert_eq!(group.known_max.as_deref(), Some(largest.as_str()));
    }

    #[test]
    fn known_sum_overflow_fails_without_wrapping() {
        let largest = u128::MAX.to_string();
        let characters = [character(
            "Mira",
            "Mage",
            &[(
                "InfluenceList",
                &[("Resources", &largest), ("Resources", "1")],
            )],
        )];

        let failure = build(&characters, Kind::Influence).unwrap_err();
        assert_eq!(failure.code, "TRAIT_STATISTICS_OVERFLOW");
    }

    #[test]
    fn only_requested_lists_are_aggregated_and_missing_lists_are_valid() {
        let characters = [character(
            "Mira",
            "Mage",
            &[
                ("InfluenceList", &[("Resources", "3")]),
                ("MeritList", &[("Patient", "2")]),
                ("FlawList", &[("Short Fuse", "descriptive")]),
                ("DisciplineList", &[("Auspex", "4")]),
            ],
        )];

        let influence = build(&characters, Kind::Influence).unwrap();
        assert_eq!(influence.buckets.len(), 1);
        assert_eq!(influence.buckets[0].list, "InfluenceList");

        let merits_flaws = build(&characters, Kind::MeritsFlaws).unwrap();
        assert_eq!(merits_flaws.buckets.len(), 2);
        assert!(
            merits_flaws
                .buckets
                .iter()
                .all(|bucket| bucket.list == "MeritList" || bucket.list == "FlawList")
        );

        let empty_lists = [character("Sera", "Changeling", &[])];
        let empty_report = build(&empty_lists, Kind::Influence).unwrap();
        assert_eq!(empty_report.characters, 1);
        assert!(empty_report.buckets.is_empty());
    }

    #[test]
    fn missing_names_and_out_of_bound_inputs_are_errors() {
        assert!(build(&[character("  ", "Mage", &[])], Kind::Influence).is_err());
        assert!(
            build(
                &[character(
                    "Mira",
                    "Mage",
                    &[("InfluenceList", &[("  ", "1")])]
                )],
                Kind::Influence
            )
            .is_err()
        );
        assert!(
            build(
                &[character(
                    "Mira",
                    "Mage",
                    &[("InfluenceList", &[("Resources", "bad\0value")])]
                )],
                Kind::Influence
            )
            .is_err()
        );

        let too_long = "x".repeat(MAX_TEXT_BYTES + 1);
        assert!(
            build(
                &[character(
                    "Mira",
                    "Mage",
                    &[("InfluenceList", &[(&too_long, "1")])]
                )],
                Kind::Influence
            )
            .is_err()
        );

        let too_many_entries = (0..=MAX_ENTRIES_PER_CHARACTER)
            .map(|index| Entry {
                name: format!("Trait {index}"),
                value: "1".into(),
            })
            .collect();
        let too_many_entries = Character {
            name: "Mira".into(),
            family: "Mage".into(),
            lists: BTreeMap::from([("InfluenceList".into(), too_many_entries)]),
        };
        assert!(build(&[too_many_entries], Kind::Influence).is_err());

        let too_many_characters = vec![character("Mira", "Mage", &[]); MAX_CHARACTERS + 1];
        assert!(build(&too_many_characters, Kind::Influence).is_err());
    }

    #[test]
    fn report_occurrence_bound_is_enforced() {
        let entries = vec![
            Entry {
                name: "Resources".into(),
                value: "1".into(),
            };
            MAX_ENTRIES_PER_CHARACTER
        ];
        let character = Character {
            name: "Mira".into(),
            family: "Mage".into(),
            lists: BTreeMap::from([("InfluenceList".into(), entries)]),
        };
        let characters = vec![character; MAX_REPORT_OCCURRENCES / MAX_ENTRIES_PER_CHARACTER + 1];
        assert!(build(&characters, Kind::Influence).is_err());
    }
}
