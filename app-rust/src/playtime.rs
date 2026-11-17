//! A bounded comparison of XP awards with the declared OWbN calendar-month cap.
//! This report does not authorize XP or determine whether an award was valid.

use crate::{
    model::{Record, Result, error},
    xp::{self, Ledger, XpKind},
};
use serde::{Deserialize, Serialize};

pub const SOURCE_URL: &str = "https://council.owbn.net/en/charter-bylaws/character-bylaws/";
pub const POLICY_VERSION: &str = "owbn-character-2026-11-07";
const MAX_MONTHS: usize = 1200;
const MAX_MONTH_ENTRY_WORK: usize = 2_000_000;

const STATUS_WITHIN: &str = "WithinDeclaredCeiling";
const STATUS_EXCEEDS: &str = "ExceedsDeclaredCeiling";
const STATUS_INSUFFICIENT: &str = "InsufficientEvidence";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapClass {
    #[default]
    Unknown,
    Standard,
    Wraith,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Request {
    pub start_month: String,
    pub end_month: String,
    pub class: CapClass,
    pub policy_scope_confirmed: bool,
    pub complete_regular_history_confirmed: bool,
    pub full_month_eligible_confirmed: bool,
    pub home_chronicle: String,
    pub review_evidence: String,
    pub award_reviews: Vec<AwardReview>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AwardClass {
    Regular,
    SeparateReview,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwardReview {
    pub entry_id: String,
    pub entry_hash: String,
    pub class: AwardClass,
    pub reason: String,
    pub reference: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwardSnapshot {
    pub entry_id: String,
    pub entry_hash: String,
    pub day: String,
    pub amount: String,
    pub reason: String,
    pub evidence: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthReport {
    pub month: String,
    pub opening_earned: Option<String>,
    pub regular_awarded: String,
    pub separate_review_awarded: String,
    pub unclassified_awarded: String,
    pub source_awarded: String,
    pub ceiling: Option<String>,
    pub status: String,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub source_url: String,
    pub policy_version: String,
    pub start_month: String,
    pub end_month: String,
    pub status: String,
    pub months: Vec<MonthReport>,
    pub limitations: Vec<String>,
    pub award_classification_digest: String,
}

fn parse_month(value: &str) -> Option<u32> {
    let bytes = value.as_bytes();
    if bytes.len() != 7
        || bytes[4] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && !byte.is_ascii_digit())
    {
        return None;
    }
    let year = value[..4].parse::<u32>().ok()?;
    let month = value[5..].parse::<u32>().ok()?;
    if year == 0 || month == 0 || month > 12 {
        return None;
    }
    Some(year * 12 + month - 1)
}

fn month_string(index: u32) -> String {
    let year = index / 12;
    let month = index % 12 + 1;
    format!("{year:04}-{month:02}")
}

fn month_range(start: &str, end: &str) -> Result<Vec<String>> {
    let start_index = parse_month(start).ok_or_else(|| {
        error(
            "PLAYTIME_RANGE",
            "Enter a valid start month in YYYY-MM format.",
        )
    })?;
    let end_index = parse_month(end).ok_or_else(|| {
        error(
            "PLAYTIME_RANGE",
            "Enter a valid end month in YYYY-MM format.",
        )
    })?;
    if end_index < start_index {
        return Err(error(
            "PLAYTIME_RANGE",
            "The end month must not precede the start month.",
        ));
    }
    let count = (end_index - start_index + 1) as usize;
    if count > MAX_MONTHS {
        return Err(error(
            "PLAYTIME_RANGE_LIMIT",
            "A playtime report may cover at most 1,200 calendar months.",
        ));
    }
    Ok((start_index..=end_index).map(month_string).collect())
}

pub(crate) fn validate_month_range(start: &str, end: &str) -> Result<()> {
    month_range(start, end).map(|_| ())
}

fn valid_character(record: &Record) -> bool {
    matches!(
        record.kind.as_str(),
        "Vampire"
            | "Werewolf"
            | "Mortal"
            | "Changeling"
            | "Wraith"
            | "Mage"
            | "Fera"
            | "Various"
            | "Mummy"
            | "KueiJin"
            | "Hunter"
            | "Demon"
            | "Risen"
    )
}

fn evidence_issues(input: &Request) -> Vec<String> {
    let mut issues = Vec::new();
    if input.class == CapClass::Unknown {
        issues.push("Choose the character class before applying a monthly ceiling.".into());
    }
    if !input.policy_scope_confirmed {
        issues.push("Confirm that this policy applies to the requested historical range.".into());
    }
    if !input.complete_regular_history_confirmed {
        issues.push("Confirm that regular XP history is complete for the requested range.".into());
    }
    if !input.full_month_eligible_confirmed {
        issues
            .push("Confirm full-month eligibility for every month in the requested range.".into());
    }
    if !required(&input.home_chronicle) {
        issues.push("Record the home chronicle evidence.".into());
    }
    if !required(&input.review_evidence) {
        issues.push("Record the evidence reference for this review.".into());
    }
    issues
}

fn required(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 16000
}

fn opening_earned(ledger: &Ledger, first_day: &str) -> Option<String> {
    if ledger.baseline.day.as_str() > first_day {
        return None;
    }
    let mut preceding = ledger.clone();
    preceding
        .entries
        .retain(|entry| entry.command.day.as_str() < first_day);
    let (earned, _) = xp::replay(&preceding).ok()?;
    Some(xp::decimal(earned))
}

pub fn awards(record: &Record, start: &str, end: &str) -> Result<Vec<AwardSnapshot>> {
    if !valid_character(record) || record.build.is_some() {
        return Err(error(
            "PLAYTIME_SCOPE",
            "Review awards only on an original character.",
        ));
    }
    month_range(start, end)?;
    let ledger = record.xp_ledger.as_ref().ok_or_else(|| {
        error(
            "PLAYTIME_AWARDS_UNAVAILABLE",
            "A supported XP opening and ledger are required to classify awards.",
        )
    })?;
    let reversals = ledger
        .entries
        .iter()
        .filter(|entry| entry.command.kind == XpKind::Reversal)
        .count();
    if ledger
        .entries
        .len()
        .checked_mul(reversals + 1)
        .is_none_or(|work| work > MAX_MONTH_ENTRY_WORK)
    {
        return Err(error(
            "PLAYTIME_WORK_LIMIT",
            "This ledger requires too much replay work for a synchronous award review.",
        ));
    }
    xp::replay(ledger).map_err(|_| {
        error(
            "PLAYTIME_AWARDS_UNAVAILABLE",
            "The XP ledger does not replay cleanly; award classification is unavailable.",
        )
    })?;
    let mut rows = vec![];
    for entry in &ledger.entries {
        if entry.command.kind != XpKind::Award
            || !entry
                .command
                .day
                .get(..7)
                .is_some_and(|m| m >= start && m <= end)
        {
            continue;
        }
        if rows.len() >= 10000 {
            return Err(error(
                "PLAYTIME_AWARD_LIMIT",
                "Choose a shorter range with at most10000 awards.",
            ));
        }
        rows.push(AwardSnapshot {
            entry_id: entry.command.id.clone(),
            entry_hash: crate::legacy_profile::hash(&(&record.id, &record.source_sha256, entry))?,
            day: entry.command.day.clone(),
            amount: entry.command.amount.clone(),
            reason: entry.command.reason.clone(),
            evidence: entry.command.evidence.clone(),
        });
    }
    Ok(rows)
}
fn reviewed_classes(
    record: &Record,
    input: &Request,
) -> Result<std::collections::HashMap<String, AwardClass>> {
    if input.award_reviews.len() > 10000
        || input
            .award_reviews
            .iter()
            .try_fold(0usize, |n, r| {
                n.checked_add(
                    r.entry_id.len() + r.entry_hash.len() + r.reason.len() + r.reference.len() + 64,
                )
            })
            .is_none_or(|n| n > 2 * 1024 * 1024)
    {
        return Err(error(
            "PLAYTIME_AWARD_LIMIT",
            "Award classifications exceed the bounded review limit.",
        ));
    }
    let mut map = std::collections::HashMap::new();
    if input.award_reviews.is_empty() {
        return Ok(map);
    }
    let current = awards(record, &input.start_month, &input.end_month)?
        .into_iter()
        .map(|r| (r.entry_id, r.entry_hash))
        .collect::<std::collections::HashMap<_, _>>();
    for review in &input.award_reviews {
        if !required(&review.reason) || !required(&review.reference) {
            return Err(error(
                "PLAYTIME_AWARD_EVIDENCE",
                "Record a reason and reference for every award classification.",
            ));
        }
        if current.get(&review.entry_id) != Some(&review.entry_hash)
            || map.insert(review.entry_id.clone(), review.class).is_some()
        {
            return Err(error(
                "PLAYTIME_AWARD_REVIEW_STALE",
                "An award classification is duplicated, outside this range, unknown or changed. Reload and review its exact entry.",
            ));
        }
    }
    Ok(map)
}
fn award_totals(
    ledger: &Ledger,
    month: &str,
    classes: &std::collections::HashMap<String, AwardClass>,
) -> Option<(String, String, String, String)> {
    let (mut regular, mut separate, mut unclassified, mut source) = (0i64, 0i64, 0i64, 0i64);
    for entry in &ledger.entries {
        if entry.command.kind != XpKind::Award || entry.command.day.get(..7) != Some(month) {
            continue;
        }
        let units = xp::amount(&entry.command.amount).ok()?;
        source = source.checked_add(units)?;
        let total = match classes.get(&entry.command.id) {
            Some(AwardClass::Regular) => &mut regular,
            Some(AwardClass::SeparateReview) => &mut separate,
            None => &mut unclassified,
        };
        *total = total.checked_add(units)?;
    }
    Some((
        xp::decimal(regular),
        xp::decimal(separate),
        xp::decimal(unclassified),
        xp::decimal(source),
    ))
}
fn adjustment_or_award_reversal(ledger: &Ledger, month: &str) -> bool {
    let targets: std::collections::HashMap<_, _> = ledger
        .entries
        .iter()
        .map(|entry| (entry.command.id.as_str(), entry.command.kind))
        .collect();
    ledger.entries.iter().any(|entry| {
        if entry.command.day.get(..7) != Some(month) {
            return false;
        }
        match entry.command.kind {
            XpKind::Adjustment => true,
            XpKind::Reversal => entry
                .command
                .reversal_of
                .as_deref()
                .and_then(|target| targets.get(target))
                .is_some_and(|kind| matches!(kind, XpKind::Award | XpKind::Adjustment)),
            XpKind::Award | XpKind::Spend => false,
        }
    })
}

fn standard_ceiling(earned: i64) -> Option<i64> {
    let twelve = 200 * xp::SCALE;
    let ten = 201 * xp::SCALE;
    let eight = 400 * xp::SCALE;
    let nine = 401 * xp::SCALE;
    if earned <= twelve {
        Some(12)
    } else if earned < ten {
        None
    } else if earned <= eight {
        Some(10)
    } else if earned < nine {
        None
    } else {
        Some(8)
    }
}

fn build_month(
    month: String,
    ledger: Option<&Ledger>,
    ledger_valid: bool,
    input: &Request,
    base_issues: &[String],
    classes: &std::collections::HashMap<String, AwardClass>,
) -> MonthReport {
    let first_day = format!("{month}-01");
    let mut issues = base_issues.to_vec();
    let mut opening = None;
    let mut awards = "unknown".to_string();
    let mut separate = "unknown".to_string();
    let mut unclassified = "unknown".to_string();
    let mut source = "unknown".to_string();

    match ledger {
        None => issues.push("No supported XP opening and ledger are available.".into()),
        Some(ledger) => {
            if !ledger_valid {
                issues.push(
                    "The XP ledger does not replay cleanly; no clean comparison is possible."
                        .into(),
                );
            } else if ledger.baseline.day > first_day {
                issues.push("The opening balance begins after this month's first day.".into());
            } else {
                opening = opening_earned(ledger, &first_day);
                if opening.is_none() {
                    issues
                        .push("Opening earned XP cannot be derived safely for this month.".into());
                }
            }
            match award_totals(ledger, &month, classes) {
                Some((regular, other, pending, total)) => {
                    awards = regular;
                    separate = other;
                    unclassified = pending;
                    source = total;
                    if separate != "0" {
                        issues.push("Separate-review awards remain unresolved; no cap exemption is inferred.".into());
                    }
                    if unclassified != "0" {
                        issues.push("Unclassified awards remain unresolved; they are not silently counted as regular.".into());
                    }
                }
                None => issues.push("Award amounts for this month cannot be summed safely.".into()),
            }
            if adjustment_or_award_reversal(ledger, &month) {
                issues.push(
                    "An adjustment or award/adjustment reversal occurs in this month; its cap effect needs separate review."
                        .into(),
                );
            }
        }
    }

    let ceiling = match input.class {
        CapClass::Unknown => None,
        CapClass::Wraith => Some(8),
        CapClass::Standard => match opening.as_deref().and_then(|value| xp::amount(value).ok()) {
            Some(earned) => match standard_ceiling(earned) {
                Some(limit) => Some(limit),
                None => {
                    issues.push(
                        "Opening earned XP falls in a fractional gap with no declared ceiling band."
                            .into(),
                    );
                    None
                }
            },
            None => None,
        },
    };
    let ceiling_text = ceiling.map(|value| value.to_string());
    let status = match (issues.is_empty(), ceiling, xp::amount(&awards).ok()) {
        (true, Some(limit), Some(units)) if units <= limit * xp::SCALE => STATUS_WITHIN,
        (true, Some(_), Some(_)) => STATUS_EXCEEDS,
        _ => STATUS_INSUFFICIENT,
    };

    MonthReport {
        month,
        opening_earned: opening,
        regular_awarded: awards,
        separate_review_awarded: separate,
        unclassified_awarded: unclassified,
        source_awarded: source,
        ceiling: ceiling_text,
        status: status.into(),
        issues,
    }
}

/// Compares declared regular award amounts with the monthly cap band.
///
/// This is a read-only report. It does not authorize awards, establish eligibility,
/// or determine whether any XP was earned under the governing rules.
pub fn assess(record: &Record, input: &Request) -> Result<Report> {
    if !valid_character(record) || record.build.is_some() {
        return Err(error(
            "PLAYTIME_SCOPE",
            "Assess playtime only on a live character record, not a build or non-character record.",
        ));
    }
    if (record.kind == "Wraith" && input.class == CapClass::Standard)
        || (!matches!(record.kind.as_str(), "Wraith" | "Risen" | "Various")
            && input.class == CapClass::Wraith)
    {
        return Err(error(
            "PLAYTIME_CLASS",
            "The selected cap class does not match the character type.",
        ));
    }

    let months = month_range(&input.start_month, &input.end_month)?;
    let ledger = record.xp_ledger.as_ref();
    if ledger.is_some_and(|ledger| {
        let reversals = ledger
            .entries
            .iter()
            .filter(|e| e.command.kind == XpKind::Reversal)
            .count();
        // Reversal canonicalization searches its target; include that lookup cost and
        // the initial full-ledger replay in the synchronous work budget.
        months
            .len()
            .checked_add(if input.award_reviews.is_empty() { 1 } else { 2 })
            .and_then(|n| n.checked_mul(ledger.entries.len()))
            .and_then(|n| n.checked_mul(reversals + 1))
            .is_none_or(|work| work > MAX_MONTH_ENTRY_WORK)
    }) {
        return Err(error(
            "PLAYTIME_WORK_LIMIT",
            "This report would require too much ledger replay work; choose a shorter range.",
        ));
    }
    let classes = reviewed_classes(record, input)?;
    let mut canonical_reviews = input.award_reviews.clone();
    canonical_reviews.sort_by(|a, b| a.entry_id.cmp(&b.entry_id));
    let award_classification_digest = crate::legacy_profile::hash(&canonical_reviews)?;
    let ledger_valid = ledger.is_some_and(|ledger| xp::replay(ledger).is_ok());
    let mut base_issues = evidence_issues(input);
    if matches!(record.kind.as_str(), "Risen" | "Various") {
        base_issues.push("Risen or mixed-template monthly-cap classification requires a separately reviewed rule; this report does not infer the Wraith exception.".into());
    }
    let month_reports: Vec<_> = months
        .into_iter()
        .map(|month| build_month(month, ledger, ledger_valid, input, &base_issues, &classes))
        .collect();
    let status = if month_reports
        .iter()
        .any(|month| month.status == STATUS_EXCEEDS)
    {
        STATUS_EXCEEDS
    } else if month_reports
        .iter()
        .any(|month| month.status == STATUS_INSUFFICIENT)
    {
        STATUS_INSUFFICIENT
    } else {
        STATUS_WITHIN
    };

    Ok(Report {
        source_url: SOURCE_URL.into(),
        policy_version: POLICY_VERSION.into(),
        start_month: input.start_month.clone(),
        end_month: input.end_month.clone(),
        status: status.into(),
        award_classification_digest,
        months: month_reports,
        limitations: vec![
            "The caller must establish that this policy applies to the requested historical range and that each month qualifies for the regular full-month cap.".into(),
            "Only explicitly classified awards are counted as regular for this local review. Separate-review or unclassified awards remain unresolved; no exception or exemption is inferred.".into(),
            "The report does not prove that XP was earned, that a limit was violated as misconduct, or that any transfer or approval is authorized.".into(),
        ],
    })
}
