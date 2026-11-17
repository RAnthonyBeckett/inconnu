use super::*;
use crate::published_reference as source;
impl Library {
    pub fn published_reference_context(&self, id: &str, role: Role) -> Result<source::Context> {
        require_st(role)?;
        let (revision, data): (i64, String) = self.conn.query_row(
            "SELECT library.revision,records.data FROM library JOIN records ON records.id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let record: Record = serde_json::from_str(&data)?;
        if !crate::sheet::is_character_kind(&record.kind) {
            return Err(error(
                "PUBLISHED_REFERENCE_SCOPE",
                "Choose a Vampire character.",
            ));
        }
        let list = crate::purchases::discipline_list(&record)?;
        let history = source::history(&record)?;
        Ok(source::Context {
            history,
            revision,
            record_id: record.id,
            record_name: record.name,
            traits: list["Traits"].as_array().cloned().unwrap_or_default(),
        })
    }
    pub fn quote_published_reference(
        &self,
        id: &str,
        request: &source::Request,
        expected: i64,
        role: Role,
    ) -> Result<source::Quote> {
        require_st(role)?;
        let (library_id,revision,data):(String,i64,String)=self.conn.query_row(
            "SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; review the source attribution again.",
            ));
        }
        source::quote(
            &library_id,
            revision,
            &serde_json::from_str(&data)?,
            request,
        )
    }
    pub fn commit_published_reference(
        &mut self,
        quote: &source::Quote,
        role: Role,
    ) -> Result<source::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        let data: String = tx.query_row(
            "SELECT data FROM records WHERE id=?1",
            [&quote.record_id],
            |row| row.get(0),
        )?;
        let mut record: Record = serde_json::from_str(&data)?;
        let mut history = source::history(&record)?;
        if library_id != quote.library_id {
            return Err(error(
                "PUBLISHED_REFERENCE_LIBRARY",
                "This review belongs to another library.",
            ));
        }
        if let Some(saved) = history
            .iter()
            .find(|receipt| receipt.quote.request.id == quote.request.id)
        {
            if saved.quote == *quote {
                return Ok(saved.clone());
            }
            return Err(error(
                "PUBLISHED_REFERENCE_ID_CONFLICT",
                "This source review ID already identifies another attribution.",
            ));
        }
        if revision != quote.revision
            || source::quote(&library_id, revision, &record, &quote.request)? != *quote
        {
            return Err(error(
                "STALE_PUBLISHED_REFERENCE",
                "The character or source review changed. Prepare a new review.",
            ));
        }
        let mut list = crate::purchases::discipline_list(&record)?;
        list["Traits"][quote.request.trait_index] = quote.updated_trait.clone();
        let field = crate::creature_profiles::profile(&record.kind)
            .unwrap()
            .primary_power_list
            .clone();
        set_sheet_field(&mut record, &field, list);
        let receipt = source::Receipt {
            quote: quote.clone(),
            committed_revision: revision
                .checked_add(1)
                .ok_or_else(|| error("REVISION_LIMIT", "Library revision limit reached."))?,
        };
        history.push(receipt.clone());
        record
            .private_fields
            .insert(source::FIELD.into(), serde_json::to_value(history)?);
        source::history(&record)?;
        advance(&tx, revision)?;
        if tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, record.id],
        )? != 1
        {
            return Err(error("NOT_FOUND", "The character no longer exists."));
        }
        audit(
            &tx,
            "published_source_attribution",
            Some(&record.id),
            receipt.committed_revision,
        )?;
        tx.commit()?;
        Ok(receipt)
    }
}
