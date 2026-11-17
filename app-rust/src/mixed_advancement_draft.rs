use crate::mixed_advancement::Request as PurchaseRequest;
// Persistent V2 intent for mixed steps; V1 power-only drafts remain untouched.

use crate::{
    mixed_advancement::{self as purchase_batch, Quote},
    model::{BuildKind, Record, Result, error},
    purchases,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FIELD: &str = "_AppMixedAdvancementPlanV2";
const MAX_METADATA_BYTES: usize = 2 * 1024 * 1024;
const MAX_REQUESTS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub version: u32,
    pub library_id: String,
    pub source_id: String,
    pub source_hash: String,
    pub draft_hash: String,
    pub requests: Vec<PurchaseRequest>,
    pub intent_hash: String,
    pub applied: Option<Quote>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub draft_id: String,
    pub draft_name: String,
    pub source_name: String,
    pub draft_hash: String,
    pub source_hash: String,
    pub quote: Quote,
    pub applied: bool,
}

/// Hash all Draft payload fields except this private plan metadata field.
pub fn payload_hash(record: &Record) -> Result<String> {
    if record.public_fields.contains_key(FIELD) {
        return Err(error(
            "ADVANCEMENT_PLAN_PRIVACY",
            "Advancement plan metadata must remain private.",
        ));
    }
    let mut payload = record.clone();
    payload.private_fields.remove(FIELD);
    purchases::record_hash(&payload)
}

/// Read and validate a saved plan without changing the Draft or its source.
pub fn metadata(record: &Record) -> Result<Metadata> {
    let build = record
        .build
        .as_ref()
        .filter(|build| build.kind == BuildKind::Draft)
        .ok_or_else(|| {
            error(
                "ADVANCEMENT_DRAFT_SCOPE",
                "A reviewed advancement plan must belong to a Draft copy.",
            )
        })?;
    if record.public_fields.contains_key(FIELD) {
        return Err(error(
            "ADVANCEMENT_PLAN_PRIVACY",
            "Advancement plan metadata must remain private.",
        ));
    }
    let value = record.private_fields.get(FIELD).ok_or_else(|| {
        error(
            "ADVANCEMENT_PLAN_MISSING",
            "This Draft has no saved advancement plan.",
        )
    })?;
    if serde_json::to_vec(value)?.len() > MAX_METADATA_BYTES {
        return Err(error(
            "ADVANCEMENT_PLAN_LIMIT",
            "Saved advancement plan metadata exceeds the supported size.",
        ));
    }
    let metadata: Metadata = serde_json::from_value(value.clone()).map_err(|_| {
        error(
            "ADVANCEMENT_PLAN_INVALID",
            "Saved advancement plan metadata needs inspection; no changes were made.",
        )
    })?;
    if metadata.version != 2
        || metadata.library_id.trim().is_empty()
        || metadata.source_id.trim().is_empty()
        || metadata.source_id != build.source_id
        || !valid_hash(&metadata.source_hash)
        || !valid_hash(&metadata.draft_hash)
        || !valid_hash(&metadata.intent_hash)
        || metadata.requests.is_empty()
        || metadata.requests.len() > MAX_REQUESTS
        || metadata
            .requests
            .iter()
            .any(|request| matches!(request, PurchaseRequest::Power(p) if p.review_id.is_some()))
    {
        return Err(error(
            "ADVANCEMENT_PLAN_INVALID",
            "Saved advancement plan identities or request bounds are invalid.",
        ));
    }
    if intent_hash(&metadata)? != metadata.intent_hash {
        return Err(error(
            "ADVANCEMENT_PLAN_INVALID",
            "Saved advancement requests changed after this plan was created.",
        ));
    }
    if payload_hash(record)? != metadata.draft_hash {
        return Err(error(
            "ADVANCEMENT_DRAFT_CHANGED",
            "The Draft payload changed after this advancement plan was saved. Create a new plan to review those edits.",
        ));
    }
    if let Some(quote) = &metadata.applied
        && !quote_matches_metadata(&metadata, quote)
    {
        return Err(error(
            "ADVANCEMENT_PLAN_INVALID",
            "The applied batch marker does not match its saved requests and source.",
        ));
    }
    Ok(metadata)
}

/// Bind queued requests to one unchanged live Vampire and one linked Draft.
pub fn make(source: &Record, draft: &Record, quote: &Quote) -> Result<Metadata> {
    require_live_vampire(source)?;
    let build = draft
        .build
        .as_ref()
        .filter(|build| build.kind == BuildKind::Draft)
        .ok_or_else(|| {
            error(
                "ADVANCEMENT_DRAFT_SCOPE",
                "Only a Draft copy can hold a saved advancement plan.",
            )
        })?;
    if build.source_id != source.id || draft.kind != source.kind {
        return Err(error(
            "ADVANCEMENT_SOURCE_LINK",
            "The Draft is not linked to this live character.",
        ));
    }
    if draft.private_fields.contains_key(FIELD) {
        return Err(error(
            "ADVANCEMENT_PLAN_EXISTS",
            "This Draft already has an advancement plan.",
        ));
    }
    if draft.public_fields.contains_key(FIELD) {
        return Err(error(
            "ADVANCEMENT_PLAN_PRIVACY",
            "Advancement plan metadata must remain private.",
        ));
    }
    let source_hash = purchases::record_hash(source)?;
    if quote.record_id != source.id || quote.record_hash != source_hash {
        return Err(error(
            "ADVANCEMENT_SOURCE_CHANGED",
            "The quote does not match the current source character.",
        ));
    }
    if quote.steps.is_empty() || quote.steps.len() > MAX_REQUESTS {
        return Err(error(
            "ADVANCEMENT_PLAN_LIMIT",
            "A saved advancement plan must contain one to thirty-two purchases.",
        ));
    }
    let requests: Vec<_> = quote.steps.iter().map(|step| step.request()).collect();
    if purchase_batch::quote(&quote.library_id, quote.revision, source, &requests)? != *quote {
        return Err(error(
            "ADVANCEMENT_QUOTE_INVALID",
            "The advancement quote no longer matches the selected source and requests.",
        ));
    }
    let mut metadata = Metadata {
        version: 2,
        library_id: quote.library_id.clone(),
        source_id: source.id.clone(),
        source_hash,
        draft_hash: payload_hash(draft)?,
        requests,
        intent_hash: String::new(),
        applied: None,
    };
    metadata.intent_hash = intent_hash(&metadata)?;
    // Reserve room for the eventual applied quote before saving immutable intent.
    let mut completed = metadata.clone();
    completed.applied = Some(quote.clone());
    if serde_json::to_vec(&completed)?.len() > MAX_METADATA_BYTES {
        return Err(error(
            "ADVANCEMENT_PLAN_LIMIT",
            "This saved plan would exceed the metadata limit when applied. Use fewer purchases or shorter evidence references.",
        ));
    }
    Ok(metadata)
}

/// Reopen a saved plan. Changed source or Draft content is reported without rebasing.
pub fn review(library_id: &str, revision: i64, source: &Record, draft: &Record) -> Result<Review> {
    let metadata = metadata(draft)?;
    require_live_vampire(source)?;
    if metadata.source_id != source.id {
        return Err(error(
            "DRAFT_SOURCE_CHANGED",
            "The Draft source link no longer matches the selected live character.",
        ));
    }
    if metadata.library_id != library_id {
        return Err(error(
            "DRAFT_LIBRARY_CHANGED",
            "This advancement plan belongs to a different library.",
        ));
    }
    if let Some(applied) = &metadata.applied {
        if applied.library_id != library_id || !quote_matches_metadata(&metadata, applied) {
            return Err(error(
                "ADVANCEMENT_PLAN_INVALID",
                "The applied batch marker belongs to a different library or source.",
            ));
        }
        return Ok(Review {
            draft_id: draft.id.clone(),
            draft_name: draft.name.clone(),
            source_name: source.name.clone(),
            draft_hash: metadata.draft_hash,
            source_hash: metadata.source_hash,
            quote: applied.clone(),
            applied: true,
        });
    }

    if purchases::record_hash(source)? != metadata.source_hash {
        return Err(error(
            "DRAFT_SOURCE_CHANGED",
            "The live source changed after this advancement plan was saved. Review a new plan instead of rebasing silently.",
        ));
    }
    let quote = purchase_batch::quote(library_id, revision, source, &metadata.requests)?;
    Ok(Review {
        draft_id: draft.id.clone(),
        draft_name: draft.name.clone(),
        source_name: source.name.clone(),
        draft_hash: metadata.draft_hash,
        source_hash: metadata.source_hash,
        quote,
        applied: false,
    })
}

fn require_live_vampire(record: &Record) -> Result<()> {
    if record.kind != "Vampire" || record.build.is_some() {
        return Err(error(
            "ADVANCEMENT_SOURCE_SCOPE",
            "An advancement plan requires a live Vampire source character.",
        ));
    }
    Ok(())
}

fn quote_matches_metadata(metadata: &Metadata, quote: &Quote) -> bool {
    quote.library_id == metadata.library_id
        && quote.record_id == metadata.source_id
        && quote.record_hash == metadata.source_hash
        && quote.steps.len() == metadata.requests.len()
        && quote
            .steps
            .iter()
            .zip(&metadata.requests)
            .all(|(step, request)| step.request() == *request)
}

fn intent_hash(metadata: &Metadata) -> Result<String> {
    let canonical = (
        metadata.version,
        metadata.library_id.as_str(),
        metadata.source_id.as_str(),
        metadata.source_hash.as_str(),
        metadata.draft_hash.as_str(),
        metadata.requests.as_slice(),
    );
    let digest = Sha256::digest(serde_json::to_vec(&canonical)?);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
