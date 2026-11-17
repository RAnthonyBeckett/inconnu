//! Private, local review declarations bound to an exact character snapshot.
use crate::{
    model::{Record, Result, error},
    policy_templates::PolicyTemplate,
    pricing::ClanRelation,
    purchases::{PurchaseRequest, record_hash},
    xp,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FIELD: &str = "_AppPurchaseReviews";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewBinding {
    pub id: String,
    pub library_id: String,
    pub record_id: String,
    pub record_hash: String,
    pub power_id: String,
    pub policy: PolicyTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_snapshot_hash: Option<String>,
    pub relation: ClanRelation,
    pub affiliation_name: String,
    pub affiliation_evidence: String,
    pub eligibility_evidence: String,
    pub actor: String,
    pub reviewed_day: String,
}

fn required(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= 16000
}

pub fn state_hash(record: &Record) -> Result<String> {
    let mut snapshot = record.clone();
    snapshot.private_fields.remove(FIELD);
    record_hash(&snapshot)
}

pub fn history(record: &Record) -> Result<Vec<ReviewBinding>> {
    if record.public_fields.contains_key(FIELD) {
        return Err(error(
            "REVIEW_PRIVACY",
            "Local purchase reviews must remain private.",
        ));
    }
    let Some(value) = record.private_fields.get(FIELD) else {
        return Ok(Vec::new());
    };
    let bindings: Vec<ReviewBinding> = serde_json::from_value(value.clone()).map_err(|_| {
        error(
            "REVIEW_INVALID",
            "Stored local review history needs inspection; no changes were made.",
        )
    })?;
    let mut ids = std::collections::HashSet::new();
    if bindings.len() > 1024
        || bindings.iter().any(|b| {
            b.record_id != record.id
                || !ids.insert(&b.id)
                || (b.policy.profile == crate::pricing::PricingProfile::ConfiguredCreature)
                    != b.configured_snapshot_hash.is_some()
                || b.configured_snapshot_hash.as_ref().is_some_and(|hash| {
                    hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
        })
    {
        return Err(error(
            "REVIEW_INVALID",
            "Stored review identities do not match this character.",
        ));
    }
    Ok(bindings)
}

fn configured_snapshot_hash(
    kind: &str,
    power_id: &str,
    relation: ClanRelation,
    powers: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<String> {
    let rules = profiles
        .profile(kind)
        .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
    let definition = powers
        .lookup(power_id)
        .ok_or_else(|| error("UNKNOWN_ANIMALISM_POWER", "Unknown reviewed power."))?;
    crate::purchase_preview::preview_from_definition(
        kind,
        definition,
        Some(crate::pricing::PricingProfile::ConfiguredCreature),
        Some(relation),
        rules,
    )?;
    let snapshot = serde_json::to_vec(&(rules, definition))?;
    Ok(Sha256::digest(snapshot)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn verify_configured_snapshot(
    record: &Record,
    review: &ReviewBinding,
    powers: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<()> {
    let Some(expected) = review.configured_snapshot_hash.as_deref() else {
        return Ok(());
    };
    let current = configured_snapshot_hash(
        &record.kind,
        &review.power_id,
        review.relation,
        powers,
        profiles,
    )
    .map_err(|_| {
        error(
            "REVIEW_CONFIG_STALE",
            "The configured creature profile or power definition changed; record a new review.",
        )
    })?;
    if expected != current {
        return Err(error(
            "REVIEW_CONFIG_STALE",
            "The configured creature profile or power definition changed; record a new review.",
        ));
    }
    Ok(())
}

pub fn make(
    library_id: &str,
    record: &Record,
    input: &PurchaseRequest,
    name: &str,
) -> Result<ReviewBinding> {
    make_from_catalogues(
        library_id,
        record,
        input,
        name,
        crate::power_catalogue::builtin(),
        crate::creature_profiles::builtin(),
    )
}

/// Make a source-bound review using explicitly supplied catalogue/profile snapshots.
/// The injection seam is read-only and exists for independently validated configurations.
pub fn make_from_catalogues(
    library_id: &str,
    record: &Record,
    input: &PurchaseRequest,
    name: &str,
    powers: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<ReviewBinding> {
    if !crate::sheet::is_character_kind(&record.kind)
        || record.build.is_some()
        || (record.kind != "Vampire"
            && input.profile == Some(crate::pricing::PricingProfile::MetLawsOfTheNightRevised))
    {
        return Err(error(
            "REVIEW_SCOPE",
            if record.kind == "Vampire" {
                "Review a live Vampire character in the local Storyteller area."
            } else {
                "Review a live character in the local Storyteller area."
            },
        ));
    }
    let powers = crate::power_catalogue::Catalogue::from_json(&serde_json::to_string(powers)?)?;
    let profiles =
        crate::creature_profiles::Catalogue::from_json(&serde_json::to_string(profiles)?)?;
    let rules = profiles
        .profile(&record.kind)
        .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
    let policy = if record.kind == "Vampire"
        && input.profile == Some(crate::pricing::PricingProfile::MetLawsOfTheNightRevised)
    {
        PolicyTemplate::from_request(name, input)?
    } else {
        PolicyTemplate::from_request_for_kind(name, &record.kind, input, rules)?
    };
    let definition = powers.lookup(&input.power_id).ok_or_else(|| {
        error(
            "UNKNOWN_ANIMALISM_POWER",
            "The source-reviewed catalogue entry is unavailable.",
        )
    })?;
    crate::purchase_preview::preview_from_definition(
        &record.kind,
        definition,
        input.profile,
        input.relation,
        rules,
    )?;
    let configured_snapshot_hash =
        if input.profile == Some(crate::pricing::PricingProfile::ConfiguredCreature) {
            Some(configured_snapshot_hash(
                &record.kind,
                &input.power_id,
                input.relation.ok_or_else(|| {
                    error(
                        "REVIEW_AFFILIATION",
                        "Resolve affiliation before saving review.",
                    )
                })?,
                &powers,
                &profiles,
            )?)
        } else {
            None
        };
    if !xp::day_valid(&input.day)
        || [
            &input.affiliation_name,
            &input.affiliation_evidence,
            &input.eligibility_evidence,
            &input.actor,
        ]
        .iter()
        .any(|s| !required(s))
    {
        return Err(error(
            "REVIEW_EVIDENCE",
            "Record the review date, recorder, affiliation and eligibility evidence first.",
        ));
    }
    history(record)?;
    Ok(ReviewBinding {
        id: uuid::Uuid::new_v4().to_string(),
        library_id: library_id.into(),
        record_id: record.id.clone(),
        record_hash: state_hash(record)?,
        power_id: input.power_id.clone(),
        policy,
        configured_snapshot_hash,
        relation: input.relation.ok_or_else(|| {
            error(
                "REVIEW_AFFILIATION",
                "Resolve affiliation before saving review.",
            )
        })?,
        affiliation_name: input.affiliation_name.clone(),
        affiliation_evidence: input.affiliation_evidence.clone(),
        eligibility_evidence: input.eligibility_evidence.clone(),
        actor: input.actor.clone(),
        reviewed_day: input.day.clone(),
    })
}

pub fn is_current(library_id: &str, record: &Record, review: &ReviewBinding) -> Result<bool> {
    Ok(crate::sheet::is_character_kind(&record.kind)
        && record.build.is_none()
        && review.library_id == library_id
        && review.record_id == record.id
        && review.record_hash == state_hash(record)?)
}

pub fn append(library_id: &str, record: &Record, review: &ReviewBinding) -> Result<Record> {
    if !is_current(library_id, record, review)? {
        return Err(error(
            "REVIEW_STALE",
            "The character changed before the review was saved.",
        ));
    }
    let mut bindings = history(record)?;
    if bindings.len() >= 1024 || bindings.iter().any(|b| b.id == review.id) {
        return Err(error(
            "REVIEW_LIMIT",
            "Review history is full or the review ID is already present.",
        ));
    }
    bindings.push(review.clone());
    let mut updated = record.clone();
    updated
        .private_fields
        .insert(FIELD.into(), serde_json::to_value(bindings)?);
    Ok(updated)
}

pub fn reuse(
    library_id: &str,
    record: &Record,
    review_id: &str,
    input: &mut PurchaseRequest,
) -> Result<()> {
    reuse_from_catalogues(
        library_id,
        record,
        review_id,
        input,
        crate::power_catalogue::builtin(),
        crate::creature_profiles::builtin(),
    )
}

pub fn reuse_from_catalogues(
    library_id: &str,
    record: &Record,
    review_id: &str,
    input: &mut PurchaseRequest,
    powers: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<()> {
    let review = history(record)?
        .into_iter()
        .find(|b| b.id == review_id)
        .ok_or_else(|| {
            error(
                "REVIEW_NOT_FOUND",
                "Choose a saved review for this character.",
            )
        })?;
    if !is_current(library_id, record, &review)? {
        return Err(error(
            "REVIEW_STALE",
            "The character or XP history changed; record a new local review.",
        ));
    }
    if review.configured_snapshot_hash.is_some() {
        let powers = crate::power_catalogue::Catalogue::from_json(&serde_json::to_string(powers)?)?;
        let profiles =
            crate::creature_profiles::Catalogue::from_json(&serde_json::to_string(profiles)?)?;
        verify_configured_snapshot(record, &review, &powers, &profiles)?;
    }
    let mut updated = input.clone();
    review.policy.apply_for_kind(&mut updated, &record.kind)?;
    updated.power_id = review.power_id;
    updated.relation = Some(review.relation);
    updated.affiliation_name = review.affiliation_name;
    updated.affiliation_evidence = review.affiliation_evidence;
    updated.eligibility_evidence = review.eligibility_evidence;
    updated.review_id = Some(review.id);
    *input = updated;
    Ok(())
}

pub fn verify(library_id: &str, record: &Record, input: &PurchaseRequest) -> Result<()> {
    verify_from_catalogues(
        library_id,
        record,
        input,
        crate::power_catalogue::builtin(),
        crate::creature_profiles::builtin(),
    )
}

pub fn verify_from_catalogues(
    library_id: &str,
    record: &Record,
    input: &PurchaseRequest,
    powers: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<()> {
    let Some(id) = &input.review_id else {
        return Ok(());
    };
    let review = history(record)?
        .into_iter()
        .find(|b| &b.id == id)
        .ok_or_else(|| error("REVIEW_NOT_FOUND", "The linked local review is missing."))?;
    if !is_current(library_id, record, &review)? {
        return Err(error(
            "REVIEW_STALE",
            "The character or XP history changed; record a new local review.",
        ));
    }
    if review.configured_snapshot_hash.is_some() {
        let powers = crate::power_catalogue::Catalogue::from_json(&serde_json::to_string(powers)?)?;
        let profiles =
            crate::creature_profiles::Catalogue::from_json(&serde_json::to_string(profiles)?)?;
        verify_configured_snapshot(record, &review, &powers, &profiles)?;
    }
    verify_declarations(library_id, record, input)
}

/// Historical receipts retain their original review declarations even after XP/sheet changes.
pub fn verify_declarations(
    library_id: &str,
    record: &Record,
    input: &PurchaseRequest,
) -> Result<()> {
    let Some(id) = &input.review_id else {
        return Ok(());
    };
    let review = history(record)?
        .into_iter()
        .find(|b| &b.id == id)
        .ok_or_else(|| error("REVIEW_NOT_FOUND", "The linked local review is missing."))?;
    if review.library_id != library_id
        || review.record_id != record.id
        || input.day < review.reviewed_day
    {
        return Err(error(
            "REVIEW_SCOPE",
            "The linked local review belongs to a different library, character or later date.",
        ));
    }
    let mut expected = input.clone();
    review.policy.apply_for_kind(&mut expected, &record.kind)?;
    expected.power_id = review.power_id;
    expected.relation = Some(review.relation);
    expected.affiliation_name = review.affiliation_name;
    expected.affiliation_evidence = review.affiliation_evidence;
    expected.eligibility_evidence = review.eligibility_evidence;
    expected.review_id = Some(review.id);
    expected.approval_reference = input.approval_reference.clone();
    expected.teacher_evidence = input.teacher_evidence.clone();
    if expected != *input {
        return Err(error(
            "REVIEW_MISMATCH",
            "Purchase declarations differ from the linked review. Reuse it again or start a fresh manual form.",
        ));
    }
    Ok(())
}
