//! Paired clerical correction, not a game-rule refund or a new purchase price.
use crate::{
    model::*,
    purchases, sheet,
    trait_advancement::{self as advance, Target},
    xp,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub advancement_id: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub library_id: String,
    pub record_id: String,
    pub revision: i64,
    pub record_hash: String,
    pub request: Request,
    pub advancement: advance::Quote,
    pub refund: String,
    pub before_unspent: String,
    pub after_unspent: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub quote: Quote,
    pub committed_revision: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
enum Evidence {
    #[serde(rename = "trait_correction_v1")]
    Correction { quote: Quote },
}
pub fn recorded_quote(command: &xp::XpCommand) -> Option<Quote> {
    let Evidence::Correction { quote } = serde_json::from_str(&command.evidence).ok()?;
    (command.kind == xp::XpKind::Reversal
        && command.id == quote.request.id
        && command.reversal_of.as_deref() == Some(quote.request.advancement_id.as_str())
        && command.amount == quote.refund)
        .then_some(quote)
}
pub fn corrected_ids(ledger: &xp::Ledger) -> BTreeSet<String> {
    ledger
        .entries
        .iter()
        .filter_map(|entry| recorded_quote(&entry.command).map(|q| q.request.advancement_id))
        .collect()
}
pub fn evidence_summary(command: &xp::XpCommand, pt: bool) -> Option<String> {
    let q = recorded_quote(command)?;
    Some(format!(
        "{}: {} {} -> {}; {} {} XP. {}",
        if pt { "Correção" } else { "Correction" },
        q.advancement.label,
        q.advancement.price.next,
        q.advancement.price.current,
        if pt {
            "Reembolso original"
        } else {
            "Original refund"
        },
        q.refund,
        q.request.evidence
    ))
}
fn restored_field(record: &Record, grant: &advance::Quote) -> Result<Value> {
    let ledger = record
        .xp_ledger
        .as_ref()
        .ok_or_else(|| error("XP_OPENING_REQUIRED", "The original XP ledger is required."))?;
    let corrected = corrected_ids(ledger);
    let active = ledger
        .entries
        .iter()
        .filter_map(|entry| advance::recorded_quote(&entry.command))
        .filter(|q| {
            !corrected.contains(&q.request.id) && q.field == grant.field && q.label == grant.label
        })
        .collect::<Vec<_>>();
    if active.last().map(|q| q.request.id.as_str()) != Some(grant.request.id.as_str()) {
        return Err(error(
            "TRAIT_CORRECTION_DEPENDENCY",
            "Correct the most recent active increase on this target first.",
        ));
    }
    let fields = sheet::extract(record);
    match &grant.request.target {
        Target::Resource { .. } => {
            if fields.resources.get(&grant.field) != Some(&grant.price.next) {
                return Err(error(
                    "TRAIT_CORRECTION_CHANGED",
                    "The permanent count changed after that increase; review it manually.",
                ));
            }
            Ok(json!(grant.price.current))
        }
        Target::Trait { index, .. } => {
            let mut list = fields.lists.get(&grant.field).cloned().ok_or_else(|| {
                error(
                    "TRAIT_CORRECTION_CHANGED",
                    "The original trait list is no longer readable.",
                )
            })?;
            let rows = list["Traits"].as_array_mut().unwrap();
            let matches = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row["Name"].as_str().is_some_and(|name| {
                        name.trim().to_lowercase() == grant.label.trim().to_lowercase()
                    })
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(error(
                    "TRAIT_CORRECTION_CHANGED",
                    "The original trait is missing or ambiguous.",
                ));
            }
            let slot = matches[0];
            let count = grant.price.next.to_string();
            if rows[slot]["Name"] != grant.label
                || rows[slot]["Value"].as_str() != Some(count.as_str())
                || rows[slot]["AppTraitAdvancementId"] != grant.request.id
            {
                return Err(error(
                    "TRAIT_CORRECTION_CHANGED",
                    "The trait no longer matches that recorded increase.",
                ));
            }
            if index.is_none() {
                let expected = json!({"Name":grant.label,"Value":count,"Note":"","AppTraitAdvancementId":grant.request.id});
                if rows[slot] != expected {
                    return Err(error(
                        "TRAIT_CORRECTION_CHANGED",
                        "This newly created trait has later notes or metadata; removing it would lose those changes.",
                    ));
                }
                rows.remove(slot);
            } else {
                rows[slot]["Value"] = json!(grant.price.current.to_string());
                if let Some(previous) = active.iter().rev().nth(1) {
                    rows[slot]["AppTraitAdvancementId"] = json!(previous.request.id);
                } else {
                    rows[slot]
                        .as_object_mut()
                        .unwrap()
                        .remove("AppTraitAdvancementId");
                }
            }
            Ok(list)
        }
    }
}
fn command(q: &Quote) -> Result<xp::XpCommand> {
    Ok(xp::XpCommand {
        id: q.request.id.clone(),
        kind: xp::XpKind::Reversal,
        amount: q.refund.clone(),
        day: q.request.day.clone(),
        actor: q.request.actor.clone(),
        reason: q.request.reason.clone(),
        evidence: serde_json::to_string(&Evidence::Correction { quote: q.clone() })?,
        reversal_of: Some(q.request.advancement_id.clone()),
        cost: None,
    })
}
fn refunded(record: &Record, q: &Quote, time: u64) -> Result<Record> {
    let mut updated = record.clone();
    let ledger = updated
        .xp_ledger
        .as_mut()
        .ok_or_else(|| error("XP_OPENING_REQUIRED", "The original XP ledger is required."))?;
    let command = xp::canonical_command(ledger, &command(q)?)?;
    if ledger
        .entries
        .iter()
        .any(|entry| entry.command.id == command.id)
    {
        return Err(error(
            "XP_DUPLICATE_ID",
            "This correction ID is already recorded.",
        ));
    }
    ledger.entries.push(xp::Entry {
        command,
        recorded_unix_seconds: time,
        origin_library_id: q.library_id.clone(),
    });
    xp::replay(ledger)?;
    Ok(updated)
}
pub fn quote(library_id: &str, revision: i64, record: &Record, request: &Request) -> Result<Quote> {
    if !crate::sheet::is_character_kind(&record.kind) || record.build.is_some() {
        return Err(error(
            "TRAIT_CORRECTION_SCOPE",
            "Only the current Vampire record can receive this paired correction.",
        ));
    }
    if uuid::Uuid::parse_str(&request.id)
        .ok()
        .is_none_or(|id| id.to_string() != request.id)
        || request.id == request.advancement_id
        || [
            (&request.actor, 200),
            (&request.reason, 512),
            (&request.evidence, 512),
        ]
        .iter()
        .any(|(v, max)| v.trim().is_empty() || v.len() > *max)
    {
        return Err(error(
            "TRAIT_CORRECTION_EVIDENCE",
            "Record a new correction ID, ST, reason and evidence reference.",
        ));
    }
    let ledger = record
        .xp_ledger
        .as_ref()
        .ok_or_else(|| error("XP_OPENING_REQUIRED", "The original XP ledger is required."))?;
    if ledger
        .entries
        .iter()
        .any(|entry| entry.command.reversal_of.as_deref() == Some(request.advancement_id.as_str()))
    {
        return Err(error(
            "TRAIT_ALREADY_CORRECTED",
            "This debit is already reversed.",
        ));
    }
    let grant = ledger
        .entries
        .iter()
        .find(|entry| entry.command.id == request.advancement_id)
        .and_then(|entry| advance::recorded_quote(&entry.command))
        .ok_or_else(|| {
            error(
                "TRAIT_CORRECTION_TARGET",
                "Choose an original ordinary-trait advancement.",
            )
        })?;
    if grant.record_id != record.id || grant.library_id != library_id {
        return Err(error(
            "TRAIT_CORRECTION_ORIGIN",
            "Copied or foreign ledger entries need separate review.",
        ));
    }
    let _ = restored_field(record, &grant)?;
    let (_, before) = xp::replay(ledger)?;
    let q = Quote {
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        request: request.clone(),
        refund: grant.price.cost.clone(),
        advancement: grant,
        before_unspent: xp::decimal(before),
        after_unspent: String::new(),
    };
    let updated = refunded(record, &q, 0)?;
    Ok(Quote {
        after_unspent: xp::decimal(xp::replay(updated.xp_ledger.as_ref().unwrap())?.1),
        ..q
    })
}
pub fn apply(record: &Record, q: &Quote, time: u64) -> Result<Record> {
    if quote(&q.library_id, q.revision, record, &q.request)? != *q {
        return Err(error(
            "TRAIT_CORRECTION_CHANGED",
            "The character or correction quote changed; review it again.",
        ));
    }
    let value = restored_field(record, &q.advancement)?;
    let mut updated = refunded(record, q, time)?;
    sheet::write_field(&mut updated, &q.advancement.field, value);
    Ok(updated)
}
