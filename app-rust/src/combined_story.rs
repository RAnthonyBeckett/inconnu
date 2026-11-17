//! Read-only, exact-character review of Actions and audience-bound Rumors.

use crate::story_work;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Request {
    pub character_id: String,
    pub from: String,
    pub through: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RumorRow {
    pub record_id: String,
    pub snapshot: story_work::Snapshot,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub request: Request,
    pub actions: crate::action_links::Timeline,
    pub rumors: Vec<RumorRow>,
    /// Library-wide count from the underlying date-scoped Rumor queue.
    pub undated_rumors: usize,
    /// In-range Rumors with no saved recipients, regardless of selected character.
    pub unassigned_rumors_in_range: usize,
}
