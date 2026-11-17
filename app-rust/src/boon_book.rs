//! Holder-owned Boon edits layered over preserved legacy evidence.
use crate::{
    boons,
    model::{Record, Result, error},
};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError, ser::Error as SerError,
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const FIELD: &str = "_AppBoonsV1";

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Edit {
    pub partner: String,
    pub kind: String,
    /// True means the holder owes the named partner.
    pub owed_by_holder: Option<bool>,
    /// Empty means that the legacy date is unresolved.
    pub day: String,
    pub description: String,
}

/// An application-owned stable ID with its editable projection and immutable source evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub edit: Edit,
    pub legacy: Option<boons::LegacyBoon>,
    extra: BTreeMap<String, Value>,
    edit_extra: BTreeMap<String, Value>,
}

impl Entry {
    pub(crate) fn new(id: String, edit: Edit, legacy: Option<boons::LegacyBoon>) -> Self {
        Self {
            id,
            edit,
            legacy,
            extra: BTreeMap::new(),
            edit_extra: BTreeMap::new(),
        }
    }
}

impl Serialize for Entry {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        let mut edit = serde_json::to_value(&self.edit).map_err(S::Error::custom)?;
        if let Some(fields) = edit.as_object_mut() {
            for (key, value) in &self.edit_extra {
                fields.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        let mut map = serializer.serialize_map(Some(3 + self.extra.len()))?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("edit", &edit)?;
        map.serialize_entry("legacy", &self.legacy)?;
        for (key, value) in &self.extra {
            if !matches!(key.as_str(), "id" | "edit" | "legacy") {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut fields = Map::<String, Value>::deserialize(deserializer)?;
        let id = fields
            .remove("id")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("Boon entry has no string ID"))?;
        let edit_value = fields
            .remove("edit")
            .ok_or_else(|| D::Error::custom("Boon entry has no edit"))?;
        let edit: Edit = serde_json::from_value(edit_value.clone()).map_err(D::Error::custom)?;
        let edit_extra = edit_value
            .as_object()
            .into_iter()
            .flat_map(|edit_fields| edit_fields.iter())
            .filter(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "partner" | "kind" | "owed_by_holder" | "day" | "description"
                )
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let legacy = fields
            .remove("legacy")
            .filter(|value| !value.is_null())
            .map(serde_json::from_value)
            .transpose()
            .map_err(D::Error::custom)?;
        let extra = fields.into_iter().collect();
        Ok(Self {
            id,
            edit,
            legacy,
            extra,
            edit_extra,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub holder_id: String,
    pub holder_name: String,
    pub revision: i64,
    pub entries: Vec<Entry>,
    pub removed_count: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct State {
    pub entries: Vec<Entry>,
    pub removed: Vec<Entry>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

const ENTRY_LIMIT: usize = 2000;
const TEXT_LIMIT: usize = 16_000;

fn invalid(message: &str) -> crate::model::AppError {
    error("BOON_EDIT_INVALID", message)
}

fn valid_text(text: &str) -> bool {
    text.len() <= TEXT_LIMIT && !text.contains('\0')
}

fn validate_state(state: &State) -> Result<()> {
    if state.entries.len().saturating_add(state.removed.len()) > ENTRY_LIMIT {
        return Err(error(
            "BOON_LIMIT",
            "This Vampire's Boon list exceeds the 2,000-entry limit.",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    for entry in state.entries.iter().chain(state.removed.iter()) {
        if entry.id.is_empty()
            || entry.id.len() > 128
            || entry.id.contains('\0')
            || !ids.insert(entry.id.as_str())
        {
            return Err(error(
                "BOON_OVERLAY_INVALID",
                "A saved Boon has an invalid or repeated ID. The retained list needs review.",
            ));
        }
        if [
            entry.edit.partner.as_str(),
            entry.edit.kind.as_str(),
            entry.edit.day.as_str(),
            entry.edit.description.as_str(),
        ]
        .into_iter()
        .any(|text| !valid_text(text))
        {
            return Err(error(
                "BOON_OVERLAY_INVALID",
                "A saved Boon contains text outside the supported size or character limits.",
            ));
        }
        if entry.legacy.is_none() && validate_new(&entry.edit).is_err() {
            return Err(error(
                "BOON_OVERLAY_INVALID",
                "A saved app-owned Boon is missing a partner, type, direction or valid date.",
            ));
        }
    }
    Ok(())
}

fn adopted(legacy: boons::LegacyBoon) -> Entry {
    let edit = Edit {
        partner: legacy.partner.clone().unwrap_or_default(),
        kind: legacy.kind.clone().unwrap_or_default(),
        owed_by_holder: legacy.owed_by_holder,
        day: legacy.day.clone().unwrap_or_default(),
        description: legacy.description.clone().unwrap_or_default(),
    };
    Entry::new(legacy.id.clone(), edit, Some(legacy))
}

pub(crate) fn state(record: &Record) -> Result<State> {
    let state = match record.private_fields.get(FIELD) {
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| {
            error(
                "BOON_OVERLAY_INVALID",
                "The saved Boon edits could not be read. The retained list needs review.",
            )
        })?,
        None => State {
            entries: boons::legacy(record)?.into_iter().map(adopted).collect(),
            ..State::default()
        },
    };
    validate_state(&state)?;
    Ok(state)
}

pub(crate) fn validate_new(edit: &Edit) -> Result<()> {
    validate_text(edit)?;
    if edit.partner.trim().is_empty()
        || edit.kind.trim().is_empty()
        || edit.owed_by_holder.is_none()
        || !crate::xp::day_valid(&edit.day)
    {
        return Err(invalid(
            "Enter a partner, Boon type, direction and valid YYYY-MM-DD date.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_change(old: &Edit, new: &Edit) -> Result<()> {
    validate_text(new)?;
    let retained_empty = |before: &str, after: &str| before == after && before.trim().is_empty();
    if (new.partner.trim().is_empty() && !retained_empty(&old.partner, &new.partner))
        || (new.kind.trim().is_empty() && !retained_empty(&old.kind, &new.kind))
    {
        return Err(invalid("Enter a partner and Boon type."));
    }
    if new.day != old.day && !crate::xp::day_valid(&new.day) {
        return Err(invalid("A changed Boon date must be valid YYYY-MM-DD."));
    }
    if new.owed_by_holder.is_none() && new.owed_by_holder != old.owed_by_holder {
        return Err(invalid("A changed Boon direction must be selected."));
    }
    Ok(())
}

fn validate_text(edit: &Edit) -> Result<()> {
    if [
        edit.partner.as_str(),
        edit.kind.as_str(),
        edit.day.as_str(),
        edit.description.as_str(),
    ]
    .into_iter()
    .any(|text| !valid_text(text))
    {
        return Err(invalid(
            "Boon text must be at most 16,000 characters and contain no NUL characters.",
        ));
    }
    Ok(())
}

pub(crate) fn snapshot(holder: &Record, revision: i64, state: State) -> Snapshot {
    Snapshot {
        holder_id: holder.id.clone(),
        holder_name: holder.name.clone(),
        revision,
        entries: state.entries,
        removed_count: state.removed.len(),
    }
}
