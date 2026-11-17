use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Storyteller,
    Player,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Portuguese,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Record {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub player_id: Option<String>,
    pub legacy_id: Option<String>,
    pub public_fields: Map<String, Value>,
    pub private_fields: Map<String, Value>,
    pub original: Value,
    pub source_sha256: String,
    #[serde(default)]
    pub st_markers: Option<(String, String)>,
    #[serde(default)]
    pub build: Option<BuildInfo>,
    #[serde(default)]
    pub xp_ledger: Option<crate::xp::Ledger>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum BuildKind {
    Draft,
    Test,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BuildInfo {
    pub source_id: String,
    pub source_revision: i64,
    pub kind: BuildKind,
}
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum PlayerLink {
    #[default]
    Keep,
    Clear,
    Assign(String),
}
#[derive(Clone, Default, PartialEq, Serialize, Deserialize, Debug)]
pub struct SheetEdit {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub identity: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub visibility: std::collections::BTreeMap<String, bool>,
    pub lists: std::collections::BTreeMap<String, Value>,
    pub resources: std::collections::BTreeMap<String, i64>,
    pub player_link: PlayerLink,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Summary {
    pub build: Option<BuildKind>,
    pub id: String,
    pub kind: String,
    pub name: String,
    pub player_name: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RecordView {
    pub build: Option<BuildKind>,
    pub id: String,
    pub kind: String,
    pub name: String,
    pub status: String,
    pub player_name: String,
    pub notes: String,
    pub fields: Map<String, Value>,
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize, Debug)]
pub struct Edit {
    pub name: String,
    pub status: String,
    pub notes: String,
}

#[derive(Clone, Debug)]
pub struct Preview {
    pub source_format: String,
    pub menu_reference: Option<String>,
    pub menu_candidate: Option<std::path::PathBuf>,
    pub source_name: String,
    pub source_hash: String,
    pub records: Vec<Record>,
    pub counts: Vec<(String, usize)>,
    pub conflicts: Vec<String>,
    pub warnings: Vec<String>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) report: Value,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ImportResult {
    pub inserted: usize,
    pub skipped: usize,
    pub revision: i64,
}

#[derive(Debug)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
pub type Result<T> = std::result::Result<T, AppError>;
pub fn error(code: &'static str, message: impl Into<String>) -> AppError {
    AppError {
        code,
        message: message.into(),
    }
}
impl From<rusqlite::Error> for AppError {
    fn from(_: rusqlite::Error) -> Self {
        error(
            "STORAGE_ERROR",
            "The library operation failed. Existing committed data was retained; check diagnostics.",
        )
    }
}
impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        error(
            "FILE_ERROR",
            "The file operation failed. Check the path and permissions.",
        )
    }
}
impl From<serde_json::Error> for AppError {
    fn from(_: serde_json::Error) -> Self {
        error(
            "DATA_ERROR",
            "Stored or imported data could not be decoded.",
        )
    }
}
