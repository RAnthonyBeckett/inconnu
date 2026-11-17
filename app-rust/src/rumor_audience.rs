//! Staff-only, exact-ID audience selections for local Rumor review.

use crate::rumors;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const FIELD: &str = "_AppRumorAudienceV1";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recipient {
    pub character_id: String,
    pub label_at_binding: String,
    /// Retained for forward-compatible metadata edits.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRecipient {
    pub binding: Recipient,
    pub current_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: i64,
    pub rumor_id: String,
    pub rumor: rumors::Rumor,
    pub legacy: bool,
    pub retained: bool,
    pub recipients: Vec<ResolvedRecipient>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    pub revision: i64,
    pub rumor_id: String,
    pub rumor: rumors::Rumor,
    pub legacy: bool,
    pub retained: bool,
    pub recipients: Vec<ResolvedRecipient>,
    pub character_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueRow {
    pub rumor_id: String,
    pub title: String,
    pub day: String,
    pub reviewed: bool,
    pub recipient_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queue {
    pub revision: i64,
    pub from: String,
    pub through: String,
    pub undated_omitted: usize,
    pub rows: Vec<QueueRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct AudienceState {
    pub recipients: Vec<Recipient>,
    /// Unknown root fields are kept when the selection is replaced.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
