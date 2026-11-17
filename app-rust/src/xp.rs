//! Exact local XP bookkeeping and evidence-limited audits; not organizational approval.
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCALE: i64 = 1_000_000;
pub const LIMIT: i64 = 9_000_000_000_000_000;
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum XpKind {
    Award,
    Spend,
    Adjustment,
    Reversal,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct Baseline {
    pub day: String,
    pub earned: String,
    pub unspent: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct CostEvidence {
    #[serde(default)]
    pub affiliation_name: String,
    #[serde(default)]
    pub power_family: String,
    #[serde(default)]
    pub approval_reference: String,
    pub base: String,
    pub affiliation_adjustment: String,
    pub affiliation: String,
    pub source: String,
    pub edition: String,
    pub page: String,
    pub policy_version: String,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct XpCommand {
    pub id: String,
    pub kind: XpKind,
    pub amount: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
    pub reversal_of: Option<String>,
    pub cost: Option<CostEvidence>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Entry {
    pub command: XpCommand,
    pub recorded_unix_seconds: u64,
    pub origin_library_id: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Ledger {
    pub baseline: Baseline,
    pub entries: Vec<Entry>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Finding {
    pub code: String,
    pub severity: String,
    pub entry_id: Option<String>,
    pub message: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LegacyEvent {
    pub id: String,
    pub date: String,
    pub kind: String,
    pub amount: String,
    pub earned: String,
    pub unspent: String,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct XpView {
    pub record_id: String,
    pub record_name: String,
    pub is_build: bool,
    pub ledger: Option<Ledger>,
    pub earned: Option<String>,
    pub unspent: Option<String>,
    pub legacy_history_count: usize,
    pub legacy_history: Vec<LegacyEvent>,
    pub legacy_earned: Option<String>,
    pub legacy_unspent: Option<String>,
    pub findings: Vec<Finding>,
    pub coverage: String,
}

pub fn amount(text: &str) -> Result<i64> {
    let text = text.trim();
    let (negative, body) = if let Some(s) = text.strip_prefix('-') {
        (true, s)
    } else {
        (false, text)
    };
    let mut parts = body.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.is_empty()
        || whole.len() > 10
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|c| c.is_ascii_digit())
        || body.ends_with('.')
    {
        return Err(error(
            "INVALID_XP",
            "Enter a decimal XP amount with at most six fractional digits; no rounding is performed.",
        ));
    }
    let whole = whole
        .parse::<i64>()
        .map_err(|_| error("INVALID_XP", "XP amount is too large."))?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i64>().unwrap() * 10_i64.pow(6 - fraction.len() as u32)
    };
    let units = whole
        .checked_mul(SCALE)
        .and_then(|n| n.checked_add(fraction))
        .ok_or_else(|| error("XP_LIMIT", "XP amount exceeds storage limits."))?;
    if units > LIMIT {
        return Err(error("XP_LIMIT", "XP amount exceeds storage limits."));
    }
    Ok(if negative { -units } else { units })
}
pub fn decimal(units: i64) -> String {
    let magnitude = i128::from(units).abs();
    let whole = magnitude / i128::from(SCALE);
    let frac = magnitude % i128::from(SCALE);
    let sign = if units < 0 { "-" } else { "" };
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{:06}", frac)
            .trim_end_matches('0')
            .to_string()
    }
}
pub fn day_valid(day: &str) -> bool {
    let b = day.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return false;
    }
    let y = day[..4].parse::<u32>().unwrap();
    let m = day[5..7].parse::<u32>().unwrap();
    let d = day[8..].parse::<u32>().unwrap();
    let max = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 400 == 0 || (y % 4 == 0 && y % 100 != 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    y >= 1 && d >= 1 && d <= max
}
fn required(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 16000
}
pub fn validate_baseline(b: &Baseline) -> Result<()> {
    let earned = amount(&b.earned)?;
    let unspent = amount(&b.unspent)?;
    if !day_valid(&b.day) || !required(&b.actor) || !required(&b.reason) || !required(&b.evidence) {
        return Err(error(
            "XP_EVIDENCE_REQUIRED",
            "Opening balance needs a valid YYYY-MM-DD date, recorder, reason and evidence reference.",
        ));
    }
    if unspent < 0 || earned < 0 || unspent > earned {
        return Err(error(
            "INVALID_XP_BASELINE",
            "Opening earned/unspent must be nonnegative, with unspent no greater than earned.",
        ));
    }
    Ok(())
}
pub fn canonical_command(ledger: &Ledger, input: &XpCommand) -> Result<XpCommand> {
    if input.id.trim().is_empty()
        || input.id.len() > 128
        || !day_valid(&input.day)
        || input.day < ledger.baseline.day
        || !required(&input.actor)
        || !required(&input.reason)
        || !required(&input.evidence)
    {
        return Err(error(
            "XP_EVIDENCE_REQUIRED",
            "Entry needs an ID, valid date on/after the opening date, recorder, reason and evidence reference.",
        ));
    }
    let mut command = input.clone();
    if command.kind == XpKind::Reversal {
        let target = ledger
            .entries
            .iter()
            .find(|e| Some(&e.command.id) == input.reversal_of.as_ref())
            .ok_or_else(|| error("XP_REVERSAL_TARGET", "Select an existing entry to reverse."))?;
        if target.command.kind == XpKind::Reversal || command.day < target.command.day {
            return Err(error(
                "XP_REVERSAL_TARGET",
                "Reverse an original entry on or after its effective date.",
            ));
        }
        command.amount = target.command.amount.clone();
        command.cost = None;
    } else {
        if command.reversal_of.is_some() {
            return Err(error(
                "XP_REVERSAL_TARGET",
                "Only reversals may reference another entry.",
            ));
        }
        let units = amount(&command.amount)?;
        if units == 0 || (command.kind != XpKind::Adjustment && units < 0) {
            return Err(error(
                "INVALID_XP",
                "Awards/spending must be positive; adjustments may be signed and nonzero.",
            ));
        }
        command.amount = decimal(units);
        if command.cost.is_some() && command.kind != XpKind::Spend {
            return Err(error(
                "INVALID_XP_COST",
                "Cost components apply only to spending.",
            ));
        }
        if let Some(cost) = &mut command.cost {
            if amount(&cost.base)? < 0
                || amount(&cost.affiliation_adjustment)? < 0
                || [
                    &cost.affiliation,
                    &cost.affiliation_name,
                    &cost.power_family,
                    &cost.approval_reference,
                    &cost.source,
                    &cost.edition,
                    &cost.page,
                    &cost.policy_version,
                ]
                .iter()
                .any(|s| s.len() > 16000)
            {
                return Err(error(
                    "INVALID_XP_COST",
                    "Cost components must be nonnegative and references within text limits.",
                ));
            }
            cost.base = decimal(amount(&cost.base)?);
            cost.affiliation_adjustment = decimal(amount(&cost.affiliation_adjustment)?);
        }
    }
    Ok(command)
}
pub(crate) fn ordinary_delta(c: &XpCommand) -> Result<(i64, i64)> {
    let n = amount(&c.amount)?;
    Ok(match c.kind {
        XpKind::Award | XpKind::Adjustment => (n, n),
        XpKind::Spend => (0, -n),
        XpKind::Reversal => return Err(error("XP_CORRUPT_LEDGER", "Reversal target is invalid.")),
    })
}
/// Replays effective dates, ties by append order. Every intermediate balance is checked.
pub fn replay(ledger: &Ledger) -> Result<(i64, i64)> {
    validate_baseline(&ledger.baseline)?;
    if ledger.entries.len() > 100000 {
        return Err(error("XP_LIMIT", "Ledger has too many entries."));
    }
    let mut seen = BTreeMap::<String, &XpCommand>::new();
    let mut reversed = std::collections::BTreeSet::new();
    let mut deltas = vec![];
    for (seq, entry) in ledger.entries.iter().enumerate() {
        let c = canonical_command(ledger, &entry.command)?;
        if c != entry.command || seen.contains_key(&c.id) || !required(&entry.origin_library_id) {
            return Err(error(
                "XP_CORRUPT_LEDGER",
                "Ledger has an invalid or duplicate entry; preserve it for review.",
            ));
        }
        let delta = if c.kind == XpKind::Reversal {
            let target_id = c.reversal_of.as_ref().unwrap();
            let target = seen.get(target_id).ok_or_else(|| {
                error(
                    "XP_CORRUPT_LEDGER",
                    "Reversal precedes its recorded target.",
                )
            })?;
            if !reversed.insert(target_id.clone()) {
                return Err(error(
                    "XP_ALREADY_REVERSED",
                    "An entry can be reversed only once.",
                ));
            }
            let (e, u) = ordinary_delta(target)?;
            (-e, -u)
        } else {
            ordinary_delta(&c)?
        };
        seen.insert(c.id.clone(), &entry.command);
        deltas.push((c.day, seq, delta));
    }
    deltas.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let (mut earned, mut unspent) = (
        i128::from(amount(&ledger.baseline.earned)?),
        i128::from(amount(&ledger.baseline.unspent)?),
    );
    for (_, _, (e, u)) in deltas {
        earned += i128::from(e);
        unspent += i128::from(u);
        if earned < 0 || unspent < 0 || unspent > earned {
            return Err(error(
                "XP_NEGATIVE_HISTORY",
                "This entry would create a negative or inconsistent running XP balance.",
            ));
        }
        if earned > i128::from(LIMIT) || unspent > i128::from(LIMIT) {
            return Err(error(
                "XP_LIMIT",
                "Running XP balance exceeds storage limits.",
            ));
        }
    }
    Ok((earned as i64, unspent as i64))
}
fn add(findings: &mut Vec<Finding>, code: &str, severity: &str, id: Option<String>, message: &str) {
    findings.push(Finding {
        code: code.into(),
        severity: severity.into(),
        entry_id: id,
        message: message.into(),
    });
}
pub fn view(record: &Record) -> XpView {
    let legacy = record
        .private_fields
        .get("Experience")
        .or_else(|| record.public_fields.get("Experience"));
    let history = legacy
        .and_then(|x| x.get("History"))
        .and_then(serde_json::Value::as_array);
    let mut findings = vec![];
    add(
        &mut findings,
        "AUTHORIZATION_UNVERIFIED",
        "unverified",
        None,
        "Local declarations and imported history do not prove external award or purchase authorization.",
    );
    let mut earned = None;
    let mut unspent = None;
    if let Some(ledger) = &record.xp_ledger {
        match replay(ledger) {
            Ok((e, u)) => {
                earned = Some(decimal(e));
                unspent = Some(decimal(u));
            }
            Err(e) => add(&mut findings, e.code, "discrepancy", None, &e.message),
        }
        add(
            &mut findings,
            "DECLARED_BASELINE",
            "unverified",
            None,
            "Opening balance is a local declaration; pre-opening coverage and external evidence need review.",
        );
        let mut awards = BTreeMap::<(String, String, String), usize>::new();
        for entry in &ledger.entries {
            let c = &entry.command;
            if c.kind == XpKind::Adjustment {
                add(
                    &mut findings,
                    "MANUAL_ADJUSTMENT",
                    "review",
                    Some(c.id.clone()),
                    "Manual signed adjustment requires review of its reason and evidence.",
                );
            }
            if c.kind == XpKind::Award {
                let key = (c.day.clone(), c.amount.clone(), c.evidence.clone());
                let count = awards.entry(key).or_default();
                *count += 1;
                if *count > 1 {
                    add(
                        &mut findings,
                        "POSSIBLE_DUPLICATE_AWARD",
                        "review",
                        Some(c.id.clone()),
                        "Another award has the same effective date, amount and evidence reference; check whether both were intended.",
                    );
                }
            }
            if c.kind == XpKind::Spend {
                match &c.cost {
                    None => add(
                        &mut findings,
                        "COST_UNVERIFIED",
                        "unverified",
                        Some(c.id.clone()),
                        "Recorded spending lacks structured base/affiliation/source evidence; automatic cost validation is unavailable.",
                    ),
                    Some(cost) => {
                        let total = amount(&cost.base).ok().and_then(|b| {
                            amount(&cost.affiliation_adjustment)
                                .ok()
                                .and_then(|a| b.checked_add(a))
                        });
                        if total != amount(&c.amount).ok() {
                            add(
                                &mut findings,
                                "COST_MISMATCH",
                                "discrepancy",
                                Some(c.id.clone()),
                                "Recorded spending differs from its declared base plus affiliation adjustment.",
                            );
                        }
                        if !["in", "out", "InClan", "OutOfClan"]
                            .contains(&cost.affiliation.as_str())
                            || [
                                &cost.affiliation_name,
                                &cost.power_family,
                                &cost.source,
                                &cost.edition,
                                &cost.page,
                                &cost.policy_version,
                            ]
                            .iter()
                            .any(|s| !required(s))
                        {
                            add(
                                &mut findings,
                                "AFFILIATION_COST_UNVERIFIED",
                                "unverified",
                                Some(c.id.clone()),
                                "Affiliation or source/edition/page/policy evidence is incomplete; a base rate must not be assumed correct.",
                            );
                        }
                        add(
                            &mut findings,
                            "DECLARED_COST_POLICY",
                            "unverified",
                            Some(c.id.clone()),
                            "Declared cost components are not independently verified against a rules pack or organizational approval.",
                        );
                    }
                }
            }
        }
    } else {
        add(
            &mut findings,
            "OPENING_REQUIRED",
            "unverified",
            None,
            "No exact local ledger is open; inspect imported history before declaring a supported opening balance.",
        );
    }
    legacy_findings(legacy, &mut findings);
    let legacy_text = |key: &str| legacy.and_then(|x| x.get(key)).map(ToString::to_string);
    let legacy_history = history
        .map(|events| {
            events
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let text = |key: &str| {
                        e.get(key)
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "Unknown".into())
                    };
                    LegacyEvent {
                        id: format!("legacy:{i}"),
                        date: e
                            .pointer("/When/ole_automation_days")
                            .and_then(serde_json::Value::as_f64)
                            .map(legacy_date)
                            .unwrap_or_else(|| "Unknown date".into()),
                        kind: match e.get("ChangeType").and_then(serde_json::Value::as_i64) {
                            Some(0) => "Earned",
                            Some(1) => "Deducted",
                            Some(2) => "Set earned",
                            Some(3) => "Spent",
                            Some(4) => "Refund/unspent",
                            Some(5) => "Set unspent",
                            Some(6) => "Comment",
                            _ => "Unknown event",
                        }
                        .into(),
                        amount: text("Change"),
                        earned: text("Earned"),
                        unspent: text("Unspent"),
                        reason: e
                            .get("Reason")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .into(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    XpView {
        record_id: record.id.clone(), record_name: record.name.clone(), is_build: record.build.is_some(),
        ledger: record.xp_ledger.clone(), earned, unspent,
        legacy_history_count: history.map_or(0, Vec::len), legacy_history,
        legacy_earned: legacy_text("Earned"), legacy_unspent: legacy_text("Unspent"), findings,
        coverage: "Local arithmetic with separate linked-receipt findings where available. External authorization and pre-opening completeness remain unverified. Manual XP entries do not grant traits.".into(),
    }
}

fn legacy_findings(legacy: Option<&serde_json::Value>, findings: &mut Vec<Finding>) {
    let Some(legacy) = legacy else {
        return;
    };
    if ["Earned", "Unspent"].iter().any(|key| {
        legacy
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .is_some_and(|n| n < 0.0)
    }) {
        add(
            findings,
            "LEGACY_NEGATIVE_TOTALS",
            "discrepancy",
            None,
            "Imported character totals include negative earned or unspent XP.",
        );
    }
    let Some(history) = legacy.get("History").and_then(serde_json::Value::as_array) else {
        add(
            findings,
            "LEGACY_HISTORY_UNAVAILABLE",
            "unverified",
            None,
            "Imported XP totals have no recognized history structure.",
        );
        return;
    };
    if !history.is_empty() {
        add(
            findings,
            "LEGACY_HISTORY_UNVERIFIED",
            "unverified",
            None,
            "Legacy events have no recorder/transaction/approval IDs; replay checks arithmetic only, using stored order and VB Single rounding.",
        );
    }
    let (mut earned, mut unspent) = (0_f32, 0_f32);
    let mut replay_known = true;
    let mut previous_day = None;
    let mut spend_count = 0;
    let mut awards = BTreeMap::new();
    let number = |e: &serde_json::Value, key: &str| {
        e.get(key)
            .and_then(serde_json::Value::as_f64)
            .filter(|n| n.is_finite())
            .map(|n| n as f32)
    };
    for (index, entry) in history.iter().enumerate() {
        let id = Some(format!("legacy:{index}"));
        if let Some(day) = entry
            .pointer("/When/ole_automation_days")
            .and_then(serde_json::Value::as_f64)
        {
            if previous_day.is_some_and(|previous| previous > day) {
                add(
                    findings,
                    "LEGACY_DATE_ORDER",
                    "review",
                    id.clone(),
                    "Imported effective dates are out of order; inspect the retained source sequence.",
                );
            }
            previous_day = Some(day);
        }
        let actual_e = number(entry, "Earned");
        let actual_u = number(entry, "Unspent");
        if actual_e.is_some_and(|n| n < 0.0) || actual_u.is_some_and(|n| n < 0.0) {
            add(
                findings,
                "LEGACY_NEGATIVE_SNAPSHOT",
                "discrepancy",
                id.clone(),
                "Imported history records a negative earned or unspent snapshot.",
            );
        }
        if actual_e.zip(actual_u).is_some_and(|(e, u)| u > e) {
            add(
                findings,
                "LEGACY_TOTALS_INCONSISTENT",
                "review",
                id.clone(),
                "Imported unspent snapshot exceeds its recorded earned total.",
            );
        }
        let kind = entry.get("ChangeType").and_then(serde_json::Value::as_i64);
        let Some(change) = number(entry, "Change") else {
            add(
                findings,
                "LEGACY_AMOUNT_UNAVAILABLE",
                "unverified",
                id,
                "Legacy amount is missing or not a finite number.",
            );
            replay_known = false;
            continue;
        };
        if kind == Some(3) {
            spend_count += 1;
        }
        if kind == Some(0) && change > 0.0 {
            let day = entry
                .pointer("/When/ole_automation_days")
                .and_then(serde_json::Value::as_f64)
                .map(f64::to_bits);
            let key = (
                day,
                change.to_bits(),
                entry
                    .get("Reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            );
            if awards.insert(key, index).is_some() {
                add(
                    findings,
                    "LEGACY_POSSIBLE_DUPLICATE_AWARD",
                    "review",
                    id.clone(),
                    "Another legacy award has the same date,amount and reason; verify whether both were intended.",
                );
            }
        }
        if change < 0.0 {
            add(
                findings,
                "LEGACY_SIGNED_CHANGE",
                "review",
                id.clone(),
                "Legacy change has a negative amount; its meaning follows its recorded event type and requires review.",
            );
        }
        if matches!(kind, Some(2 | 5)) {
            add(
                findings,
                "LEGACY_SET_TOTAL",
                "review",
                id.clone(),
                "Legacy event directly sets a balance; arithmetic does not establish that this override was authorized.",
            );
        }
        if replay_known {
            match kind {
                Some(0) => {
                    earned += change;
                    unspent += change;
                }
                Some(1) => {
                    earned -= change;
                    unspent -= change;
                }
                Some(2) => earned = change,
                Some(3) => unspent -= change,
                Some(4) => unspent += change,
                Some(5) => unspent = change,
                Some(6) => {}
                _ => {
                    add(
                        findings,
                        "LEGACY_UNKNOWN_TYPE",
                        "unverified",
                        id.clone(),
                        "Unknown legacy XP event type stops dependent arithmetic comparisons; original evidence is retained.",
                    );
                    replay_known = false;
                }
            }
            if replay_known {
                if index == 0
                    && actual_e
                        .zip(actual_u)
                        .is_some_and(|(e, u)| e != earned || u != unspent)
                {
                    if (kind == Some(2) && actual_e != Some(change))
                        || (kind == Some(5) && actual_u != Some(change))
                    {
                        add(
                            findings,
                            "LEGACY_SNAPSHOT_MISMATCH",
                            "discrepancy",
                            id.clone(),
                            "The first set-total event does not match the value it explicitly sets.",
                        );
                    }
                    add(
                        findings,
                        "LEGACY_OPENING_UNKNOWN",
                        "unverified",
                        id.clone(),
                        "The first retained snapshot implies an unknown opening history. Later replay is anchored to this snapshot; it does not verify its opening authorization.",
                    );
                    earned = actual_e.unwrap();
                    unspent = actual_u.unwrap();
                }
                if earned < 0.0 || unspent < 0.0 {
                    add(
                        findings,
                        "LEGACY_NEGATIVE_REPLAY",
                        "discrepancy",
                        id.clone(),
                        "Replaying imported events produces a negative running XP balance.",
                    );
                }
                if actual_e.is_none() || actual_u.is_none() {
                    add(
                        findings,
                        "LEGACY_SNAPSHOT_UNAVAILABLE",
                        "unverified",
                        id.clone(),
                        "Stored post-event totals are missing or not finite.",
                    );
                } else if actual_e != Some(earned) || actual_u != Some(unspent) {
                    add(
                        findings,
                        "LEGACY_SNAPSHOT_MISMATCH",
                        "discrepancy",
                        id.clone(),
                        &format!(
                            "Computed post-event earned/unspent ({earned}/{unspent}) differ from the retained snapshot."
                        ),
                    );
                }
            }
        }
    }
    if replay_known
        && !history.is_empty()
        && (number(legacy, "Earned") != Some(earned) || number(legacy, "Unspent") != Some(unspent))
    {
        add(
            findings,
            "LEGACY_CONTAINER_MISMATCH",
            "discrepancy",
            None,
            "Final replayed XP totals differ from the imported character container totals.",
        );
    }
    if spend_count > 0 {
        add(
            findings,
            "LEGACY_COST_UNVERIFIED",
            "unverified",
            None,
            &format!(
                "{spend_count} legacy spending entries lack structured affiliation,source and approval evidence; base-only pricing cannot be assumed correct."
            ),
        );
    }
    if history.is_empty()
        && (number(legacy, "Earned").unwrap_or(0.0) != 0.0
            || number(legacy, "Unspent").unwrap_or(0.0) != 0.0)
    {
        add(
            findings,
            "LEGACY_OPENING_UNKNOWN",
            "unverified",
            None,
            "Nonzero imported XP totals have no replayable opening history.",
        );
    }
}

pub fn legacy_date(ole: f64) -> String {
    if !ole.is_finite() || !(0.0..2_958_466.0).contains(&ole) {
        return format!("Retained legacy date {ole}");
    }
    let seconds = (ole.fract() * 86400.0).floor() as i64;
    let day = ole_day(ole.floor() as i64).expect("validated positive OLE day");
    format!(
        "{day} {:02}:{:02}:{:02}",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

pub(crate) fn ole_day(serial: i64) -> Option<String> {
    if !(-657_434..2_958_466).contains(&serial) {
        return None;
    }
    let z = serial - 25569 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}
