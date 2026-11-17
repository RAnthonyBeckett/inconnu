use super::*;
use crate::character_lifecycle as lifecycle;
impl Library {
    pub fn character_lifecycle_context(&self, id: &str, role: Role) -> Result<lifecycle::Context> {
        require_st(role)?;
        let record = self.stored_record(id)?;
        lifecycle::scope(&record)?;
        let history = lifecycle::history(&record)?;
        let status = lifecycle::status(&record).to_owned();
        let status_drift = history.last().is_some_and(|r| r.quote.after != status);
        Ok(lifecycle::Context {
            revision: self.revision()?,
            name: record.name.clone(),
            status,
            created_in_app: record.original["created_in_app"] == true,
            has_xp_opening: record.xp_ledger.is_some(),
            history,
            status_drift,
        })
    }
    pub fn quote_character_lifecycle(
        &self,
        id: &str,
        request: &lifecycle::Request,
        expected: i64,
        role: Role,
    ) -> Result<lifecycle::Quote> {
        require_st(role)?;
        let (library_id,revision,data):(String,i64,String)=self.conn.query_row("SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing a lifecycle decision.",
            ));
        }
        lifecycle::quote(
            &library_id,
            revision,
            &serde_json::from_str(&data)?,
            request,
        )
    }
    pub fn commit_character_lifecycle(
        &mut self,
        quote: &lifecycle::Quote,
        role: Role,
    ) -> Result<lifecycle::Receipt> {
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
                "LIFECYCLE_LIBRARY",
                "This decision belongs to another library.",
            ));
        }
        let data: String = tx.query_row(
            "SELECT data FROM records WHERE id=?1",
            [&quote.record_id],
            |r| r.get(0),
        )?;
        let mut record: Record = serde_json::from_str(&data)?;
        let mut history = lifecycle::history(&record)?;
        if let Some(saved) = history
            .iter()
            .find(|r| r.quote.request.id == quote.request.id)
        {
            if saved.quote == *quote {
                return Ok(saved.clone());
            }
            return Err(error(
                "LIFECYCLE_ID_CONFLICT",
                "This decision ID already identifies another request.",
            ));
        }
        if revision != quote.revision
            || lifecycle::quote(&library_id, revision, &record, &quote.request)? != *quote
        {
            return Err(error(
                "STALE_REVISION",
                "Character or review changed; preview this decision again.",
            ));
        }
        crate::sheet::write_field(&mut record, "Status", Value::String(quote.after.clone()));
        let receipt = lifecycle::Receipt {
            quote: quote.clone(),
            committed_revision: revision
                .checked_add(1)
                .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?,
        };
        history.push(receipt.clone());
        record
            .private_fields
            .insert(lifecycle::FIELD.into(), serde_json::to_value(history)?);
        lifecycle::history(&record)?;
        advance(&tx, revision)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, record.id],
        )?;
        audit(&tx, "character_lifecycle", Some(&record.id), revision + 1)?;
        tx.commit()?;
        Ok(receipt)
    }
}
