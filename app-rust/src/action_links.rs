//! Explicit stable character bindings for Action records.
use crate::{actions::Action, story_work};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const FIELD: &str = "_AppActionCharacterV1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub character_id: String,
    pub label_at_binding: String,
    extra: BTreeMap<String, Value>,
}

impl Binding {
    pub(crate) fn new(character_id: String, label_at_binding: String) -> Self {
        Self {
            character_id,
            label_at_binding,
            extra: BTreeMap::new(),
        }
    }
}

impl Serialize for Binding {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(2 + self.extra.len()))?;
        map.serialize_entry("character_id", &self.character_id)?;
        map.serialize_entry("label_at_binding", &self.label_at_binding)?;
        for (key, value) in &self.extra {
            if !matches!(key.as_str(), "character_id" | "label_at_binding") {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Binding {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut fields = Map::<String, Value>::deserialize(deserializer)?;
        let character_id = fields
            .remove("character_id")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("Action binding has no string character ID"))?;
        let label_at_binding = fields
            .remove("label_at_binding")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("Action binding has no string label"))?;
        Ok(Self {
            character_id,
            label_at_binding,
            extra: fields.into_iter().collect(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBinding {
    pub binding: Binding,
    /// Current character name when the saved target is still a live character.
    pub current_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LinkedSnapshot {
    pub story: story_work::Snapshot,
    pub binding: Option<ResolvedBinding>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TimelineRow {
    pub action_id: String,
    pub snapshot: story_work::Snapshot,
}

#[derive(Clone, Debug, Serialize)]
pub struct Timeline {
    pub revision: i64,
    pub character_id: String,
    pub character_name: String,
    pub from: String,
    pub through: String,
    pub rows: Vec<TimelineRow>,
    pub undated_omitted: usize,
    pub unlinked_in_library: usize,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyOptions {
    pub carry_unused: bool,
    pub carry_growth: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyPreview {
    pub revision: i64,
    pub character_id: String,
    pub source_id: String,
    pub source_day: String,
    pub day: String,
    pub options: CopyOptions,
    pub draft: Action,
}

pub(crate) fn read(record: &crate::model::Record) -> crate::model::Result<Option<Binding>> {
    let Some(value) = record.private_fields.get(FIELD) else {
        return Ok(None);
    };
    let binding: Binding = serde_json::from_value(value.clone()).map_err(|_| {
        crate::model::error(
            "ACTION_BINDING_INVALID",
            "The saved character link could not be read. Review or clear it before saving.",
        )
    })?;
    if binding.character_id.trim().is_empty()
        || binding.character_id.len() > 128
        || binding.character_id.contains('\0')
        || binding.label_at_binding.len() > 16_000
        || binding.label_at_binding.contains('\0')
    {
        return Err(crate::model::error(
            "ACTION_BINDING_INVALID",
            "The saved character link has an invalid ID or label.",
        ));
    }
    Ok(Some(binding))
}
