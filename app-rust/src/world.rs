//! Bounded edits for descriptive Item, Location and Rote records.
//! These fields are preserved as entered and do not verify rules or ownership.
use crate::model::{Result, error};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

const ITEM_TEXT: &[&str] = &[
    "ItemType",
    "ItemSubtype",
    "DamageType",
    "Concealability",
    "Powers",
    "Appearance",
    "Notes",
];
const ITEM_INT: &[&str] = &["Level", "Bonus", "DamageAmount"];
const ITEM_LIST: &[&str] = &["TemperList", "NegativeList", "AbilityList", "Availability"];
const LOCATION_TEXT: &[&str] = &[
    "LocType",
    "Owner",
    "Where",
    "Appearance",
    "Access",
    "Security",
    "Affinity",
    "Totem",
    "Umbra",
    "Notes",
];
const LOCATION_INT: &[&str] = &["Level", "SecTraits", "SecRetests", "Gauntlet"];
const LOCATION_LIST: &[&str] = &["LinkList"];
const ROTE_TEXT: &[&str] = &["Duration", "Description", "Grades"];
const ROTE_INT: &[&str] = &["Level"];
const ROTE_LIST: &[&str] = &["SphereList"];
const MAX_FIELDS: usize = 32;
const MAX_NAME_BYTES: usize = 4096;
const MAX_TEXT_BYTES: usize = 16_000;

#[derive(Clone, Default, PartialEq, Serialize, Deserialize, Debug)]
pub struct Edit {
    pub name: String,
    pub fields: BTreeMap<String, Value>,
}

pub fn is_world_kind(kind: &str) -> bool {
    matches!(kind, "Item" | "Location" | "Rote")
}

pub fn text_fields(kind: &str) -> &'static [&'static str] {
    match kind {
        "Item" => ITEM_TEXT,
        "Location" => LOCATION_TEXT,
        "Rote" => ROTE_TEXT,
        _ => &[],
    }
}

pub fn integer_fields(kind: &str) -> &'static [&'static str] {
    match kind {
        "Item" => ITEM_INT,
        "Location" => LOCATION_INT,
        "Rote" => ROTE_INT,
        _ => &[],
    }
}

pub fn list_fields(kind: &str) -> &'static [&'static str] {
    match kind {
        "Item" => ITEM_LIST,
        "Location" => LOCATION_LIST,
        "Rote" => ROTE_LIST,
        _ => &[],
    }
}

pub(crate) fn validate_edit(kind: &str, edit: &Edit) -> Result<()> {
    if !is_world_kind(kind) {
        return Err(error(
            "WORLD_KIND_INVALID",
            "Choose an item, location or rote.",
        ));
    }
    let name = edit.name.trim();
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(error(
            "WORLD_NAME_INVALID",
            "Enter a name within the supported length.",
        ));
    }
    if edit.fields.len() > MAX_FIELDS {
        return Err(error("WORLD_LIMIT", "Too many descriptive fields."));
    }
    for (key, value) in &edit.fields {
        if !value_supported(kind, key, value) {
            return Err(error(
                "WORLD_FIELD_INVALID",
                "That field is not editable for this record.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn value_supported(kind: &str, key: &str, value: &Value) -> bool {
    if text_fields(kind).contains(&key) {
        value.as_str().is_some_and(|s| s.len() <= MAX_TEXT_BYTES)
    } else if integer_fields(kind).contains(&key) {
        value.as_i64().is_some_and(|n| i16::try_from(n).is_ok())
    } else if list_fields(kind).contains(&key) {
        valid_list(value)
    } else {
        false
    }
}

fn valid_list(value: &Value) -> bool {
    if !crate::sheet::editable_list(value) {
        return false;
    }
    let rows = value["Traits"].as_array().unwrap();
    rows.len() <= 4096
        && serde_json::to_vec(value).is_ok_and(|bytes| bytes.len() <= 2 * 1024 * 1024)
        && rows.iter().all(|row| {
            ["Name", "Value", "Note"].iter().all(|key| {
                row.get(*key)
                    .and_then(Value::as_str)
                    .is_none_or(|text| text.len() <= MAX_TEXT_BYTES)
            })
        })
}
