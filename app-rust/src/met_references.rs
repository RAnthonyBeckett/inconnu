//! Reviewed MET reminder mapping for an existing original Vampire trait; never a grant.
use crate::{model::*, published_reference, purchases};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub const FIELD: &str = "_AppMetReminderReviewsV1";
pub const TRAIT_FIELD: &str = "AppMetReminderReference";
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
    pub power_id: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub review_reference: String,
    pub confirmed: bool,
    pub confirm_alias: bool,
    pub confirm_legacy_value: bool,
    pub clear: bool,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            trait_index: 0,
            power_id: String::new(),
            day: String::new(),
            actor: String::new(),
            reason: String::new(),
            review_reference: String::new(),
            confirmed: false,
            confirm_alias: false,
            confirm_legacy_value: false,
            clear: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_list: Option<String>,
    pub library_id: String,
    pub record_id: String,
    pub revision: i64,
    pub record_hash: String,
    pub request: Request,
    pub updated_trait: Value,
    pub power_name: String,
    pub citation: Option<crate::pricing::SourceCitation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub quote: Quote,
    pub committed_revision: i64,
}
pub fn history(record: &Record) -> Result<Vec<Receipt>> {
    if record.public_fields.contains_key(FIELD) {
        return Err(error(
            "MET_REFERENCE_PRIVACY",
            "MET mapping review history must remain private.",
        ));
    }
    let Some(value) = record.private_fields.get(FIELD) else {
        return Ok(vec![]);
    };
    if serde_json::to_vec(value)?.len() > 8 * 1024 * 1024 {
        return Err(error(
            "MET_REFERENCE_LIMIT",
            "MET review history exceeds its supported limit.",
        ));
    }
    let entries: Vec<Receipt> = serde_json::from_value(value.clone())?;
    let mut ids = std::collections::HashSet::new();
    if entries.len() > 1024
        || entries.iter().any(|e| {
            e.quote.record_id != record.id
                || !ids.insert(&e.quote.request.id)
                || e.quote.revision.checked_add(1) != Some(e.committed_revision)
        })
    {
        return Err(error(
            "MET_REFERENCE_HISTORY",
            "MET review identities or revisions are inconsistent.",
        ));
    }
    Ok(entries)
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
    if !crate::sheet::is_character_kind(&record.kind) || record.build.is_some() {
        return Err(error(
            "MET_REFERENCE_SCOPE",
            "Review MET mappings on an original Vampire. Copies inherit applicable source proof.",
        ));
    }
    history(record)?;
    if uuid::Uuid::parse_str(&request.id)
        .ok()
        .is_none_or(|u| u.to_string() != request.id)
        || !crate::xp::day_valid(&request.day)
        || !request.confirmed
        || [&request.actor, &request.reason, &request.review_reference]
            .iter()
            .any(|v| v.trim().is_empty() || v.len() > 16000)
    {
        return Err(error(
            "MET_REFERENCE_REVIEW",
            "Record a new ID, valid date, reviewer, reason and source/eligibility reference; explicitly confirm the mapping.",
        ));
    }
    let list = if requested_list.is_none() {
        purchases::discipline_list(record)?
    } else {
        purchases::power_list(record, &field)?
    };
    let row = list["Traits"]
        .as_array()
        .and_then(|r| r.get(request.trait_index))
        .ok_or_else(|| {
            error(
                "MET_REFERENCE_TRAIT",
                "Choose an existing editable Discipline row.",
            )
        })?;
    if row["Name"]
        .as_str()
        .is_none_or(|name| name.trim().is_empty())
    {
        return Err(error(
            "MET_REFERENCE_TRAIT",
            "Choose a named existing Discipline row before reviewing its mapping.",
        ));
    }
    let mut updated = row.clone();
    let mut citation = None;
    let mut power_name = String::new();
    if request.clear {
        if !request.power_id.is_empty()
            || (row.get(TRAIT_FIELD).is_none()
                && row.get(crate::creation_sources::MET_FIELD).is_none())
        {
            return Err(error(
                "MET_REFERENCE_CLEAR",
                "Select an existing reminder mapping and clear its power selection before removal.",
            ));
        }
        updated.as_object_mut().unwrap().remove(TRAIT_FIELD);
        updated
            .as_object_mut()
            .unwrap()
            .remove(crate::creation_sources::MET_FIELD);
    } else {
        let power = crate::power_catalogue::lookup(&request.power_id).ok_or_else(|| {
            error(
                "MET_REFERENCE_POWER",
                "Choose a supported MET power, separately from any published level.",
            )
        })?;
        let definition = crate::power_catalogue::builtin()
            .lookup(&request.power_id)
            .ok_or_else(|| error("MET_REFERENCE_POWER", "Unknown power."))?;
        if power.template_scope != record.kind || definition.target_list != field {
            return Err(error(
                "MET_REFERENCE_SCOPE",
                "This power/list belongs to a different creature profile.",
            ));
        }
        if row["Name"]
            .as_str()
            .is_none_or(|name| !name.trim().eq_ignore_ascii_case(&power.name))
            && !request.confirm_alias
        {
            return Err(error(
                "MET_REFERENCE_ALIAS",
                "Confirm that this recorded name is an alias for the selected MET power.",
            ));
        }
        if row["Value"]
            .as_str()
            .is_none_or(|v| !v.trim().eq_ignore_ascii_case(power.tier.sheet_value()))
            && !request.confirm_legacy_value
        {
            return Err(error(
                "MET_REFERENCE_VALUE",
                "Explicitly review the legacy value and selected MET tier; no numerical-level conversion is inferred.",
            ));
        }
        if let Some(existing) = row["AppSourcePowerId"]
            .as_str()
            .and_then(crate::power_catalogue::lookup)
            && row["Name"].as_str() == Some(existing.name.as_str())
            && row["Value"].as_str() == Some(existing.tier.sheet_value())
            && existing.stable_id != power.stable_id
        {
            return Err(error(
                "MET_REFERENCE_PAID_SOURCE",
                "The recorded canonical purchase identifies another power. Correct the purchase rather than overriding it with a reminder mapping.",
            ));
        }
        let source = power.source.ok_or_else(|| {
            error(
                "MET_REFERENCE_SOURCE",
                "The selected MET source is unresolved.",
            )
        })?;
        if definition.legacy_power.is_some() {
            citation = Some(crate::pricing::SourceCitation {
                book: "Laws of the Night".into(),
                edition: "Revised".into(),
                system: "Mind's Eye Theatre".into(),
                printed_page: source.printed_page,
                pdf_page_1based: source.pdf_page_1based,
                original_pdf_sha256: crate::simple_mechanics::SOURCE_SHA.into(),
            });
        } else {
            citation = Some(definition.source_citation()?);
        }
        power_name = power.name;
        updated[TRAIT_FIELD] = json!({"review_id":request.id,"power_id":request.power_id,"trait_signature":published_reference::trait_signature(row)?,"source_sha256":citation.as_ref().unwrap().original_pdf_sha256});
    }
    Ok(Quote {
        target_list: (field
            != crate::creature_profiles::profile(&record.kind)
                .unwrap()
                .primary_power_list)
            .then_some(field),
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        request: request.clone(),
        updated_trait: updated,
        power_name,
        citation,
    })
}
pub fn verified(record: &Record, row: &Value) -> Option<String> {
    if !crate::sheet::is_character_kind(&record.kind) {
        return None;
    }
    let binding = row.get(TRAIT_FIELD)?;
    let id = binding["power_id"].as_str()?;
    let definition = crate::power_catalogue::builtin().lookup(id)?;
    if definition.template_scope != record.kind {
        return None;
    }
    if binding["source_sha256"].as_str() != Some(definition.original_pdf_sha256.as_str())
        || binding["trait_signature"].as_str()
            != Some(published_reference::trait_signature(row).ok()?.as_str())
    {
        return None;
    }
    let list = purchases::power_list(record, &definition.target_list).ok()?;
    if list["Traits"]
        .as_array()?
        .iter()
        .filter(|r| r.get(TRAIT_FIELD) == row.get(TRAIT_FIELD))
        .count()
        != 1
    {
        return None;
    }
    history(record).ok()?.iter().find(|r| {
        r.quote.request.id == binding["review_id"].as_str().unwrap_or("")
            && target_field(record, r.quote.target_list.as_deref())
                .ok()
                .as_deref()
                == Some(definition.target_list.as_str())
            && !r.quote.request.clear
            && r.quote.updated_trait.get(TRAIT_FIELD) == Some(binding)
            && record
                .build
                .as_ref()
                .is_none_or(|b| r.committed_revision <= b.source_revision)
    })?;
    crate::power_catalogue::lookup(id).map(|p| p.stable_id)
}

/// Resolves historical primary requests and validates explicit secondary targets.
pub(crate) fn target_field(record: &Record, requested: Option<&str>) -> Result<String> {
    let profile = crate::creature_profiles::profile(&record.kind).ok_or_else(|| {
        error(
            "REFERENCE_LIST_SCOPE",
            "Choose a supported creature power list.",
        )
    })?;
    let field = requested
        .filter(|s| !s.is_empty())
        .unwrap_or(&profile.primary_power_list);
    if !profile.owns_power_list(field) {
        return Err(error(
            "REFERENCE_LIST_SCOPE",
            "This power list belongs to a different creature profile.",
        ));
    }
    Ok(field.to_owned())
}
