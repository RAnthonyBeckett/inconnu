//! Deterministic, source-backed quotes for an atomic group of purchases.
//!
//! This module simulates the existing single-purchase quote and apply path in order.
//! Storage must recompute the steps against the live record inside its transaction.

use crate::{
    model::{Record, Result, error},
    purchases::{self, PurchaseQuote, PurchaseRequest},
    xp,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const MAX_STEPS: usize = 32;
const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub library_id: String,
    pub record_id: String,
    pub revision: i64,
    pub record_hash: String,
    pub steps: Vec<PurchaseQuote>,
    pub before_unspent: String,
    pub after_unspent: String,
    pub total_spend: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub quote_hash: String,
    pub index: usize,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub quote: Quote,
    pub receipts: Vec<purchases::PurchaseReceipt>,
}

/// Build a deterministic quote by simulating each purchase against the prior step.
///
/// Reviews are rejected explicitly: the current review binding covers one exact record
/// snapshot and becomes stale as soon as an earlier purchase changes that record.
pub fn quote(
    library_id: &str,
    revision: i64,
    record: &Record,
    requests: &[PurchaseRequest],
) -> Result<Quote> {
    if requests.is_empty() {
        return Err(error("BATCH_EMPTY", "Select at least one purchase."));
    }
    if requests.len() > MAX_STEPS {
        return Err(error(
            "BATCH_LIMIT",
            format!("A purchase batch can contain at most {MAX_STEPS} items."),
        ));
    }
    if serde_json::to_vec(requests)?.len() > MAX_REQUEST_BYTES {
        return Err(error(
            "BATCH_REQUEST_LIMIT",
            "The purchase batch request exceeds the supported size.",
        ));
    }

    let mut ids = HashSet::with_capacity(requests.len());
    for request in requests {
        if request.review_id.is_some() {
            return Err(error(
                "BATCH_FRESH_REVIEW",
                "Batch purchases need fresh per-item review evidence; a single-record review cannot be reused across changing steps.",
            ));
        }
        if !ids.insert(request.id.as_str()) {
            return Err(error(
                "BATCH_DUPLICATE_ID",
                "Every purchase in a batch needs a distinct request ID.",
            ));
        }
    }

    let base_hash = purchases::record_hash(record)?;
    let mut working = record.clone();
    let mut steps = Vec::with_capacity(requests.len());
    for request in requests {
        let step = purchases::quote(library_id, revision, &working, request)?;
        working = purchases::apply(&working, &step, 0)?;
        steps.push(step);
    }

    let total_units = steps.iter().try_fold(0_i64, |sum, step| {
        sum.checked_add(xp::amount(&step.price.price.total)?)
            .ok_or_else(|| {
                error(
                    "BATCH_XP_LIMIT",
                    "The total purchase cost exceeds supported XP arithmetic.",
                )
            })
    })?;

    Ok(Quote {
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: base_hash,
        before_unspent: steps[0].before_unspent.clone(),
        after_unspent: steps[steps.len() - 1].after_unspent.clone(),
        total_spend: xp::decimal(total_units),
        steps,
    })
}

/// Hash the exact serialized batch quote for transaction-time preview binding.
pub fn hash(quote: &Quote) -> Result<String> {
    let digest = Sha256::digest(serde_json::to_vec(quote)?);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::Record,
        pricing::{ClanRelation, PricingProfile},
        purchase_preview::{ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID},
        purchases::{PurchaseRequest, TeacherRule},
        xp::{Baseline, Ledger},
    };
    use serde_json::{Map, json};

    fn record(unspent: &str) -> Record {
        Record {
            id: "00000000-0000-4000-8000-000000000010".into(),
            kind: "Vampire".into(),
            name: "Synthetic Vampire".into(),
            player_id: None,
            legacy_id: None,
            public_fields: Map::new(),
            private_fields: Map::new(),
            original: json!({"fixture":"purchase batch"}),
            source_sha256: "a".repeat(64),
            st_markers: None,
            build: None,
            xp_ledger: Some(Ledger {
                baseline: Baseline {
                    day: "2026-01-01".into(),
                    earned: unspent.into(),
                    unspent: unspent.into(),
                    actor: "Synthetic ST".into(),
                    reason: "Synthetic opening".into(),
                    evidence: "Synthetic fixture".into(),
                },
                entries: vec![],
            }),
        }
    }

    fn request(power_id: &str, number: u128) -> PurchaseRequest {
        PurchaseRequest {
            review_id: None,
            id: uuid::Uuid::from_u128(number).to_string(),
            power_id: power_id.into(),
            profile: Some(PricingProfile::MetLawsOfTheNightRevised),
            relation: Some(ClanRelation::OutOfClan),
            affiliation_name: "Synthetic clan".into(),
            day: "2026-01-02".into(),
            available_from: "2026-01-03".into(),
            next_session_reference: "Synthetic next session".into(),
            actor: "Synthetic ST".into(),
            reason: "Synthetic batch purchase".into(),
            policy_version: "synthetic-v1".into(),
            policy_reference: "Synthetic local policy".into(),
            affiliation_evidence: "Synthetic affiliation evidence".into(),
            approval_reference: "Synthetic approval reference".into(),
            eligibility_evidence: "Synthetic eligibility evidence".into(),
            teacher_rule: Some(TeacherRule::Required),
            teacher_evidence: "Synthetic teacher evidence".into(),
            standard_cost_confirmed: true,
        }
    }

    #[test]
    fn quote_simulates_in_order_and_is_deterministically_hashable() {
        let record = record("10.25");
        let original_hash = purchases::record_hash(&record).unwrap();
        let requests = [
            request(ANIMALISM_FERAL_WHISPERS_ID, 1),
            request(ANIMALISM_BECKONING_ID, 2),
        ];

        let first = quote("synthetic-library", 7, &record, &requests).unwrap();
        let second = quote("synthetic-library", 7, &record, &requests).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.record_hash, original_hash);
        assert_eq!(first.record_id, record.id);
        assert_eq!(first.revision, 7);
        assert_eq!(first.before_unspent, "10.25");
        assert_eq!(first.after_unspent, "2.25");
        assert_eq!(first.total_spend, "8");
        assert_eq!(first.steps.len(), 2);
        assert_eq!(first.steps[0].before_unspent, "10.25");
        assert_eq!(first.steps[0].after_unspent, "6.25");
        assert_eq!(first.steps[1].before_unspent, "6.25");
        assert_eq!(first.steps[1].after_unspent, "2.25");
        assert_ne!(first.steps[0].record_hash, first.steps[1].record_hash);
        assert_eq!(hash(&first).unwrap(), hash(&second).unwrap());

        // Quoting is a pure simulation; it never mutates the caller's record.
        assert_eq!(purchases::record_hash(&record).unwrap(), original_hash);
    }

    #[test]
    fn cumulative_balance_and_duplicate_power_checks_apply_between_steps() {
        let insufficient = record("7.99");
        let before_hash = purchases::record_hash(&insufficient).unwrap();
        let requests = [
            request(ANIMALISM_FERAL_WHISPERS_ID, 1),
            request(ANIMALISM_BECKONING_ID, 2),
        ];
        assert_eq!(
            quote("synthetic-library", 1, &insufficient, &requests)
                .unwrap_err()
                .code,
            "PURCHASE_INSUFFICIENT_XP"
        );
        assert_eq!(purchases::record_hash(&insufficient).unwrap(), before_hash);

        let enough = record("20");
        let duplicate = [
            request(ANIMALISM_FERAL_WHISPERS_ID, 3),
            request(ANIMALISM_FERAL_WHISPERS_ID, 4),
        ];
        assert_eq!(
            quote("synthetic-library", 1, &enough, &duplicate)
                .unwrap_err()
                .code,
            "PURCHASE_ALREADY_PRESENT"
        );
    }

    #[test]
    fn request_bounds_and_review_binding_are_explicit() {
        let record = record("1000");
        assert_eq!(
            quote("synthetic-library", 1, &record, &[])
                .unwrap_err()
                .code,
            "BATCH_EMPTY"
        );

        let mut oversized = request(ANIMALISM_FERAL_WHISPERS_ID, 10);
        oversized.teacher_evidence = "x".repeat(1024 * 1024 + 1);
        assert_eq!(
            quote("synthetic-library", 1, &record, &[oversized])
                .unwrap_err()
                .code,
            "BATCH_REQUEST_LIMIT"
        );

        let too_many = (1..=33)
            .map(|number| request(ANIMALISM_FERAL_WHISPERS_ID, number))
            .collect::<Vec<_>>();
        assert_eq!(
            quote("synthetic-library", 1, &record, &too_many)
                .unwrap_err()
                .code,
            "BATCH_LIMIT"
        );

        let mut reviewed = request(ANIMALISM_FERAL_WHISPERS_ID, 50);
        reviewed.review_id = Some("saved-single-record-review".into());
        assert_eq!(
            quote("synthetic-library", 1, &record, &[reviewed])
                .unwrap_err()
                .code,
            "BATCH_FRESH_REVIEW"
        );
    }

    #[test]
    fn duplicate_request_ids_are_rejected_before_simulation() {
        let record = record("20");
        let requests = [
            request(ANIMALISM_FERAL_WHISPERS_ID, 99),
            request(ANIMALISM_BECKONING_ID, 99),
        ];
        assert_eq!(
            quote("synthetic-library", 1, &record, &requests)
                .unwrap_err()
                .code,
            "BATCH_DUPLICATE_ID"
        );
    }
}
