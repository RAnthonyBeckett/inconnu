//! Bounded cancellation for locally supported purchased powers.
//!
//! A cancellation is accepted only while the purchase is the latest ledger event
//! and the rest of the committed record still reconstructs the purchase snapshot.
//! The XP debit remains in place; the appended reversal records the exact refund.
use crate::{
    importer,
    model::{Record, Result, error},
    purchases::{self, PurchaseReceipt},
    xp::{self, CostEvidence, Entry, Finding, XpCommand, XpKind},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancellationRequest {
    pub id: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancellationQuote {
    pub request: CancellationRequest,
    pub purchase_id: String,
    pub library_id: String,
    pub record_id: String,
    pub record_name: String,
    pub revision: i64,
    pub record_hash: String,
    pub before_unspent: String,
    pub after_unspent: String,
    pub refund: String,
    pub restored_public_list: Option<Value>,
    pub restored_private_list: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancellationReceipt {
    pub quote: CancellationQuote,
    pub committed_revision: i64,
}

fn required(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 16000
}

fn canonical_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value)
        .ok()
        .is_some_and(|parsed| parsed.to_string() == value)
}

fn purchase_command(receipt: &PurchaseReceipt) -> XpCommand {
    let request = &receipt.quote.request;
    let price = &receipt.quote.price.price;
    XpCommand {
        id: request.id.clone(),
        kind: XpKind::Spend,
        amount: price.total.clone(),
        day: request.day.clone(),
        actor: request.actor.clone(),
        reason: request.reason.clone(),
        evidence: format!(
            "Purchase receipt {}; {}",
            request.id, request.approval_reference
        ),
        reversal_of: None,
        cost: Some(CostEvidence {
            affiliation_name: request.affiliation_name.clone(),
            power_family: receipt.quote.price.cost_family_label(),
            approval_reference: request.approval_reference.clone(),
            base: price.base.clone(),
            affiliation_adjustment: price.affiliation_adjustment.clone(),
            affiliation: request
                .relation
                .map(|relation| format!("{relation:?}"))
                .unwrap_or_else(|| "Unknown".into()),
            source: price.citation.book.clone(),
            edition: price.citation.edition.clone(),
            page: price.citation.printed_page.to_string(),
            policy_version: request.policy_version.clone(),
        }),
    }
}

fn reversal_command(request: &CancellationRequest, purchase_id: &str, refund: &str) -> XpCommand {
    XpCommand {
        id: request.id.clone(),
        kind: XpKind::Reversal,
        amount: refund.into(),
        day: request.day.clone(),
        actor: request.actor.clone(),
        reason: request.reason.clone(),
        evidence: request.evidence.clone(),
        reversal_of: Some(purchase_id.into()),
        cost: None,
    }
}

fn public_projection(record: &Record, list: &Value) -> Value {
    let mut safe = importer::redact(list);
    if let Some((start, end)) = &record.st_markers {
        safe = importer::redact_custom(&safe, start, end);
    }
    safe
}

fn candidate_with_lists(
    record: &Record,
    field: &str,
    public_list: Option<Value>,
    private_list: Option<Value>,
) -> Record {
    let mut candidate = record.clone();
    match public_list {
        Some(value) => {
            candidate.public_fields.insert(field.into(), value);
        }
        None => {
            candidate.public_fields.remove(field);
        }
    }
    match private_list {
        Some(value) => {
            candidate.private_fields.insert(field.into(), value);
        }
        None => {
            candidate.private_fields.remove(field);
        }
    }
    candidate
}

/// Reconstructs the exact pre-purchase list mapping. Each possible mapping follows
/// the purchase writer's public/private rules, and the original receipt hash must
/// identify exactly one candidate.
fn reconstruct_purchase_snapshot(
    record: &Record,
    purchase: &PurchaseReceipt,
    expected_spend: &XpCommand,
) -> Result<(Option<Value>, Option<Value>)> {
    let field = purchase.quote.price.target_list();
    let source = record
        .private_fields
        .get(field)
        .or_else(|| record.public_fields.get(field))
        .ok_or_else(|| {
            error(
                "CANCEL_SNAPSHOT_UNKNOWN",
                "The purchased power list is missing; the original list mapping cannot be reconstructed safely.",
            )
        })?;
    if !crate::sheet::editable_list(source) {
        return Err(error(
            "CANCEL_SNAPSHOT_UNKNOWN",
            "The power list has an unsupported shape; cancellation cannot infer its prior mapping.",
        ));
    }
    let mut after_purchase_list = source.clone();
    let traits = after_purchase_list["Traits"].as_array_mut().unwrap();
    let matches: Vec<usize> = traits
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item["AppPurchaseId"].as_str() == Some(purchase.quote.request.id.as_str())
        })
        .map(|(index, _)| index)
        .collect();
    if matches.len() != 1 || traits[matches[0]] != purchase.quote.trait_added {
        return Err(error(
            "CANCEL_SNAPSHOT_UNKNOWN",
            "The purchased trait does not match its receipt exactly.",
        ));
    }
    traits.remove(matches[0]);

    let public_present = record.public_fields.contains_key(field);
    let private_present = record.private_fields.contains_key(field);
    let mut after_purchase_candidate = record.clone();
    let ledger = after_purchase_candidate.xp_ledger.as_mut().ok_or_else(|| {
        error(
            "CANCEL_SNAPSHOT_UNKNOWN",
            "The XP ledger is missing; the purchase snapshot cannot be reconstructed.",
        )
    })?;
    if ledger.entries.last().is_none_or(|entry| {
        entry.command != *expected_spend || entry.origin_library_id != purchase.quote.library_id
    }) {
        return Err(error(
            "CANCEL_PURCHASE_ORDER",
            "The purchase debit is no longer the last XP event.",
        ));
    }
    ledger.entries.pop();

    let mut candidates = Vec::new();
    if public_present {
        let current_public = record.public_fields.get(field).unwrap();
        let expected_public_after_purchase = public_projection(record, source);
        if &expected_public_after_purchase != current_public {
            return Err(error(
                "CANCEL_SNAPSHOT_UNKNOWN",
                "The public power list no longer matches the saved privacy projection.",
            ));
        }
        if private_present {
            // Before purchase, a private source was mirrored through the public policy.
            candidates.push((
                Some(public_projection(record, &after_purchase_list)),
                Some(after_purchase_list.clone()),
            ));
            // Before purchase, a public-only source became private when the purchase
            // trait introduced fields that the public projection removes.
            candidates.push((Some(after_purchase_list.clone()), None));
        } else {
            candidates.push((Some(after_purchase_list.clone()), None));
        }
    } else if private_present {
        // Private-only list, or no list at all (the purchase writer starts from empty_list).
        candidates.push((None, Some(after_purchase_list.clone())));
        if after_purchase_list == crate::sheet::empty_list(field) {
            candidates.push((None, None));
        }
    } else {
        return Err(error(
            "CANCEL_SNAPSHOT_UNKNOWN",
            "The current field mapping could not have been produced by the supported purchase flow.",
        ));
    }

    let mut matches = Vec::new();
    for (public_list, private_list) in candidates {
        if matches
            .iter()
            .any(|(public, private): &(Option<Value>, Option<Value>)| {
                public == &public_list && private == &private_list
            })
        {
            continue;
        }
        let candidate = candidate_with_lists(
            &after_purchase_candidate,
            field,
            public_list.clone(),
            private_list.clone(),
        );
        if purchases::record_hash(&candidate)? == purchase.quote.record_hash {
            matches.push((public_list, private_list));
        }
    }
    if matches.len() != 1 {
        return Err(error(
            "CANCEL_SNAPSHOT_UNKNOWN",
            "The prior power-list mapping cannot be identified uniquely from the purchase snapshot.",
        ));
    }
    Ok(matches.pop().unwrap())
}

struct ValidatedPurchase {
    refund: String,
    before: i64,
    restored_public_list: Option<Value>,
    restored_private_list: Option<Value>,
}

fn validate_purchase_for_cancellation(
    library_id: &str,
    revision: i64,
    record: &Record,
    purchase: &PurchaseReceipt,
) -> Result<ValidatedPurchase> {
    if purchase.cancellation.is_some() {
        return Err(error(
            "CANCEL_ALREADY_RECORDED",
            "This purchase already has a cancellation receipt.",
        ));
    }
    if !crate::sheet::is_character_kind(&record.kind)
        || record.build.is_some()
        || record.id != purchase.quote.record_id
        || record.name != purchase.quote.record_name
        || library_id != purchase.quote.library_id
    {
        return Err(error(
            "CANCEL_SCOPE",
            "Only the live record and library that own this purchase receipt can cancel it.",
        ));
    }
    purchase
        .quote
        .price
        .validate_scope(&record.kind)
        .map_err(|_| {
            error(
                "CANCEL_SCOPE",
                "Only the live record and library that own this purchase receipt can cancel it.",
            )
        })?;
    if revision < purchase.committed_revision || revision < 0 {
        return Err(error(
            "CANCEL_STALE",
            "The cancellation revision predates the committed purchase.",
        ));
    }
    if !purchases::findings(record, purchase).is_empty() {
        return Err(error(
            "CANCEL_PURCHASE_DRIFT",
            "The purchase receipt, trait, or XP debit has changed; resolve that review before cancellation.",
        ));
    }
    let ledger = record.xp_ledger.as_ref().ok_or_else(|| {
        error(
            "CANCEL_PURCHASE_DRIFT",
            "The purchase XP ledger is missing.",
        )
    })?;
    let expected_spend = purchase_command(purchase);
    let linked: Vec<_> = ledger
        .entries
        .iter()
        .filter(|entry| entry.command.id == purchase.quote.request.id)
        .collect();
    let last = ledger.entries.last().ok_or_else(|| {
        error(
            "CANCEL_PURCHASE_DRIFT",
            "The purchase debit is missing from the XP ledger.",
        )
    })?;
    if linked.len() != 1
        || last.command != expected_spend
        || last.origin_library_id != purchase.quote.library_id
    {
        return Err(error(
            "CANCEL_PURCHASE_ORDER",
            "Cancellation is available only while the exact purchase debit remains the last XP event.",
        ));
    }
    let refund_units = xp::amount(&last.command.amount)?;
    let refund = xp::decimal(refund_units);
    let (_, before) = xp::replay(ledger)?;
    let (restored_public, restored_private) =
        reconstruct_purchase_snapshot(record, purchase, &expected_spend)?;
    Ok(ValidatedPurchase {
        refund,
        before,
        restored_public_list: restored_public,
        restored_private_list: restored_private,
    })
}

pub fn quote(
    library_id: &str,
    revision: i64,
    record: &Record,
    purchase: &PurchaseReceipt,
    input: &CancellationRequest,
) -> Result<CancellationQuote> {
    if purchase
        .batch
        .as_ref()
        .is_some_and(|binding| binding.count > 1)
    {
        return Err(error(
            "BATCH_CANCELLATION_UNSUPPORTED",
            "This purchase belongs to a multi-purchase plan. Grouped cancellation requires a separate reviewed workflow; individual automatic cancellation is unavailable.",
        ));
    }
    if !canonical_uuid(&input.id)
        || input.id == purchase.quote.request.id
        || record.xp_ledger.as_ref().is_some_and(|ledger| {
            ledger
                .entries
                .iter()
                .any(|entry| entry.command.id == input.id)
        })
    {
        return Err(error(
            "CANCEL_ID",
            "Use a new canonical UUID that does not identify the purchase or an XP event.",
        ));
    }
    if !xp::day_valid(&input.day) || input.day < purchase.quote.request.day {
        return Err(error(
            "CANCEL_DATE",
            "Use a valid cancellation date on or after the purchase date.",
        ));
    }
    if !required(&input.actor) || !required(&input.reason) || !required(&input.evidence) {
        return Err(error(
            "CANCEL_EVIDENCE_REQUIRED",
            "Record an actor, reason, and evidence reference of at most 16,000 characters.",
        ));
    }

    let validated = validate_purchase_for_cancellation(library_id, revision, record, purchase)?;
    let mut trial = record.clone();
    let ledger = trial.xp_ledger.as_mut().unwrap();
    let command = xp::canonical_command(
        ledger,
        &reversal_command(input, &purchase.quote.request.id, &validated.refund),
    )?;
    ledger.entries.push(Entry {
        command,
        recorded_unix_seconds: 0,
        origin_library_id: library_id.into(),
    });
    let (_, after) = xp::replay(ledger)?;

    Ok(CancellationQuote {
        request: input.clone(),
        purchase_id: purchase.quote.request.id.clone(),
        library_id: library_id.into(),
        record_id: record.id.clone(),
        record_name: record.name.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        before_unspent: xp::decimal(validated.before),
        after_unspent: xp::decimal(after),
        refund: validated.refund,
        restored_public_list: validated.restored_public_list,
        restored_private_list: validated.restored_private_list,
    })
}

pub fn apply(
    record: &Record,
    quote: &CancellationQuote,
    purchase: &PurchaseReceipt,
    recorded_seconds: u64,
) -> Result<Record> {
    if record.id != quote.record_id || purchases::record_hash(record)? != quote.record_hash {
        return Err(error(
            "CANCEL_STALE",
            "The committed record changed after this cancellation quote.",
        ));
    }
    let current = self::quote(
        &quote.library_id,
        quote.revision,
        record,
        purchase,
        &quote.request,
    )?;
    if current != *quote {
        return Err(error(
            "CANCEL_QUOTE_MISMATCH",
            "The cancellation quote no longer matches the purchase, refund, or restored list.",
        ));
    }

    let mut updated = candidate_with_lists(
        record,
        purchase.quote.price.target_list(),
        quote.restored_public_list.clone(),
        quote.restored_private_list.clone(),
    );
    let ledger = updated.xp_ledger.as_mut().ok_or_else(|| {
        error(
            "CANCEL_PURCHASE_DRIFT",
            "The purchase XP ledger is missing.",
        )
    })?;
    let command = xp::canonical_command(
        ledger,
        &reversal_command(&quote.request, &quote.purchase_id, &quote.refund),
    )?;
    ledger.entries.push(Entry {
        command,
        recorded_unix_seconds: recorded_seconds,
        origin_library_id: quote.library_id.clone(),
    });
    xp::replay(ledger)?;
    Ok(updated)
}

fn finding(code: &str, entry_id: Option<String>, message: &str) -> Finding {
    Finding {
        code: code.into(),
        severity: "review".into(),
        entry_id,
        message: message.into(),
    }
}

fn has_purchase_trait(record: &Record, purchase_id: &str) -> bool {
    crate::sheet::LIST_FIELDS.iter().any(|field| {
        [&record.private_fields, &record.public_fields]
            .into_iter()
            .filter_map(|fields| fields.get(*field))
            .filter_map(|list| list["Traits"].as_array())
            .flatten()
            .any(|item| item["AppPurchaseId"].as_str() == Some(purchase_id))
    })
}

fn cancellation_findings(
    record: &Record,
    purchase: &PurchaseReceipt,
    cancel: &CancellationReceipt,
) -> (Vec<Finding>, bool) {
    let mut out = Vec::new();
    let mut valid = true;
    let q = &cancel.quote;
    let purchase_id = &purchase.quote.request.id;
    let cancel_id = &q.request.id;
    let add = |out: &mut Vec<Finding>, valid: &mut bool, code: &str, message: &str| {
        *valid = false;
        out.push(finding(code, Some(cancel_id.clone()), message));
    };

    if purchase.cancellation.as_ref() != Some(cancel)
        || q.purchase_id != *purchase_id
        || q.library_id != purchase.quote.library_id
        || q.record_id != purchase.quote.record_id
        || record.id != purchase.quote.record_id
    {
        add(
            &mut out,
            &mut valid,
            "CANCEL_RECEIPT_MISMATCH",
            "The cancellation metadata does not match the stored local purchase receipt.",
        );
    }

    let ledger = record.xp_ledger.as_ref();
    let expected_spend = purchase_command(purchase);
    let original_spends: Vec<_> = ledger
        .into_iter()
        .flat_map(|value| &value.entries)
        .filter(|entry| entry.command.id == *purchase_id)
        .collect();
    let original_spend = original_spends.first().copied();
    let original_refund = original_spend
        .and_then(|entry| xp::amount(&entry.command.amount).ok())
        .map(xp::decimal);
    if original_spends.len() != 1
        || original_spend.is_none_or(|entry| {
            entry.command != expected_spend || entry.origin_library_id != purchase.quote.library_id
        })
    {
        add(
            &mut out,
            &mut valid,
            "CANCEL_PURCHASE_SPEND_MISMATCH",
            "The original XP debit is missing or differs from its purchase receipt.",
        );
    }

    let request_valid = canonical_uuid(cancel_id)
        && cancel_id != purchase_id
        && xp::day_valid(&q.request.day)
        && q.request.day >= purchase.quote.request.day
        && required(&q.request.actor)
        && required(&q.request.reason)
        && required(&q.request.evidence);
    if !request_valid
        || q.refund != original_refund.unwrap_or_default()
        || q.record_name != purchase.quote.record_name
        || q.revision < purchase.committed_revision
    {
        add(
            &mut out,
            &mut valid,
            "CANCEL_QUOTE_MISMATCH",
            "The cancellation quote has invalid request, revision, or exact-refund details.",
        );
    }

    let matching_refunds: Vec<_> = ledger
        .into_iter()
        .flat_map(|value| &value.entries)
        .filter(|entry| entry.command.id == *cancel_id)
        .collect();
    let reverse_links: Vec<_> = ledger
        .into_iter()
        .flat_map(|value| &value.entries)
        .filter(|entry| entry.command.reversal_of.as_deref() == Some(purchase_id.as_str()))
        .collect();
    let expected_refund = ledger.and_then(|value| {
        xp::canonical_command(value, &reversal_command(&q.request, purchase_id, &q.refund)).ok()
    });
    if matching_refunds.len() != 1
        || reverse_links.len() != 1
        || expected_refund.is_none()
        || matching_refunds.first().is_none_or(|entry| {
            Some(&entry.command) != expected_refund.as_ref()
                || entry.origin_library_id != q.library_id
        })
    {
        add(
            &mut out,
            &mut valid,
            "CANCEL_REFUND_MISMATCH",
            "The exact XP refund command is missing or differs from the cancellation receipt.",
        );
    }

    if has_purchase_trait(record, purchase_id) {
        add(
            &mut out,
            &mut valid,
            "CANCEL_TRAIT_REAPPEARED",
            "The cancelled purchase trait ID is still present in a supported power list.",
        );
    }

    if ledger.is_none_or(|value| xp::replay(value).is_err()) {
        add(
            &mut out,
            &mut valid,
            "CANCEL_LEDGER_INVALID",
            "The XP ledger cannot be replayed with this cancellation.",
        );
    }
    (out, valid)
}

pub fn findings(
    record: &Record,
    purchase: &PurchaseReceipt,
    cancel: &CancellationReceipt,
) -> Vec<Finding> {
    let (mut out, valid_cancellation) = cancellation_findings(record, purchase, cancel);
    let mut uncancelled = purchase.clone();
    uncancelled.cancellation = None;
    let mut purchase_findings = purchases::findings(record, &uncancelled);
    if valid_cancellation {
        purchase_findings.retain(|item| {
            item.code != "PURCHASE_TRAIT_DRIFT" && item.code != "PURCHASE_SPEND_REVERSED"
        });
    }
    out.extend(purchase_findings);
    out
}
