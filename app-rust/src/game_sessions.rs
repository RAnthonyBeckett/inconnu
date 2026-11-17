//! Pure local game-session XP previews. No write, external rules verification, or approval.
use crate::{
    model::{Record, Result, error},
    xp::{self, Entry, XpCommand, XpKind},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const TEXT_LIMIT: usize = 4096;
const ATTENDANCE_LIMIT: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attendance {
    pub record_id: String,
    pub present: bool,
    /// Manual award amount for this selected present character; None retains an excluded row.
    pub award: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRequest {
    pub id: String,
    pub title: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
    pub attendance: Vec<Attendance>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAwardQuote {
    pub record_id: String,
    pub command: XpCommand,
    pub before_earned: String,
    pub before_unspent: String,
    pub after_earned: String,
    pub after_unspent: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionQuote {
    pub request: SessionRequest,
    pub library_id: String,
    pub revision: i64,
    /// Hashes cover the supplied records without exposing their private fields or notes.
    pub record_hashes: BTreeMap<String, String>,
    pub record_names: BTreeMap<String, String>,
    /// Only present characters with an explicit manual award receive commands.
    pub awards: Vec<SessionAwardQuote>,
}

fn required_text(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > TEXT_LIMIT {
        return Err(error(
            "SESSION_EVIDENCE_REQUIRED",
            format!("{field} must be present and within the text limit."),
        ));
    }
    Ok(())
}

fn canonical_uuid(value: &str, field: &str) -> Result<Uuid> {
    let parsed = Uuid::parse_str(value)
        .ok()
        .filter(|id| id.to_string() == value);
    parsed.ok_or_else(|| {
        error(
            "SESSION_ID_INVALID",
            format!("{field} must be a canonical UUID."),
        )
    })
}

fn record_hash(record: &Record) -> Result<String> {
    let digest = Sha256::digest(serde_json::to_vec(record)?);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn command_id(session_id: Uuid, record_id: &str) -> String {
    Uuid::new_v5(
        &session_id,
        format!("game-session-award:{record_id}").as_bytes(),
    )
    .to_string()
}

pub(crate) fn validate_header(
    library_id: &str,
    revision: i64,
    request: &SessionRequest,
) -> Result<Uuid> {
    let session_id = canonical_uuid(&request.id, "Session ID")?;
    for (value, field) in [
        (library_id, "Library ID"),
        (&request.title, "Session title"),
        (&request.actor, "Recorder"),
        (&request.reason, "Reason"),
        (&request.evidence, "Evidence reference"),
    ] {
        required_text(value, field)?;
    }
    if revision < 0 || !xp::day_valid(&request.day) {
        return Err(error(
            "SESSION_DATE_OR_REVISION",
            "Use a valid session date and library revision.",
        ));
    }
    if request.attendance.len() > ATTENDANCE_LIMIT {
        return Err(error(
            "SESSION_ATTENDANCE_INVALID",
            "Attendance must contain a bounded set of records.",
        ));
    }

    Ok(session_id)
}

/// Prepare a read-only session quote from explicit attendance and manual award amounts.
/// A missing ledger blocks only an awarded character; attendance-only rows need no XP opening.
pub fn prepare(
    library_id: &str,
    revision: i64,
    records: &[Record],
    request: &SessionRequest,
) -> Result<SessionQuote> {
    let session_id = validate_header(library_id, revision, request)?;
    if request.attendance.is_empty() {
        return Err(error(
            "SESSION_ATTENDANCE_INVALID",
            "Attendance must contain a bounded set of records.",
        ));
    }

    let mut by_id = BTreeMap::new();
    for record in records {
        canonical_uuid(&record.id, "Character ID")?;
        if by_id.insert(record.id.as_str(), record).is_some() {
            return Err(error(
                "SESSION_RECORD_ID_DUPLICATE",
                "The supplied record set contains duplicate IDs.",
            ));
        }
    }

    let mut seen = BTreeSet::new();
    let mut hashes = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut awards = Vec::new();
    for attendance in &request.attendance {
        canonical_uuid(&attendance.record_id, "Attendance character ID")?;
        if !seen.insert(attendance.record_id.as_str()) {
            return Err(error(
                "SESSION_ATTENDANCE_DUPLICATE",
                "A character can appear only once in session attendance.",
            ));
        }
        if !attendance.present && attendance.award.is_some() {
            return Err(error(
                "SESSION_ABSENT_AWARD",
                "An absent character cannot receive a session award.",
            ));
        }
        let record = by_id.get(attendance.record_id.as_str()).ok_or_else(|| {
            error(
                "SESSION_RECORD_MISSING",
                "An attendance record is not present in the supplied library snapshot.",
            )
        })?;
        if record.build.is_some() || !crate::sheet::is_character_kind(&record.kind) {
            return Err(error(
                "SESSION_CHARACTER_REQUIRED",
                "Only live character records can appear in session attendance.",
            ));
        }
        hashes.insert(record.id.clone(), record_hash(record)?);
        names.insert(record.id.clone(), record.name.clone());

        let current = record.xp_ledger.as_ref().map(xp::replay).transpose()?;
        if let Some(amount) = attendance.award.as_deref() {
            let units = xp::amount(amount)?;
            if units <= 0 {
                return Err(error(
                    "INVALID_XP",
                    "A manual session award must be positive.",
                ));
            }
            let ledger = record.xp_ledger.as_ref().ok_or_else(|| {
                error(
                    "SESSION_XP_OPENING_REQUIRED",
                    "An XP opening is required for every awarded character.",
                )
            })?;
            let command = xp::canonical_command(
                ledger,
                &XpCommand {
                    id: command_id(session_id, &record.id),
                    kind: XpKind::Award,
                    amount: amount.to_string(),
                    day: request.day.clone(),
                    actor: request.actor.clone(),
                    reason: request.reason.clone(),
                    evidence: format!(
                        "Game session {}: {}; {}",
                        request.id, request.title, request.evidence
                    ),
                    reversal_of: None,
                    cost: None,
                },
            )?;
            if ledger
                .entries
                .iter()
                .any(|entry| entry.command.id == command.id)
            {
                return Err(error(
                    "SESSION_COMMAND_DUPLICATE",
                    "This session award command already exists in the XP history.",
                ));
            }
            let (before_earned, before_unspent) = current.ok_or_else(|| {
                error(
                    "SESSION_XP_OPENING_REQUIRED",
                    "An XP opening is required for every awarded character.",
                )
            })?;
            let mut after_ledger = ledger.clone();
            after_ledger.entries.push(Entry {
                command: command.clone(),
                recorded_unix_seconds: 0,
                origin_library_id: library_id.to_string(),
            });
            let (after_earned, after_unspent) = xp::replay(&after_ledger)?;
            awards.push(SessionAwardQuote {
                record_id: record.id.clone(),
                command,
                before_earned: xp::decimal(before_earned),
                before_unspent: xp::decimal(before_unspent),
                after_earned: xp::decimal(after_earned),
                after_unspent: xp::decimal(after_unspent),
            });
        }
    }

    Ok(SessionQuote {
        request: request.clone(),
        library_id: library_id.to_string(),
        revision,
        record_hashes: hashes,
        record_names: names,
        awards,
    })
}
