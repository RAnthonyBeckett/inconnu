use crate::model::{Result, error};
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub position: String,
    pub status: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub notes: String,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub id: String,
    pub revision: i64,
    pub legacy_id: Option<String>,
    pub profile: Profile,
    pub linked_characters: Vec<(String, String)>,
}
pub(crate) fn validate(p: &Profile) -> Result<()> {
    if p.name.trim().is_empty()
        || p.name.len() > 4096
        || [
            &p.name,
            &p.position,
            &p.status,
            &p.email,
            &p.phone,
            &p.address,
            &p.notes,
        ]
        .iter()
        .any(|s| s.len() > 16000 || s.contains('\0'))
    {
        return Err(error(
            "PLAYER_PROFILE_INVALID",
            "Enter a name and keep profile text within the editor limits.",
        ));
    }
    Ok(())
}
