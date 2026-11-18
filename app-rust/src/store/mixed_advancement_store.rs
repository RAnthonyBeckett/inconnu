use super::*;
use crate::mixed_advancement as mixed;
impl Library {
    pub fn quote_mixed_advancement(
        &self,
        id: &str,
        requests: &[mixed::Request],
        expected: i64,
        role: Role,
    ) -> Result<mixed::Quote> {
        require_st(role)?;
        let (library_id,revision,data):(String,i64,String)=self.conn.query_row("SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing this mixed plan.",
            ));
        }
        let record = serde_json::from_str(&data)?;
        let (profile, _) = super::chronicle_store::read_profile(&self.conn)?;
        check_policy(&profile, &record, requests)?;
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing advancement.",
            ));
        }
        mixed::quote(&library_id, revision, &record, requests)
    }
    pub fn mixed_advancement_history(&self, id: &str, role: Role) -> Result<Vec<mixed::Receipt>> {
        require_st(role)?;
        receipts(&self.conn, id)
    }
    pub fn commit_mixed_advancement(
        &mut self,
        quote: &mixed::Quote,
        role: Role,
    ) -> Result<mixed::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let r = commit_in_transaction(&tx, quote)?;
        tx.commit()?;
        Ok(r)
    }
}
pub(super) fn commit_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    q: &mixed::Quote,
) -> Result<mixed::Receipt> {
    let (library_id, revision): (String, i64) =
        tx.query_row("SELECT id,revision FROM library", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    if library_id != q.library_id {
        return Err(error(
            "MIXED_PLAN_LIBRARY",
            "This plan belongs to another library.",
        ));
    }
    let data: String = tx.query_row(
        "SELECT data FROM records WHERE id=?1",
        [&q.record_id],
        |r| r.get(0),
    )?;
    let record: Record = serde_json::from_str(&data)?;
    let history = receipts(tx, &q.record_id)?;
    if let Some(saved) = history.iter().find(|r| r.quote == *q) {
        return Ok(saved.clone());
    }
    if q.steps.is_empty() || q.steps.len() > 32 {
        return Err(error(
            "MIXED_PLAN_LIMIT",
            "A plan requires one to thirty-two steps.",
        ));
    }
    for step in &q.steps {
        let request = step.request();
        let id = request.id();
        let reserved:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE id=?1 OR json_extract(data,'$.cancellation.quote.request.id')=?1) OR EXISTS(SELECT 1 FROM records, json_each(records.data,'$.xp_ledger.entries') e WHERE json_extract(e.value,'$.command.id')=?1)",[id],|r|r.get(0))?;
        if reserved
            || record
                .xp_ledger
                .as_ref()
                .is_some_and(|l| l.entries.iter().any(|e| e.command.id == id))
            || history
                .iter()
                .any(|r| r.quote.steps.iter().any(|s| s.request().id() == id))
        {
            return Err(error(
                "MIXED_PLAN_ID_CONFLICT",
                "A plan ID already belongs to another operation or partial group. Nothing was applied.",
            ));
        }
    }
    if revision != q.revision {
        return Err(error(
            "STALE_REVISION",
            "The library changed; review this mixed plan again.",
        ));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (profile, _) = super::chronicle_store::read_profile(tx)?;
    check_policy(&profile, &record, &q.requests())?;
    let (updated, actual_steps) = mixed::apply(&record, q, now)?;
    crate::owbn_bylaws::check_advancement(&profile, &updated)?;
    let committed_revision = revision
        .checked_add(1)
        .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?;
    for step in &actual_steps {
        if let mixed::Step::Power(power) = step {
            // Preserve existing standalone receipt/audit/correction semantics. The separate
            // private mixed-group proof binds ordinary steps without faking paid-power rows.
            let receipt = crate::purchases::receipt(power, committed_revision);
            tx.execute(
                "INSERT INTO purchase_receipts(id,record_id,data) VALUES(?1,?2,?3)",
                params![
                    power.request.id,
                    q.record_id,
                    serde_json::to_string(&receipt)?
                ],
            )?;
        }
    }
    let receipt = mixed::Receipt {
        quote: q.clone(),
        actual_steps,
        committed_revision,
    };
    mixed::validate_receipt(&receipt)?;
    let bytes = serde_json::to_vec(&receipt)?.len() as i64;
    let (count,total):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(data AS BLOB))),0) FROM mixed_advancement_receipts WHERE record_id=?1",[&q.record_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    if count >= 1024 || total.checked_add(bytes).is_none_or(|n| n > 8 * 1024 * 1024) {
        return Err(error(
            "MIXED_PLAN_LIMIT",
            "Mixed plan history reached its supported limit.",
        ));
    }
    mixed::validate_receipt(&receipt)?;
    tx.execute(
        "INSERT INTO mixed_advancement_receipts(id,record_id,data) VALUES(?1,?2,?3)",
        params![
            mixed::hash(q)?,
            q.record_id,
            serde_json::to_string(&receipt)?
        ],
    )?;
    advance(tx, revision)?;
    tx.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        params![serde_json::to_string(&updated)?, q.record_id],
    )?;
    audit(
        tx,
        "mixed_advancement",
        Some(&q.record_id),
        committed_revision,
    )?;
    Ok(receipt)
}

fn receipts(conn: &Connection, id: &str) -> Result<Vec<mixed::Receipt>> {
    let library_id: String = conn.query_row("SELECT id FROM library", [], |r| r.get(0))?;
    let (count,bytes):(i64,i64)=conn.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(data AS BLOB))),0) FROM mixed_advancement_receipts WHERE record_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    if count > 1024 || bytes > 8 * 1024 * 1024 {
        return Err(error(
            "MIXED_PLAN_LIMIT",
            "Mixed plan history exceeds its supported limit.",
        ));
    }
    let mut statement = conn.prepare(
        "SELECT id,data FROM mixed_advancement_receipts WHERE record_id=?1 ORDER BY rowid",
    )?;
    let mut rows = vec![];
    for row in statement.query_map([id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (hash, data) = row?;
        let receipt: mixed::Receipt = serde_json::from_str(&data)?;
        mixed::validate_receipt(&receipt)?;
        if receipt.quote.record_id != id
            || receipt.quote.library_id != library_id
            || mixed::hash(&receipt.quote)? != hash
        {
            return Err(error(
                "MIXED_PLAN_HISTORY",
                "Mixed plan receipt binding changed.",
            ));
        }
        rows.push(receipt);
    }
    Ok(rows)
}

pub(super) fn check_policy(
    profile: &crate::chronicle::Profile,
    record: &Record,
    requests: &[mixed::Request],
) -> Result<()> {
    crate::owbn_bylaws::check_advancement(profile, record)?;
    for request in requests {
        if let mixed::Request::Power(power) = request {
            crate::owbn_bylaws::check_purchase(profile, record, power)?;
        }
    }
    Ok(())
}
