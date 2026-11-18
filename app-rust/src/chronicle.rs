//! Library-level Chronicle profile and private storyteller handover projections.

use serde::{Deserialize, Serialize};

/// Editable local information about the Chronicle associated with one library.
///
/// Contacts and notes are private unless a caller uses the explicit public
/// projection options below. Legacy Game metadata is not copied into this model.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub title: String,
    pub website: String,
    pub email: String,
    pub phone: String,
    pub meeting_place: String,
    pub meeting_time: String,
    pub description: String,
    pub house_rules: String,
    pub private_notes: String,
    pub publish_details: bool,
    pub publish_contacts: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub owbn_bylaws_enabled: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_non_binding_sources: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub house_rules_pack: Option<crate::house_rules::HouseRulesPack>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: i64,
    pub profile: Profile,
}

/// Deliberately allowlisted public Chronicle information.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicProfile {
    pub title: String,
    pub website: String,
    pub email: String,
    pub phone: String,
    pub meeting_place: String,
    pub meeting_time: String,
    pub description: String,
    pub house_rules: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub owbn_bylaws_enabled: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_non_binding_sources: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub house_rules_pack: Option<crate::house_rules::Summary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicSnapshot {
    pub revision: i64,
    pub profile: PublicProfile,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count {
    pub key: String,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub hash: String,
    pub format: String,
    /// Basename only; empty unless the handover explicitly includes source names.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handover {
    pub revision: i64,
    pub profile: Profile,
    pub include_contacts: bool,
    pub include_notes: bool,
    pub include_source_names: bool,
    pub counts: Vec<Count>,
    pub sources: Vec<Source>,
}
