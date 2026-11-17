//! Separate local Player points bookkeeping. Manual declarations do not prove OWbN approval.
use crate::{
    model::{Result, error},
    xp,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const TEXT_LIMIT: usize = 16_000;
const ENTRY_LIMIT: usize = 100_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    pub day: String,
    pub balance: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Award,
    Spend,
    Adjustment,
    Reversal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub kind: Kind,
    /// Reversal input is blank; its stored value is the signed balance delta that undoes its target.
    pub amount: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
    pub reversal_of: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    pub baseline: Baseline,
    pub entries: Vec<Command>,
}

fn required(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= TEXT_LIMIT
}

fn uuid_valid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}

fn baseline_units(baseline: &Baseline) -> Result<i64> {
    if !xp::day_valid(&baseline.day)
        || !required(&baseline.actor)
        || !required(&baseline.reason)
        || !required(&baseline.evidence)
    {
        return Err(error(
            "PLAYER_POINTS_BASELINE_INVALID",
            "Opening balance needs a valid date, recorder, reason and evidence reference.",
        ));
    }
    let balance = xp::amount(&baseline.balance)?;
    if balance < 0 {
        return Err(error(
            "PLAYER_POINTS_BASELINE_INVALID",
            "Opening balance cannot be negative.",
        ));
    }
    Ok(balance)
}

fn command_fields(command: &Command, baseline_day: &str) -> Result<()> {
    if !uuid_valid(&command.id) {
        return Err(error(
            "PLAYER_POINTS_ID_INVALID",
            "Command ID must be a canonical UUID.",
        ));
    }
    if !xp::day_valid(&command.day)
        || command.day.as_str() < baseline_day
        || !required(&command.actor)
        || !required(&command.reason)
        || !required(&command.evidence)
    {
        return Err(error(
            "PLAYER_POINTS_FIELDS_REQUIRED",
            "Entry needs a valid date on/after opening, recorder, reason and evidence reference.",
        ));
    }
    Ok(())
}

fn ordinary(command: &Command, baseline_day: &str) -> Result<Command> {
    command_fields(command, baseline_day)?;
    if command.kind == Kind::Reversal || command.reversal_of.is_some() {
        return Err(error(
            "PLAYER_POINTS_REVERSAL_TARGET",
            "Only reversals may reference an entry.",
        ));
    }
    let amount = xp::amount(&command.amount)?;
    if amount == 0 || (command.kind != Kind::Adjustment && amount < 0) {
        return Err(error(
            "PLAYER_POINTS_AMOUNT_INVALID",
            "Awards and spending must be positive; adjustments must be signed and nonzero.",
        ));
    }
    let mut canonical = command.clone();
    canonical.amount = xp::decimal(amount);
    Ok(canonical)
}

pub(crate) fn delta(command: &Command) -> Result<i64> {
    let amount = xp::amount(&command.amount)?;
    Ok(match command.kind {
        Kind::Award | Kind::Adjustment | Kind::Reversal => amount,
        Kind::Spend => -amount,
    })
}

pub fn open(mut baseline: Baseline) -> Result<Ledger> {
    baseline.balance = xp::decimal(baseline_units(&baseline)?);
    Ok(Ledger {
        baseline,
        entries: Vec::new(),
    })
}

fn verify_entry(
    command: &Command,
    baseline_day: &str,
    seen: &BTreeMap<String, Command>,
    reversed: &mut BTreeSet<String>,
) -> Result<i64> {
    command_fields(command, baseline_day)?;
    if seen.contains_key(&command.id) {
        return Err(error(
            "PLAYER_POINTS_DUPLICATE_ID",
            "Entry IDs must be unique.",
        ));
    }
    if command.kind != Kind::Reversal {
        let canonical = ordinary(command, baseline_day)?;
        if canonical != *command {
            return Err(error(
                "PLAYER_POINTS_CORRUPT",
                "Entry amount is not canonical.",
            ));
        }
        return delta(command);
    }

    let target_id = command
        .reversal_of
        .as_deref()
        .filter(|id| uuid_valid(id))
        .ok_or_else(|| {
            error(
                "PLAYER_POINTS_REVERSAL_TARGET",
                "A reversal must identify an original entry.",
            )
        })?;
    let target = seen.get(target_id).ok_or_else(|| {
        error(
            "PLAYER_POINTS_REVERSAL_TARGET",
            "Reversal target must precede the reversal.",
        )
    })?;
    if target.kind == Kind::Reversal || command.day < target.day {
        return Err(error(
            "PLAYER_POINTS_REVERSAL_TARGET",
            "Only an original entry may be reversed, on or after its date.",
        ));
    }
    if reversed.contains(target_id) {
        return Err(error(
            "PLAYER_POINTS_ALREADY_REVERSED",
            "An entry can be reversed only once.",
        ));
    }
    let amount = xp::amount(&command.amount)?;
    let expected = -delta(target)?;
    if amount != expected || command.amount != xp::decimal(amount) {
        return Err(error(
            "PLAYER_POINTS_CORRUPT",
            "Stored reversal amount must be the canonical signed delta that undoes its target.",
        ));
    }
    reversed.insert(target_id.to_owned());
    Ok(amount)
}

/// Replays entries by effective date, preserving append order for same-day transitions.
pub fn replay(ledger: &Ledger) -> Result<String> {
    let baseline = baseline_units(&ledger.baseline)?;
    if ledger.baseline.balance != xp::decimal(baseline) || ledger.entries.len() > ENTRY_LIMIT {
        return Err(error(
            "PLAYER_POINTS_CORRUPT",
            "Baseline or ledger exceeds canonical limits.",
        ));
    }
    let mut seen = BTreeMap::new();
    let mut reversed = BTreeSet::new();
    let mut timeline = Vec::with_capacity(ledger.entries.len());
    for (sequence, command) in ledger.entries.iter().enumerate() {
        let change = verify_entry(command, &ledger.baseline.day, &seen, &mut reversed)?;
        seen.insert(command.id.clone(), command.clone());
        timeline.push((command.day.as_str(), sequence, change));
    }
    timeline.sort_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(&b.1)));
    let mut balance = i128::from(baseline);
    for (_, _, change) in timeline {
        balance += i128::from(change);
        if balance < 0 {
            return Err(error(
                "PLAYER_POINTS_NEGATIVE_HISTORY",
                "An entry would create a negative historical Player points balance.",
            ));
        }
        if balance > i128::from(xp::LIMIT) {
            return Err(error(
                "PLAYER_POINTS_LIMIT",
                "Running Player points balance exceeds storage limits.",
            ));
        }
    }
    Ok(xp::decimal(balance as i64))
}

fn reversal_input(ledger: &Ledger, input: &Command) -> Result<Command> {
    command_fields(input, &ledger.baseline.day)?;
    if !input.amount.trim().is_empty() {
        return Err(error(
            "PLAYER_POINTS_REVERSAL_AMOUNT",
            "Leave reversal amount blank; it is derived from the target entry.",
        ));
    }
    let target_id = input
        .reversal_of
        .as_deref()
        .filter(|id| uuid_valid(id))
        .ok_or_else(|| {
            error(
                "PLAYER_POINTS_REVERSAL_TARGET",
                "Select an original entry to reverse.",
            )
        })?;
    let target = ledger
        .entries
        .iter()
        .find(|entry| entry.id == target_id)
        .ok_or_else(|| {
            error(
                "PLAYER_POINTS_REVERSAL_TARGET",
                "Select an existing entry to reverse.",
            )
        })?;
    if target.kind == Kind::Reversal || input.day < target.day {
        return Err(error(
            "PLAYER_POINTS_REVERSAL_TARGET",
            "Only an original entry may be reversed, on or after its date.",
        ));
    }
    if ledger.entries.iter().any(|entry| {
        entry.kind == Kind::Reversal && entry.reversal_of.as_deref() == Some(target_id)
    }) {
        return Err(error(
            "PLAYER_POINTS_ALREADY_REVERSED",
            "An entry can be reversed only once.",
        ));
    }
    let mut command = input.clone();
    command.amount = xp::decimal(-delta(target)?);
    Ok(command)
}

fn same_reversal_request(input: &Command, saved: &Command) -> bool {
    input.id == saved.id
        && input.kind == saved.kind
        && input.day == saved.day
        && input.actor == saved.actor
        && input.reason == saved.reason
        && input.evidence == saved.evidence
        && input.reversal_of == saved.reversal_of
        && (input.amount.trim().is_empty() || input.amount == saved.amount)
}

/// Adds one manually declared entry; validation failure leaves the input ledger unchanged.
pub fn apply(ledger: &Ledger, input: &Command) -> Result<Ledger> {
    replay(ledger)?;
    if let Some(saved) = ledger.entries.iter().find(|entry| entry.id == input.id) {
        let same = if input.kind == Kind::Reversal && saved.kind == Kind::Reversal {
            same_reversal_request(input, saved)
        } else {
            ordinary(input, &ledger.baseline.day).is_ok_and(|canonical| canonical == *saved)
        };
        if same {
            return Ok(ledger.clone());
        }
        return Err(error(
            "PLAYER_POINTS_COMMAND_CONFLICT",
            "This command ID already identifies different data.",
        ));
    }

    let command = if input.kind == Kind::Reversal {
        reversal_input(ledger, input)?
    } else {
        ordinary(input, &ledger.baseline.day)?
    };
    let mut updated = ledger.clone();
    updated.entries.push(command);
    replay(&updated)?;
    Ok(updated)
}
