use super::*;
use crate::game_sessions::{self, SessionQuote, SessionRequest};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionReceipt {
    pub quote: SessionQuote,
    pub committed_revision: i64,
}
fn snapshot(conn: &Connection, request: &SessionRequest) -> Result<Vec<Record>> {
    if request.attendance.is_empty() || request.attendance.len() > 10_000 {
        return Err(error(
            "SESSION_ATTENDANCE_INVALID",
            "Select a bounded set of attendance records.",
        ));
    }
    request
        .attendance
        .iter()
        .map(|row| {
            let data: Option<String> = conn
                .query_row(
                    "SELECT data FROM records WHERE id=?1",
                    [&row.record_id],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(serde_json::from_str(&data.ok_or_else(|| {
                error("NOT_FOUND", "A session character no longer exists.")
            })?)?)
        })
        .collect()
}
fn ensure_not_cancelled(conn: &Connection, id: &str) -> Result<()> {
    let cancelled: Option<bool> = conn
        .query_row(
            "SELECT json_extract(data,'$.cancelled') FROM scheduled_sessions WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    if cancelled == Some(true) {
        return Err(error(
            "SESSION_CANCELLED",
            "This scheduled event was cancelled; start a new event.",
        ));
    }
    Ok(())
}
impl Library {
    pub fn quote_session(
        &self,
        request: &SessionRequest,
        expected: i64,
        role: Role,
    ) -> Result<SessionQuote> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        if revision != expected {
            return Err(error("STALE_REVISION", "Refresh the session preview."));
        }
        ensure_not_cancelled(&tx, &request.id)?;
        game_sessions::prepare(&library_id, revision, &snapshot(&tx, request)?, request)
    }
    pub fn commit_session(&mut self, quote: &SessionQuote, role: Role) -> Result<SessionReceipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        if library_id != quote.library_id {
            return Err(error(
                "SESSION_LIBRARY_MISMATCH",
                "This preview belongs to another library.",
            ));
        }
        let saved: Option<String> = tx
            .query_row(
                "SELECT data FROM game_sessions WHERE id=?1",
                [&quote.request.id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(saved) = saved {
            let receipt: SessionReceipt = serde_json::from_str(&saved)?;
            if receipt.quote == *quote {
                return Ok(receipt);
            }
            return Err(error(
                "SESSION_ID_CONFLICT",
                "This session ID already identifies different data.",
            ));
        }
        if revision != quote.revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed; preview the session again.",
            ));
        }
        ensure_not_cancelled(&tx, &quote.request.id)?;
        let mut records = snapshot(&tx, &quote.request)?;
        let checked = game_sessions::prepare(&library_id, revision, &records, &quote.request)?;
        if checked != *quote {
            return Err(error(
                "SESSION_QUOTE_CHANGED",
                "Session preview no longer matches its records or amounts.",
            ));
        }
        for award in &quote.awards {
            let reserved: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE id=?1 OR json_extract(data,'$.cancellation.quote.request.id')=?1)",[&award.command.id],|r|r.get(0))?;
            if reserved {
                return Err(error(
                    "PURCHASE_ID_RESERVED",
                    "An award ID is reserved by a purchase receipt.",
                ));
            }
            let record = records
                .iter_mut()
                .find(|r| r.id == award.record_id)
                .ok_or_else(|| error("NOT_FOUND", "Award character is missing."))?;
            let ledger = record.xp_ledger.as_mut().ok_or_else(|| {
                error(
                    "XP_OPENING_REQUIRED",
                    "An awarded character needs an XP opening.",
                )
            })?;
            ledger.entries.push(crate::xp::Entry {
                command: award.command.clone(),
                origin_library_id: library_id.clone(),
                recorded_unix_seconds: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
            crate::xp::replay(ledger)?;
            if tx.execute(
                "UPDATE records SET data=?1 WHERE id=?2",
                params![serde_json::to_string(record)?, record.id],
            )? != 1
            {
                return Err(error("NOT_FOUND", "Award character disappeared."));
            }
        }
        advance(&tx, revision)?;
        let receipt = SessionReceipt {
            quote: quote.clone(),
            committed_revision: revision + 1,
        };
        tx.execute(
            "INSERT INTO game_sessions(id,data) VALUES(?1,?2)",
            params![quote.request.id, serde_json::to_string(&receipt)?],
        )?;
        tx.execute(
            "DELETE FROM scheduled_sessions WHERE id=?1",
            [&quote.request.id],
        )?;
        audit(&tx, "game_session_commit", None, revision + 1)?;
        tx.commit()?;
        Ok(receipt)
    }
    pub fn session_report(&self, id: &str, language: Language, role: Role) -> Result<String> {
        require_st(role)?;
        let receipts = self.session_receipts(role)?;
        let receipt = receipts
            .iter()
            .find(|receipt| receipt.quote.request.id == id)
            .ok_or_else(|| error("NOT_FOUND", "The session receipt no longer exists."))?;
        crate::session_report::html(receipt, language)
    }
    pub fn session_receipts(&self, role: Role) -> Result<Vec<SessionReceipt>> {
        require_st(role)?;
        let mut statement = self.conn.prepare(
            "SELECT id,data FROM game_sessions ORDER BY json_extract(data,'$.quote.request.day'),id",
        )?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let library_id = self.library_id()?;
        let mut receipts = Vec::new();
        for row in rows {
            let (stored_id, data) = row?;
            let receipt: SessionReceipt = serde_json::from_str(&data)?;
            if receipt.quote.request.id != stored_id
                || receipt.quote.library_id != library_id
                || receipt.committed_revision
                    != receipt.quote.revision.checked_add(1).ok_or_else(|| {
                        error("INVALID_RECEIPT", "Receipt revision exceeds bounds.")
                    })?
            {
                return Err(error(
                    "INVALID_RECEIPT",
                    "Session receipt identity is inconsistent.",
                ));
            }
            receipts.push(receipt);
        }
        Ok(receipts)
    }
}
