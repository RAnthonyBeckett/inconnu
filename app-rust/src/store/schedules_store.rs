use super::*;
use crate::{
    game_sessions::{self, SessionRequest},
    xp,
};
use rusqlite::Transaction;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A dated local plan; saving it never writes character XP.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledSession {
    pub request: SessionRequest,
    pub saved_revision: i64,
    pub cancelled: bool,
}

fn library_state(tx: &Transaction<'_>) -> Result<(String, i64)> {
    Ok(tx.query_row("SELECT id,revision FROM library", [], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?)
}

fn snapshot(tx: &Transaction<'_>, request: &SessionRequest) -> Result<Vec<Record>> {
    request
        .attendance
        .iter()
        .map(|attendance| {
            let data: Option<String> = tx
                .query_row(
                    "SELECT data FROM records WHERE id=?1",
                    [&attendance.record_id],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(serde_json::from_str(&data.ok_or_else(|| {
                error("NOT_FOUND", "A scheduled character no longer exists.")
            })?)?)
        })
        .collect()
}

fn validate_plan(
    tx: &Transaction<'_>,
    library_id: &str,
    revision: i64,
    request: &SessionRequest,
) -> Result<()> {
    game_sessions::validate_header(library_id, revision, request)?;
    if request.attendance.is_empty() {
        return Ok(());
    }
    let mut validation = request.clone();
    for attendance in &mut validation.attendance {
        if !attendance.present && attendance.award.is_some() {
            return Err(error(
                "SESSION_ABSENT_AWARD",
                "An absent character cannot have a scheduled award.",
            ));
        }
        if let Some(amount) = &attendance.award
            && xp::amount(amount)? <= 0
        {
            return Err(error(
                "INVALID_XP",
                "A scheduled manual award must be positive.",
            ));
        }
        attendance.award = None;
    }
    game_sessions::prepare(
        library_id,
        revision,
        &snapshot(tx, &validation)?,
        &validation,
    )?;
    Ok(())
}

fn saved(tx: &Transaction<'_>, id: &str) -> Result<Option<ScheduledSession>> {
    let data: Option<String> = tx
        .query_row(
            "SELECT data FROM scheduled_sessions WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    data.map(|data| {
        let entry: ScheduledSession = serde_json::from_str(&data)?;
        if entry.request.id != id || entry.saved_revision < 0 {
            return Err(error(
                "SCHEDULE_CORRUPT",
                "Scheduled session identity or revision is inconsistent.",
            ));
        }
        Ok(entry)
    })
    .transpose()
}

fn has_receipt(tx: &Transaction<'_>, id: &str) -> Result<bool> {
    Ok(tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM game_sessions WHERE id=?1)",
        [id],
        |row| row.get(0),
    )?)
}

fn canonical_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id)
}

impl Library {
    pub fn save_scheduled_session(
        &mut self,
        request: &SessionRequest,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision) = library_state(&tx)?;
        if has_receipt(&tx, &request.id)? {
            return Err(error(
                "SCHEDULE_ALREADY_COMMITTED",
                "A session with this ID already has a committed receipt.",
            ));
        }
        validate_plan(&tx, &library_id, revision, request)?;
        let existing = saved(&tx, &request.id)?;
        if let Some(entry) = &existing {
            if entry.cancelled {
                return Err(error(
                    "SCHEDULE_CANCELLED",
                    "A cancelled schedule cannot be edited or reopened.",
                ));
            }
            if entry.request == *request {
                return Ok(revision);
            }
        }

        advance(&tx, expected)?;
        let next = expected
            .checked_add(1)
            .ok_or_else(|| error("REVISION_LIMIT", "Library revision exceeds storage limits."))?;
        let entry = ScheduledSession {
            request: request.clone(),
            saved_revision: next,
            cancelled: false,
        };
        let data = serde_json::to_string(&entry)?;
        if existing.is_some() {
            tx.execute(
                "UPDATE scheduled_sessions SET data=?1 WHERE id=?2",
                params![data, request.id],
            )?;
        } else {
            tx.execute(
                "INSERT INTO scheduled_sessions(id,data) VALUES(?1,?2)",
                params![request.id, data],
            )?;
        }
        audit(&tx, "scheduled_session_save", Some(&request.id), next)?;
        tx.commit()?;
        Ok(next)
    }

    pub fn scheduled_sessions(&self, role: Role) -> Result<Vec<ScheduledSession>> {
        require_st(role)?;
        let mut statement = self.conn.prepare(
            "SELECT id,data FROM scheduled_sessions ORDER BY json_extract(data,'$.request.day'),id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (id, data) = row?;
            let entry: ScheduledSession = serde_json::from_str(&data)?;
            if entry.request.id != id || entry.saved_revision < 0 {
                return Err(error(
                    "SCHEDULE_CORRUPT",
                    "Scheduled session identity or revision is inconsistent.",
                ));
            }
            Ok(entry)
        })
        .collect()
    }

    pub fn cancel_scheduled_session(&mut self, id: &str, expected: i64, role: Role) -> Result<i64> {
        require_st(role)?;
        if !canonical_id(id) {
            return Err(error("SESSION_ID_INVALID", "Use a canonical session UUID."));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (_, revision) = library_state(&tx)?;
        let mut entry = saved(&tx, id)?
            .ok_or_else(|| error("NOT_FOUND", "Scheduled session was not found."))?;
        if entry.cancelled {
            return Ok(revision);
        }
        if has_receipt(&tx, id)? {
            return Err(error(
                "SCHEDULE_ALREADY_COMMITTED",
                "A committed session cannot be cancelled as a plan.",
            ));
        }
        advance(&tx, expected)?;
        let next = expected
            .checked_add(1)
            .ok_or_else(|| error("REVISION_LIMIT", "Library revision exceeds storage limits."))?;
        entry.cancelled = true;
        entry.saved_revision = next;
        tx.execute(
            "UPDATE scheduled_sessions SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&entry)?, id],
        )?;
        audit(&tx, "scheduled_session_cancel", Some(id), next)?;
        tx.commit()?;
        Ok(next)
    }
}

/// Shares the existing schedule validation/representation with a caller-owned atomic review.
pub(super) fn insert_reviewed_plans(
    tx: &Transaction<'_>,
    library_id: &str,
    revision: i64,
    next: i64,
    plans: &[SessionRequest],
) -> Result<()> {
    for request in plans {
        if saved(tx, &request.id)?.is_some() || has_receipt(tx, &request.id)? {
            return Err(error(
                "LEGACY_CALENDAR_PLAN_ID",
                "A planned or completed session already uses this source-entry identity.",
            ));
        }
        validate_plan(tx, library_id, revision, request)?;
        if !request.attendance.is_empty() {
            return Err(error(
                "LEGACY_CALENDAR_ATTENDANCE",
                "Imported calendar plans cannot carry attendance or XP.",
            ));
        }
        let entry = ScheduledSession {
            request: request.clone(),
            saved_revision: next,
            cancelled: false,
        };
        tx.execute(
            "INSERT INTO scheduled_sessions(id,data) VALUES(?1,?2)",
            params![request.id, serde_json::to_string(&entry)?],
        )?;
        audit(tx, "scheduled_session_save", Some(&request.id), next)?;
    }
    Ok(())
}
