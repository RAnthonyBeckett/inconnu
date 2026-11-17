//! Typed, non-executing editor projection for legacy Action batches.
//! The original reader payload remains the source of truth for unsupported data.
use crate::{
    model::{Record, Result, error},
    plots::{attr, children, date_text, node_text},
    xp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const FIELD: &str = "AppActionV1";
const SLOT_LIMIT: usize = 1000;
const TEXT_LIMIT: usize = 16_000;
const CHARACTER_LIMIT: usize = 512;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    /// Legacy `CharName`; this is a display label, never a foreign key.
    pub character: String,
    /// Calendar day in `YYYY-MM-DD` form when the source date is understood.
    pub day: String,
    /// Legacy `SubActions`, kept in their source order.
    pub slots: Vec<ActivitySlot>,
}

impl Action {
    /// Completion follows ActionClass: at least one slot, and each slot has
    /// nonempty action and result text. It does not execute or award effects.
    pub fn is_complete(&self) -> bool {
        !self.slots.is_empty()
            && self
                .slots
                .iter()
                .all(|slot| !slot.action.is_empty() && !slot.result.is_empty())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivitySlot {
    pub name: String,
    /// Draft strings preserve malformed source values for inspection.
    pub level: String,
    pub unused: String,
    pub total: String,
    pub growth: String,
    pub action: String,
    pub result: String,
}

impl Default for ActivitySlot {
    fn default() -> Self {
        Self {
            name: String::new(),
            level: "0".into(),
            unused: "0".into(),
            total: "0".into(),
            growth: "0".into(),
            action: String::new(),
            result: String::new(),
        }
    }
}

pub(crate) fn validate(action: &Action) -> Result<()> {
    let bounded = |text: &str| text.len() <= TEXT_LIMIT && !text.contains('\0');
    if action.character.trim().is_empty()
        || action.character.len() > CHARACTER_LIMIT
        || !bounded(&action.character)
        || action.slots.len() > SLOT_LIMIT
        || action.slots.iter().any(|slot| {
            slot.name.trim().is_empty()
                || !bounded(&slot.name)
                || !bounded(&slot.action)
                || !bounded(&slot.result)
        })
    {
        return Err(error(
            "ACTION_FIELDS_INVALID",
            "Enter a character label and activity names; keep text and row counts within the editor limits.",
        ));
    }
    if !xp::day_valid(&action.day) {
        return Err(error(
            "ACTION_DATE_INVALID",
            "Use a valid calendar day in YYYY-MM-DD form.",
        ));
    }
    for slot in &action.slots {
        for value in [&slot.level, &slot.unused, &slot.total, &slot.growth] {
            if value.len() > 16 || value.trim().parse::<i16>().is_err() {
                return Err(error(
                    "ACTION_NUMBER_INVALID",
                    "Activity level, unused, total and growth values must be signed 16-bit integers.",
                ));
            }
        }
    }
    Ok(())
}

/// Read either the current AppActionV1 projection or the legacy source shape.
/// `legacy` identifies XML/binary source. `retained` warns that effects,
/// unknown fields, or malformed source values remain only in `Record.original`.
pub(crate) fn read(record: &Record) -> Result<(Action, bool, bool)> {
    if record.kind != "Action" {
        return Err(error("ACTION_KIND_REQUIRED", "Select an Action record."));
    }
    let xml = record.original.get("xml");
    let binary = record.original.get("fields");
    let legacy = xml.is_some() || binary.is_some();
    let (source, retained) = if let Some(xml) = xml {
        read_xml(xml)
    } else if let Some(binary) = binary {
        read_binary(binary)
    } else {
        (Action::default(), false)
    };

    if let Some(value) = record.private_fields.get(FIELD) {
        return Ok((serde_json::from_value(value.clone())?, legacy, retained));
    }
    Ok((source, legacy, retained))
}

fn read_xml(node: &Value) -> (Action, bool) {
    let mut action = Action::default();
    let mut retained =
        has_unknown_xml_attributes(node, &["date", "character", "done", "lastmodified"]);

    if let Some(value) = attr(node, "date") {
        action.day = date_text(value, false);
        retained |= !xp::day_valid(&action.day);
    } else {
        retained = true;
    }
    if let Some(value) = attr(node, "character") {
        action.character = value_text(value);
        retained |= !value.is_string();
    } else {
        retained = true;
    }
    if let Some(value) = attr(node, "lastmodified") {
        let last_modified = date_text(value, false);
        retained |= !xp::day_valid(&last_modified);
    }
    let source_done = match attr(node, "done") {
        Some(value) => match xml_boolean(value) {
            Some(done) => done,
            None => {
                retained = true;
                false
            }
        },
        None => false,
    };

    for child in children(node) {
        match child["tag"].as_str() {
            Some("subaction") => {
                let (slot, slot_retained) = read_xml_slot(child);
                action.slots.push(slot);
                retained |= slot_retained;
                if action.slots.len() > SLOT_LIMIT {
                    action.slots.truncate(SLOT_LIMIT);
                    retained = true;
                    break;
                }
            }
            Some(_) => retained = true,
            None if child["text"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()) =>
            {
                retained = true;
            }
            None => {}
        }
    }
    retained |= source_done != action.is_complete();
    (action, retained)
}

fn read_xml_slot(node: &Value) -> (ActivitySlot, bool) {
    let mut slot = ActivitySlot::default();
    let mut retained =
        has_unknown_xml_attributes(node, &["name", "level", "unused", "total", "growth"]);
    if let Some(value) = attr(node, "name") {
        slot.name = value_text(value);
        retained |= !value.is_string();
    } else {
        retained = true;
    }
    slot.level = xml_integer_draft(node, "level", &mut retained);
    slot.unused = xml_integer_draft(node, "unused", &mut retained);
    slot.total = xml_integer_draft(node, "total", &mut retained);
    slot.growth = xml_integer_draft(node, "growth", &mut retained);

    let mut saw_action = false;
    let mut saw_result = false;
    for child in children(node) {
        match child["tag"].as_str() {
            Some("act") => {
                retained |= saw_action || has_element_children(child);
                saw_action = true;
                slot.action = node_text(child);
            }
            Some("results") => {
                retained |= saw_result || has_element_children(child);
                saw_result = true;
                slot.result = node_text(child);
            }
            Some("linklist") => retained = true,
            Some(_) => retained = true,
            None if child["text"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()) =>
            {
                retained = true;
            }
            None => {}
        }
    }
    (slot, retained)
}

fn read_binary(fields: &Value) -> (Action, bool) {
    let mut action = Action::default();
    let mut retained = has_unknown_object_keys(
        fields,
        &["ActDate", "CharName", "Done", "LastModified", "SubActions"],
    );

    if let Some(value) = fields.get("ActDate") {
        let date = value.get("ole_automation_days").unwrap_or(value);
        action.day = date_text(date, false);
        retained |= !xp::day_valid(&action.day);
    } else {
        retained = true;
    }
    if let Some(value) = fields.get("CharName") {
        action.character = value_text(value);
        retained |= !value.is_string();
    } else {
        retained = true;
    }
    if let Some(value) = fields.get("LastModified") {
        let date = value.get("ole_automation_days").unwrap_or(value);
        let last_modified = date_text(date, false);
        retained |= !xp::day_valid(&last_modified);
    }
    let source_done = match fields.get("Done") {
        Some(value) => match binary_boolean(value) {
            Some((done, noncanonical)) => {
                retained |= noncanonical;
                done
            }
            None => {
                retained = true;
                false
            }
        },
        None => {
            retained = true;
            false
        }
    };

    match fields.get("SubActions").and_then(Value::as_array) {
        Some(rows) => {
            if rows.len() > SLOT_LIMIT {
                retained = true;
            }
            for row in rows.iter().take(SLOT_LIMIT) {
                let (slot, row_retained) = read_binary_slot(row);
                action.slots.push(slot);
                retained |= row_retained;
            }
        }
        None => retained = true,
    }
    retained |= source_done != action.is_complete();
    (action, retained)
}

fn read_binary_slot(row: &Value) -> (ActivitySlot, bool) {
    let mut slot = ActivitySlot::default();
    let mut retained = has_unknown_object_keys(
        row,
        &[
            "Name", "Level", "Unused", "Total", "Growth", "Effects", "Action", "Result",
        ],
    );
    slot.name = binary_text(row.get("Name"), &mut retained);
    slot.level = binary_integer_draft(row.get("Level"), &mut retained);
    slot.unused = binary_integer_draft(row.get("Unused"), &mut retained);
    slot.total = binary_integer_draft(row.get("Total"), &mut retained);
    slot.growth = binary_integer_draft(row.get("Growth"), &mut retained);
    slot.action = binary_text(row.get("Action"), &mut retained);
    slot.result = binary_text(row.get("Result"), &mut retained);

    match row.get("Effects") {
        Some(Value::Object(effects)) => {
            if effects.keys().any(|key| key != "Entries") {
                retained = true;
            }
            match effects.get("Entries") {
                Some(Value::Array(entries)) if !entries.is_empty() => retained = true,
                Some(Value::Array(_)) => {}
                _ => retained = true,
            }
        }
        _ => retained = true,
    }
    (slot, retained)
}

fn has_unknown_xml_attributes(node: &Value, known: &[&str]) -> bool {
    node["attributes"].as_array().is_some_and(|attributes| {
        attributes.iter().any(|attribute| {
            attribute["name"]
                .as_str()
                .is_none_or(|name| !known.contains(&name))
        })
    })
}

fn has_unknown_object_keys(value: &Value, known: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.keys().any(|key| !known.contains(&key.as_str())))
        || !value.is_object()
}

fn has_element_children(node: &Value) -> bool {
    children(node).any(|child| child.get("tag").is_some())
}

fn value_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn xml_integer_draft(node: &Value, key: &str, retained: &mut bool) -> String {
    let Some(value) = attr(node, key) else {
        // The legacy XML reader's optional Integer arguments default to zero.
        return "0".into();
    };
    let text = value_text(value);
    if !value.is_string() || text.trim().parse::<i16>().is_err() {
        *retained = true;
    }
    text
}

fn binary_text(value: Option<&Value>, retained: &mut bool) -> String {
    match value {
        Some(value) if value.is_string() => value.as_str().unwrap_or_default().to_owned(),
        Some(value) => {
            *retained = true;
            value_text(value)
        }
        None => {
            *retained = true;
            String::new()
        }
    }
}

fn binary_integer_draft(value: Option<&Value>, retained: &mut bool) -> String {
    let Some(value) = value else {
        // A binary ActionNode always serializes all four signed 16-bit values.
        *retained = true;
        return "0".into();
    };
    let text = value_text(value);
    let valid = value
        .as_i64()
        .is_some_and(|number| i16::try_from(number).is_ok())
        || value
            .as_str()
            .is_some_and(|number| number.trim().parse::<i16>().is_ok());
    if !valid {
        *retained = true;
    }
    text
}

fn xml_boolean(value: &Value) -> Option<bool> {
    let text = value.as_str()?.trim().to_ascii_lowercase();
    match text.as_str() {
        "yes" | "y" | "true" | "t" => Some(true),
        "no" | "n" | "false" | "f" => Some(false),
        _ => None,
    }
}

fn binary_boolean(value: &Value) -> Option<(bool, bool)> {
    let object = value.as_object()?;
    let done = object.get("value")?.as_bool()?;
    let raw = object.get("vb_raw")?.as_i64()?;
    let unknown = object.keys().any(|key| key != "value" && key != "vb_raw");
    Some((done, unknown || (raw != 0 && raw != -1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, json};

    fn record(original: Value) -> Record {
        Record {
            id: "action-test-id".into(),
            kind: "Action".into(),
            name: "2020-03-04 — Test character".into(),
            player_id: None,
            legacy_id: None,
            public_fields: Map::new(),
            private_fields: Map::new(),
            original,
            source_sha256: String::new(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        }
    }

    #[test]
    fn xml_projection_keeps_order_malformed_values_and_unsupported_data() {
        let original = json!({
            "xml": {
                "tag": "action",
                "attributes": [
                    {"name":"date", "value":"03/04/2020"},
                    {"name":"character", "value":"Test character"},
                    {"name":"done", "value":"yes"},
                    {"name":"lastmodified", "value":"2020-03-04"}
                ],
                "children": [
                    {
                        "tag":"subaction",
                        "attributes":[
                            {"name":"name", "value":"Contacts"},
                            {"name":"level", "value":"2"},
                            {"name":"unused", "value":"1"},
                            {"name":"total", "value":"03"},
                            {"name":"growth", "value":"0"}
                        ],
                        "children":[
                            {"tag":"act", "children":[{"text":"Call"}]},
                            {"tag":"results", "children":[{"text":"No response"}]},
                            {"tag":"linklist", "attributes":[], "children":[]}
                        ]
                    },
                    {
                        "tag":"subaction",
                        "attributes":[
                            {"name":"name", "value":"Influence"},
                            {"name":"level", "value":"not-an-integer"}
                        ],
                        "children":[{"tag":"act", "children":[{"text":"Petition"}]}]
                    },
                    {"tag":"unknown", "children":[{"text":"keep"}]}
                ]
            }
        });
        let record = record(original.clone());
        let (action, legacy, retained) = read(&record).unwrap();
        assert!(legacy);
        assert!(retained);
        assert_eq!(action.character, "Test character");
        assert_eq!(action.day, "03/04/2020");
        assert_eq!(action.slots.len(), 2);
        assert_eq!(action.slots[0].name, "Contacts");
        assert_eq!(action.slots[0].total, "03");
        assert_eq!(action.slots[0].action, "Call");
        assert_eq!(action.slots[0].result, "No response");
        assert_eq!(action.slots[1].name, "Influence");
        assert_eq!(action.slots[1].level, "not-an-integer");
        assert_eq!(action.slots[1].result, "");
        assert!(!action.is_complete());
        assert!(validate(&action).is_err());
        assert_eq!(record.original, original);
    }

    #[test]
    fn binary_projection_unwraps_ole_date_and_flags_effects_and_noncanonical_values() {
        let original = json!({
            "fields": {
                "ActDate":{"ole_automation_days":45000.75},
                "CharName":"Test character",
                "Done":{"value":true,"vb_raw":1},
                "LastModified":{"ole_automation_days":45001.0},
                "SubActions":[
                    {
                        "Name":"Contacts","Level":2,"Unused":1,"Total":3,"Growth":0,
                        "Effects":{"Entries":[{"Target":1,"When":{"ole_automation_days":45000.0},"Item":"Influence","Subitem":"Contacts"}]},
                        "Action":"Call","Result":"No response"
                    },
                    {
                        "Name":"Influence","Level":1,"Unused":0,"Total":2,"Growth":1,
                        "Effects":{"Entries":[]},"Action":"Petition","Result":"Accepted"
                    }
                ],
                "UnknownField":"retained"
            }
        });
        let record = record(original.clone());
        let (action, legacy, retained) = read(&record).unwrap();
        assert!(legacy);
        assert!(retained);
        assert_eq!(action.day, "2023-03-15");
        assert_eq!(action.character, "Test character");
        assert_eq!(action.slots.len(), 2);
        assert_eq!(action.slots[0].unused, "1");
        assert_eq!(action.slots[1].growth, "1");
        assert!(action.is_complete());
        assert!(validate(&action).is_ok());
        assert_eq!(record.original, original);
    }

    #[test]
    fn malformed_binary_date_and_new_empty_batch_remain_inspectable() {
        let record = record(json!({
            "fields": {
                "ActDate":{"ole_automation_days":"not-a-date"},
                "CharName":"Test character",
                "Done":{"value":false,"vb_raw":0},
                "LastModified":{"ole_automation_days":45001.0},
                "SubActions":[]
            }
        }));
        let (action, legacy, retained) = read(&record).unwrap();
        assert!(legacy);
        assert!(retained);
        assert_eq!(action.day, "not-a-date");
        assert!(action.slots.is_empty());
        assert!(!action.is_complete());
        assert!(validate(&action).is_err());

        let empty = Action::default();
        assert!(!empty.is_complete());
    }

    #[test]
    fn private_action_overlay_is_a_new_record_projection() {
        let mut record = record(json!({"created_in_app":true}));
        let action = Action {
            character: "New label".into(),
            day: "2026-11-10".into(),
            slots: vec![ActivitySlot {
                name: "Contacts".into(),
                action: "Call".into(),
                result: "Answered".into(),
                ..ActivitySlot::default()
            }],
        };
        record
            .private_fields
            .insert(FIELD.into(), serde_json::to_value(&action).unwrap());
        let (saved, legacy, retained) = read(&record).unwrap();
        assert_eq!(saved, action);
        assert!(!legacy);
        assert!(!retained);
        assert!(saved.is_complete());
        assert!(validate(&saved).is_ok());
    }
}
