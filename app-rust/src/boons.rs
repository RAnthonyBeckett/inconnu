//! Read-only legacy Boon projection. Persistence and the editor follow separately.
use crate::{
    model::{Record, Result, error},
    plots, xp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegacyBoon {
    pub id: String,
    pub partner: Option<String>,
    pub kind: Option<String>,
    /// True means the holder owes the named partner.
    pub owed_by_holder: Option<bool>,
    pub day: Option<String>,
    pub raw_date: Value,
    pub description: Option<String>,
    pub original: Value,
}
fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}
fn direction(value: Option<&Value>) -> Option<bool> {
    let value = value?;
    value
        .as_bool()
        .or_else(|| value.get("value").and_then(Value::as_bool))
        .or_else(
            || match value.as_str()?.trim().to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" | "-1" => Some(true),
                "false" | "no" | "0" => Some(false),
                _ => None,
            },
        )
}
fn entry(record: &Record, original: &Value, index: usize, xml: bool) -> Result<LegacyBoon> {
    let field = |binary, attribute| {
        if xml {
            plots::attr(original, attribute)
        } else {
            original.get(binary)
        }
    };
    let raw_date = field("BoonDate", "date").cloned().unwrap_or(Value::Null);
    let display = plots::date_text(&raw_date, false);
    let day = xp::day_valid(&display).then_some(display);
    let description = if xml {
        let descriptions: Vec<_> = plots::children(original)
            .filter(|node| node["tag"] == "description")
            .collect();
        match descriptions.as_slice() {
            [description] if plots::children(description).all(|node| node["text"].is_string()) => {
                Some(plots::node_text(description))
            }
            _ => None,
        }
    } else {
        text(original.get("Description"))
    };
    let mut digest = Sha256::new();
    digest.update(record.id.as_bytes());
    digest.update([0]);
    digest.update(if xml {
        b"xml".as_slice()
    } else {
        b"binary".as_slice()
    });
    digest.update((index as u64).to_le_bytes());
    digest.update(serde_json::to_vec(original)?);
    let hash: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(LegacyBoon {
        id: format!("legacy-boon-{hash}"),
        partner: text(field("CharName", "partner")),
        kind: text(field("BoonType", "type")),
        owed_by_holder: direction(field("IsOwed", "owed")),
        day,
        raw_date,
        description,
        original: original.clone(),
    })
}
pub fn legacy(record: &Record) -> Result<Vec<LegacyBoon>> {
    if record.kind != "Vampire" || record.build.is_some() {
        return Err(error(
            "BOON_HOLDER_REQUIRED",
            "Select a live Vampire holder.",
        ));
    }
    let stored = record
        .private_fields
        .get("BoonList")
        .or_else(|| record.public_fields.get("BoonList"))
        .or_else(|| record.original.pointer("/fields/BoonList"));
    let (entries, xml): (Vec<&Value>, bool) = if let Some(stored) = stored {
        let rows = stored.as_array().ok_or_else(|| {
            error(
                "BOON_SOURCE_INVALID",
                "The retained Boon list requires review.",
            )
        })?;
        (rows.iter().collect(), false)
    } else if let Some(xml) = record.original.get("xml") {
        (
            plots::children(xml)
                .filter(|node| node["tag"] == "boon")
                .collect(),
            true,
        )
    } else {
        (Vec::new(), false)
    };
    if entries.len() > 2000 {
        return Err(error(
            "BOON_SOURCE_LIMIT",
            "The retained Boon list exceeds the projection limit.",
        ));
    }
    entries
        .into_iter()
        .enumerate()
        .map(|(index, source)| entry(record, source, index, xml))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, json};
    fn record() -> Record {
        Record {
            id: "holder-id".into(),
            kind: "Vampire".into(),
            name: "Holder".into(),
            player_id: None,
            legacy_id: None,
            public_fields: Map::new(),
            private_fields: Map::new(),
            original: json!({}),
            source_sha256: String::new(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        }
    }
    #[test]
    fn binary_direction_raw_metadata_dates_and_duplicate_identity_are_preserved() {
        let mut record = record();
        let boon = json!({"BoonType":"Major","CharName":"Partner","IsOwed":{"value":true,"vb_raw":-1},"BoonDate":{"ole_automation_days":46000.0},"Description":"Private description","unknown":{"keep":true}});
        record
            .private_fields
            .insert("BoonList".into(), json!([boon.clone(), boon.clone()]));
        let before = serde_json::to_string(&record).unwrap();
        let entries = legacy(&record).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].owed_by_holder, Some(true));
        assert!(entries[0].day.is_some());
        assert_eq!(entries[0].original, boon);
        assert_ne!(entries[0].id, entries[1].id);
        assert_eq!(entries, legacy(&record).unwrap());
        assert_eq!(serde_json::to_string(&record).unwrap(), before);
    }
    #[test]
    fn xml_missing_or_ambiguous_dates_and_directions_stay_unknown() {
        let xml=br#"<grapevine version="3"><vampire name="Holder"><boon type="Minor" partner="Partner" owed="false" date="04/05/2026"><description>Literal &lt;text&gt;</description><unknown>RETAIN</unknown></boon><boon type="Major" partner="Other" owed="maybe"/></vampire></grapevine>"#;
        let preview = crate::importer::normalize(xml, "synthetic.gex", "windows-1252").unwrap();
        let record = &preview.records[0];
        let entries = legacy(record).unwrap();
        assert_eq!(entries[0].owed_by_holder, Some(false));
        assert_eq!(entries[0].day, None);
        assert_eq!(entries[0].raw_date, "04/05/2026");
        assert_eq!(entries[0].description.as_deref(), Some("Literal <text>"));
        assert!(entries[0].original.to_string().contains("RETAIN"));
        assert_eq!(entries[1].owed_by_holder, None);
        assert_eq!(entries[1].day, None);
        assert_eq!(entries[1].raw_date, Value::Null);
    }
    #[test]
    fn explicit_empty_or_unreadable_current_lists_take_precedence() {
        let mut record = record();
        record.original = json!({"fields":{"BoonList":[{"BoonType":"Retained"}]}});
        record.private_fields.insert("BoonList".into(), json!([]));
        assert!(legacy(&record).unwrap().is_empty());
        record.private_fields.insert("BoonList".into(), Value::Null);
        assert!(legacy(&record).is_err());
        record.kind = "Player".into();
        assert!(legacy(&record).is_err());
    }
}
