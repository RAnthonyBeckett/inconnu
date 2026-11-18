//! Staff-recorded source attribution to an existing trait, never a power grant or XP spend.
use crate::{
    model::*,
    published_powers::{self, Evidence, Reference},
    purchases, xp,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
pub const FIELD: &str = "_AppPublishedReferenceReviews";
pub const TRAIT_FIELD: &str = "AppPublishedReference";

#[derive(Clone, Debug)]
pub struct Context {
    pub target_list: String,
    pub revision: i64,
    pub record_id: String,
    pub record_name: String,
    pub traits: Vec<Value>,
    pub history: Vec<Receipt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub trait_index: usize,
    pub source_id: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub clear: bool,
    pub rename_to_source: bool,
    pub confirm_alias: bool,
    pub confirm_index_evidence: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_list: Option<String>,
    pub request: Request,
    pub library_id: String,
    pub record_id: String,
    pub revision: i64,
    pub record_hash: String,
    pub old_name: String,
    pub old_value: String,
    pub reference: Option<Reference>,
    pub updated_trait: Value,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub quote: Quote,
    pub committed_revision: i64,
}

pub fn history(record: &Record) -> Result<Vec<Receipt>> {
    if record.public_fields.contains_key(FIELD) {
        return Err(error(
            "PUBLISHED_REFERENCE_PRIVACY",
            "Source review records must remain private.",
        ));
    }
    let Some(value) = record.private_fields.get(FIELD) else {
        return Ok(vec![]);
    };
    if serde_json::to_vec(value)?.len() > 8 * 1024 * 1024 {
        return Err(error(
            "PUBLISHED_REFERENCE_LIMIT",
            "Source review history exceeds its supported limit.",
        ));
    }
    let history: Vec<Receipt> = serde_json::from_value(value.clone()).map_err(|_| {
        error(
            "PUBLISHED_REFERENCE_HISTORY",
            "Source review history needs inspection.",
        )
    })?;
    let mut ids = std::collections::HashSet::new();
    if history.len() > 1024
        || history.iter().any(|r| {
            r.quote.record_id != record.id
                || !ids.insert(&r.quote.request.id)
                || r.quote.revision.checked_add(1) != Some(r.committed_revision)
        })
    {
        return Err(error(
            "PUBLISHED_REFERENCE_HISTORY",
            "Source review identities or revisions are inconsistent.",
        ));
    }
    Ok(history)
}
pub fn trait_signature(row: &Value) -> Result<String> {
    Ok(
        Sha256::digest(serde_json::to_vec(&(row.get("Name"), row.get("Value")))?)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}
pub fn quote(library_id: &str, revision: i64, record: &Record, request: &Request) -> Result<Quote> {
    quote_for_list(library_id, revision, record, request, None)
}
pub fn quote_for_list(
    library_id: &str,
    revision: i64,
    record: &Record,
    request: &Request,
    requested_list: Option<&str>,
) -> Result<Quote> {
    let field = crate::met_references::target_field(record, requested_list)?;
    if !crate::sheet::is_character_kind(&record.kind) {
        return Err(error(
            "PUBLISHED_REFERENCE_SCOPE",
            "Choose a Vampire trait to review its published source.",
        ));
    }
    if record
        .private_fields
        .contains_key(crate::advancement_draft::FIELD)
    {
        return Err(error(
            "PUBLISHED_REFERENCE_LOCKED_DRAFT",
            "Saved advancement intent is immutable. Attribute sources on the live character or a separate ordinary copy.",
        ));
    }
    if uuid::Uuid::parse_str(&request.id)
        .ok()
        .is_none_or(|id| id.to_string() != request.id)
        || !xp::day_valid(&request.day)
        || [&request.actor, &request.reason]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 16000)
    {
        return Err(error(
            "PUBLISHED_REFERENCE_EVIDENCE",
            "Record a new review ID, valid date, recorder and reason.",
        ));
    }
    if history(record)?
        .iter()
        .any(|receipt| receipt.quote.request.id == request.id)
    {
        return Err(error(
            "PUBLISHED_REFERENCE_ID_CONFLICT",
            "This review ID already identifies a saved source attribution.",
        ));
    }
    let list = purchases::power_list(record, &field)?;
    let row = list["Traits"]
        .as_array()
        .and_then(|rows| rows.get(request.trait_index))
        .ok_or_else(|| {
            error(
                "PUBLISHED_REFERENCE_TRAIT",
                "Choose an existing editable Discipline trait.",
            )
        })?;
    let name = row["Name"].as_str().ok_or_else(|| {
        error(
            "PUBLISHED_REFERENCE_TRAIT",
            "The selected trait has no readable name.",
        )
    })?;
    let mut updated_trait = row.clone();
    let reference = if request.clear {
        if row.get(TRAIT_FIELD).is_none() {
            return Err(error(
                "PUBLISHED_REFERENCE_MISSING",
                "The selected trait has no published reference to remove.",
            ));
        }
        if !request.source_id.is_empty() || request.rename_to_source {
            return Err(error(
                "PUBLISHED_REFERENCE_ACTION",
                "Clearing a reference cannot also rename or choose another source.",
            ));
        }
        updated_trait
            .as_object_mut()
            .ok_or_else(|| {
                error(
                    "PUBLISHED_REFERENCE_TRAIT",
                    "The selected trait is unreadable.",
                )
            })?
            .remove(TRAIT_FIELD);
        None
    } else {
        let reference = published_powers::catalogue()?.reference(&request.source_id)?;
        if !published_powers::catalogue()?
            .book(&reference.book_id)
            .is_some_and(|book| book.applies_to(&record.kind))
        {
            return Err(error(
                "PUBLISHED_REFERENCE_SCOPE",
                "The selected book is not scoped to this creature type.",
            ));
        }
        if reference.power.evidence == Evidence::IndexOnly && !request.confirm_index_evidence {
            return Err(error(
                "PUBLISHED_REFERENCE_SOURCE_REVIEW",
                "This entry is indexed evidence. Confirm that you checked its cited book before attributing it.",
            ));
        }
        if name != reference.power.name && !request.rename_to_source && !request.confirm_alias {
            return Err(error(
                "PUBLISHED_REFERENCE_NAME",
                "The names differ. Explicitly review the alias or choose to use the source name.",
            ));
        }
        if request.rename_to_source {
            updated_trait["Name"] = json!(reference.power.name);
        }
        let signature = trait_signature(&updated_trait)?;
        updated_trait[TRAIT_FIELD] = json!({"id":request.id,"source_id":reference.power.id,"facts_hash":reference.facts_hash,"trait_signature":signature});
        Some(reference)
    };
    if serde_json::to_vec(&updated_trait)?.len() > 1024 * 1024 {
        return Err(error(
            "PUBLISHED_REFERENCE_LIMIT",
            "Selected trait exceeds the review size limit.",
        ));
    }
    Ok(Quote {
        target_list: (field
            != crate::creature_profiles::profile(&record.kind)
                .unwrap()
                .primary_power_list)
            .then_some(field),
        request: request.clone(),
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        old_name: name.into(),
        old_value: row["Value"].as_str().unwrap_or("").into(),
        reference,
        updated_trait,
    })
}

/// Metadata alone carries only public source facts; staff declarations stay in FIELD.
pub fn verified_reference(row: &Value) -> Option<Reference> {
    let binding = row.get(TRAIT_FIELD)?;
    let reference = published_powers::catalogue()
        .ok()?
        .reference(binding["source_id"].as_str()?)
        .ok()?;
    (binding["facts_hash"].as_str() == Some(reference.facts_hash.as_str())
        && binding["trait_signature"].as_str() == Some(trait_signature(row).ok()?.as_str()))
    .then_some(reference)
}
