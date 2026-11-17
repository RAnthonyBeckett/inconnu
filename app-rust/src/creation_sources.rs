//! Proposed source choices stay in private creation drafts until the reviewed grant commits.
use crate::{model::*, published_reference as source, vampire_creation::Request};
use serde_json::{Value, json};
pub const PROPOSED: &str = "AppCreationProposedSource";
pub const MET_FIELD: &str = "AppCreationMetReference";
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetMapping {
    pub index: usize,
    pub power_id: String,
    pub signature: String,
    pub review_id: String,
}
pub struct Prepared {
    pub sheet: SheetEdit,
    pub quotes: Vec<source::Quote>,
    pub mappings: Vec<MetMapping>,
    pub issues: Vec<String>,
}
pub fn review_signature(row: &Value) -> Result<String> {
    let signature = source::trait_signature(row)?;
    let choice = &row[PROPOSED];
    if choice.get("met_power_id").is_none() {
        return Ok(signature);
    }
    use sha2::{Digest, Sha256};
    Ok(Sha256::digest(serde_json::to_vec(&(
        signature,
        &choice["source_id"],
        &choice["met_power_id"],
    ))?)
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect())
}
pub fn choose_met(row: &mut Value, id: Option<&str>) -> Result<()> {
    if let Some(id) = id {
        let power = crate::power_catalogue::lookup(id)
            .ok_or_else(|| error("CREATION_MET_SOURCE", "Choose a supported MET power."))?;
        if power.tier != crate::pricing::MetTier::Basic || row["Value"].as_str() != Some("basic") {
            return Err(error(
                "CREATION_MET_TIER",
                "Initial MET mappings require an explicitly selected Basic row.",
            ));
        }
    }
    let object = row
        .as_object_mut()
        .ok_or_else(|| error("CREATION_SOURCE_TRAIT", "Choose an editable initial power."))?;
    object.remove(MET_FIELD);
    let choice = object
        .entry(PROPOSED)
        .or_insert(json!({}))
        .as_object_mut()
        .ok_or_else(|| error("CREATION_MET_SOURCE", "Reload the proposed mapping."))?;
    choice.remove("reviewed_signature");
    if let Some(id) = id {
        choice.insert("met_power_id".into(), json!(id));
    } else {
        choice.remove("met_power_id");
    }
    if choice.get("met_power_id").is_none() && choice.get("source_id").is_none() {
        object.remove(PROPOSED);
    }
    Ok(())
}

pub fn prepare(
    library_id: &str,
    revision: i64,
    record: &Record,
    request: &Request,
) -> Result<Prepared> {
    let mut edit = request.sheet.clone();
    let mut quotes = vec![];
    let mut issues = vec![];
    let mut mappings = vec![];
    let Some(list) = edit.lists.get_mut("DisciplineList") else {
        return Ok(Prepared {
            sheet: edit,
            quotes,
            mappings,
            issues,
        });
    };
    let mut proposed = record.clone();
    let mut clean = list.clone();
    if let Some(rows) = clean.get_mut("Traits").and_then(Value::as_array_mut) {
        for row in rows {
            if let Some(object) = row.as_object_mut() {
                object.remove(PROPOSED);
            }
        }
    }
    crate::sheet::write_field(&mut proposed, "DisciplineList", clean);
    let Some(rows) = list.get_mut("Traits").and_then(Value::as_array_mut) else {
        return Ok(Prepared {
            sheet: edit,
            quotes,
            mappings,
            issues,
        });
    };
    for (index, row) in rows.iter_mut().enumerate() {
        let Some(choice) = row.get(PROPOSED).cloned() else {
            continue;
        };
        let id = choice
            .get("source_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let met_id = choice.get("met_power_id").and_then(Value::as_str);
        if (id.is_empty() && met_id.is_none())
            || choice.get("reviewed_signature").and_then(Value::as_str)
                != Some(review_signature(row)?.as_str())
        {
            issues.push(format!("Initial power {} needs its published source and MET Basic eligibility/mapping reviewed after its last name/value change.",index+1));
        } else if !id.is_empty() {
            let binding = source::Request {
                id: uuid::Uuid::new_v5(
                    &uuid::Uuid::NAMESPACE_URL,
                    format!("{library_id}/{}/{index}/{id}", request.id).as_bytes(),
                )
                .to_string(),
                trait_index: index,
                source_id: id.into(),
                day: request.day.clone(),
                actor: request.actor.clone(),
                reason: request.reason.clone(),
                clear: false,
                rename_to_source: false,
                confirm_alias: true,
                confirm_index_evidence: true,
            };
            match source::quote(library_id, revision, &proposed, &binding) {
                Ok(quote) => {
                    *row = quote.updated_trait.clone();
                    quotes.push(quote);
                }
                Err(error) => issues.push(format!(
                    "Initial power {} source review: {}",
                    index + 1,
                    error.message
                )),
            }
        }
        if let Some(met_id) = met_id {
            let valid = crate::power_catalogue::lookup(met_id)
                .is_some_and(|p| p.tier == crate::pricing::MetTier::Basic)
                && row["Value"].as_str() == Some("basic")
                && choice.get("reviewed_signature").and_then(Value::as_str)
                    == Some(
                        review_signature(&request.sheet.lists["DisciplineList"]["Traits"][index])?
                            .as_str(),
                    );
            if valid {
                let mapping = MetMapping {
                    index,
                    power_id: met_id.into(),
                    signature: source::trait_signature(row)?,
                    review_id: request.id.clone(),
                };
                row[MET_FIELD] = serde_json::to_value(&mapping)?;
                mappings.push(mapping);
            } else {
                issues.push(format!(
                    "Initial power {} needs a reviewed supported MET Basic mapping.",
                    index + 1
                ));
            }
        }
        if let Some(object) = row.as_object_mut() {
            object.remove(PROPOSED);
        }
    }
    Ok(Prepared {
        sheet: edit,
        quotes,
        mappings,
        issues,
    })
}

/// An explicit selection copies a published name; it never assigns a MET tier or grants a power.
pub fn choose(row: &mut Value, source_id: &str) -> Result<()> {
    let reference = crate::published_powers::catalogue()?.reference(source_id)?;
    let object = row
        .as_object_mut()
        .ok_or_else(|| error("CREATION_SOURCE_TRAIT", "Choose an editable initial power."))?;
    object.insert("Name".into(), json!(reference.power.name));
    object.remove(source::TRAIT_FIELD);
    object.remove(MET_FIELD);
    object.insert(PROPOSED.into(), json!({"source_id":source_id}));
    Ok(())
}

pub fn verified_met(record: &Record, row: &Value) -> Option<String> {
    if record.kind != "Vampire" {
        return None;
    }
    let mapping: MetMapping = serde_json::from_value(row.get(MET_FIELD)?.clone()).ok()?;
    if mapping.signature != source::trait_signature(row).ok()? || row["Value"] != "basic" {
        return None;
    }
    let traits = crate::sheet::extract(record)
        .lists
        .get("DisciplineList")?
        .get("Traits")?
        .as_array()?
        .clone();
    if traits
        .iter()
        .filter(|r| r.get(MET_FIELD) == row.get(MET_FIELD))
        .count()
        != 1
    {
        return None;
    }
    let matched = crate::vampire_creation::history(record)
        .ok()?
        .iter()
        .any(|receipt| {
            receipt.quote.request.id == mapping.review_id
                && receipt.quote.met_mappings.contains(&mapping)
                && record
                    .build
                    .as_ref()
                    .is_none_or(|build| receipt.committed_revision <= build.source_revision)
        });
    (matched
        && crate::power_catalogue::lookup(&mapping.power_id)
            .is_some_and(|p| p.tier == crate::pricing::MetTier::Basic))
    .then_some(mapping.power_id)
}
pub fn mapped_power(record: &Record, row: &Value) -> Option<String> {
    if let Some(id) = row.get("AppSourcePowerId").and_then(Value::as_str)
        && let Some(entry) = crate::power_catalogue::lookup(id)
        && entry.template_scope == record.kind
        && row["Name"].as_str() == Some(entry.name.as_str())
        && row["Value"]
            .as_str()
            .is_some_and(|v| v.eq_ignore_ascii_case(entry.tier.sheet_value()))
    {
        return Some(id.into());
    }
    crate::met_references::verified(record, row).or_else(|| verified_met(record, row))
}
