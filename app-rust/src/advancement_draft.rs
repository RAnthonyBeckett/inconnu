//! Persistent immutable intent for a reviewed purchase batch Draft.

use crate::{
    model::{BuildKind, Record, Result, error},
    purchase_batch::{self, Quote},
    purchases::{self, PurchaseRequest},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FIELD: &str = "_AppAdvancementPlanV1";
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
    if metadata.version != 1
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
            .any(|request| request.review_id.is_some())
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
    let requests: Vec<_> = quote
        .steps
        .iter()
        .map(|step| step.request.clone())
        .collect();
    if purchase_batch::quote(&quote.library_id, quote.revision, source, &requests)? != *quote {
        return Err(error(
            "ADVANCEMENT_QUOTE_INVALID",
            "The advancement quote no longer matches the selected source and requests.",
        ));
    }
    let mut metadata = Metadata {
        version: 1,
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
            .all(|(step, request)| step.request == *request)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{BuildInfo, BuildKind},
        pricing::{ClanRelation, PricingProfile},
        purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
        purchases::{PurchaseRequest, TeacherRule},
        xp::{Baseline, Ledger},
    };
    use serde_json::{Map, json};

    fn record(unspent: &str, id: &str, name: &str) -> Record {
        Record {
            id: id.into(),
            kind: "Vampire".into(),
            name: name.into(),
            player_id: None,
            legacy_id: None,
            public_fields: Map::new(),
            private_fields: Map::new(),
            original: json!({"fixture":"advancement draft"}),
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

    fn source() -> Record {
        record(
            "20",
            "00000000-0000-4000-8000-000000000001",
            "Synthetic source",
        )
    }

    fn draft(source: &Record) -> Record {
        let mut draft = source.clone();
        draft.id = "00000000-0000-4000-8000-000000000002".into();
        draft.name = "Synthetic draft".into();
        draft.build = Some(BuildInfo {
            source_id: source.id.clone(),
            source_revision: 5,
            kind: BuildKind::Draft,
        });
        draft
    }

    fn request() -> PurchaseRequest {
        PurchaseRequest {
            review_id: None,
            id: "00000000-0000-4000-8000-000000000003".into(),
            power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
            profile: Some(PricingProfile::MetLawsOfTheNightRevised),
            relation: Some(ClanRelation::InClan),
            affiliation_name: "Synthetic clan".into(),
            day: "2026-01-02".into(),
            available_from: "2026-01-03".into(),
            next_session_reference: "Synthetic next session".into(),
            actor: "Synthetic ST".into(),
            reason: "Synthetic advancement".into(),
            policy_version: "synthetic-v1".into(),
            policy_reference: "Synthetic local policy".into(),
            affiliation_evidence: "Synthetic affiliation evidence".into(),
            approval_reference: "Synthetic approval evidence".into(),
            eligibility_evidence: "Synthetic eligibility evidence".into(),
            teacher_rule: Some(TeacherRule::Required),
            teacher_evidence: "Synthetic teacher evidence".into(),
            standard_cost_confirmed: true,
            owbn_evidence: Default::default(),
        }
    }

    fn saved_draft(source: &Record, quote: &Quote) -> Record {
        let mut draft = draft(source);
        let metadata = make(source, &draft, quote).unwrap();
        draft
            .private_fields
            .insert(FIELD.into(), serde_json::to_value(metadata).unwrap());
        draft
    }

    #[test]
    fn saving_reserves_room_for_applied_marker_near_request_limit() {
        let mut source = source();
        source.xp_ledger.as_mut().unwrap().baseline.earned = "200".into();
        source.xp_ledger.as_mut().unwrap().baseline.unspent = "200".into();
        let mut requests: Vec<_> = crate::purchase_preview::AnimalismPower::ALL
            .iter()
            .take(16)
            .map(|power| {
                let mut r = request();
                r.id = uuid::Uuid::new_v4().to_string();
                r.power_id = power.stable_id().into();
                for text in [
                    &mut r.actor,
                    &mut r.reason,
                    &mut r.policy_reference,
                    &mut r.affiliation_evidence,
                    &mut r.teacher_evidence,
                ] {
                    *text = "x".repeat(13000);
                }
                r
            })
            .collect();
        let length = serde_json::to_vec(&requests).unwrap().len();
        let excess = length.saturating_sub(1024 * 1024 - 128);
        assert!(excess < 13000);
        requests[0].reason.truncate(13000 - excess);
        assert!(serde_json::to_vec(&requests).unwrap().len() < 1024 * 1024);
        let quote = purchase_batch::quote("synthetic-library", 5, &source, &requests).unwrap();
        assert_eq!(
            make(&source, &draft(&source), &quote).unwrap_err().code,
            "ADVANCEMENT_PLAN_LIMIT"
        );
    }
    #[test]
    fn saved_plan_reopens_with_exact_source_hash_and_fresh_quote_revision() {
        let source = source();
        let original =
            purchase_batch::quote("synthetic-library", 5, &source, &[request()]).unwrap();
        let draft = saved_draft(&source, &original);

        let review = review("synthetic-library", 6, &source, &draft).unwrap();
        assert_eq!(review.draft_id, draft.id);
        assert_eq!(review.draft_name, draft.name);
        assert_eq!(review.source_name, source.name);
        assert_eq!(review.source_hash, purchases::record_hash(&source).unwrap());
        assert_eq!(review.draft_hash, payload_hash(&draft).unwrap());
        assert_eq!(review.quote.revision, 6);
        assert_eq!(review.quote.steps[0].request, request());
        assert!(!review.applied);
    }

    #[test]
    fn manually_changed_draft_or_source_is_blocked_without_rebase() {
        let source = source();
        let quote = purchase_batch::quote("synthetic-library", 5, &source, &[request()]).unwrap();
        let draft = saved_draft(&source, &quote);

        let mut edited_draft = draft.clone();
        edited_draft
            .private_fields
            .insert("UnknownDraftField".into(), json!({"changed":true}));
        assert_eq!(
            metadata(&edited_draft).unwrap_err().code,
            "ADVANCEMENT_DRAFT_CHANGED"
        );

        let mut edited_intent = draft.clone();
        let mut saved_intent = edited_intent.private_fields.get(FIELD).unwrap().clone();
        saved_intent["requests"][0]["reason"] = json!("Changed saved intent");
        edited_intent
            .private_fields
            .insert(FIELD.into(), saved_intent);
        assert_eq!(
            metadata(&edited_intent).unwrap_err().code,
            "ADVANCEMENT_PLAN_INVALID"
        );
        assert_eq!(
            review("another-library", 6, &source, &draft)
                .unwrap_err()
                .code,
            "DRAFT_LIBRARY_CHANGED"
        );

        let mut edited_source = source.clone();
        edited_source
            .private_fields
            .insert("UnreviewedChange".into(), json!(true));
        assert_eq!(
            review("synthetic-library", 6, &edited_source, &draft)
                .unwrap_err()
                .code,
            "DRAFT_SOURCE_CHANGED"
        );
    }

    #[test]
    fn test_copy_and_public_metadata_are_rejected() {
        let source = source();
        let quote = purchase_batch::quote("synthetic-library", 5, &source, &[request()]).unwrap();
        let mut test_copy = saved_draft(&source, &quote);
        test_copy.build.as_mut().unwrap().kind = BuildKind::Test;
        assert_eq!(
            metadata(&test_copy).unwrap_err().code,
            "ADVANCEMENT_DRAFT_SCOPE"
        );

        let mut public_plan = draft(&source);
        public_plan.public_fields.insert(FIELD.into(), json!({}));
        assert_eq!(
            payload_hash(&public_plan).unwrap_err().code,
            "ADVANCEMENT_PLAN_PRIVACY"
        );
    }

    #[test]
    fn applied_marker_reopens_after_the_live_source_changes() {
        let source = source();
        let quote = purchase_batch::quote("synthetic-library", 5, &source, &[request()]).unwrap();
        let mut draft = draft(&source);
        let mut saved = make(&source, &draft, &quote).unwrap();
        saved.applied = Some(quote.clone());
        draft
            .private_fields
            .insert(FIELD.into(), serde_json::to_value(saved).unwrap());

        let step = purchases::quote("synthetic-library", 5, &source, &request()).unwrap();
        let applied_source = purchases::apply(&source, &step, 123).unwrap();
        let review = review("synthetic-library", 6, &applied_source, &draft).unwrap();
        assert!(review.applied);
        assert_eq!(review.quote, quote);
    }
}
