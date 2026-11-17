use super::*;
use crate::met_references as met;
impl Library {
    pub fn met_reference_context(&self, id: &str, role: Role) -> Result<met::Context> {
        require_st(role)?;
        let record = self.stored_record(id)?;
        if !crate::sheet::is_character_kind(&record.kind) || record.build.is_some() {
            return Err(error(
                "MET_REFERENCE_SCOPE",
                "Choose an original Vampire; copies inherit applicable source mappings.",
            ));
        }
        Ok(met::Context {
            revision: self.revision()?,
            record_id: record.id.clone(),
            record_name: record.name.clone(),
            traits: crate::purchases::discipline_list(&record)?["Traits"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            history: met::history(&record)?,
        })
    }
    pub fn quote_met_reference(
        &self,
        id: &str,
        request: &met::Request,
        expected: i64,
        role: Role,
    ) -> Result<met::Quote> {
        require_st(role)?;
        let (library_id,revision,data):(String,i64,String)=self.conn.query_row("SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if expected != revision {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing the MET reminder mapping.",
            ));
        }
        met::quote(
            &library_id,
            revision,
            &serde_json::from_str(&data)?,
            request,
        )
    }
    pub fn commit_met_reference(&mut self, quote: &met::Quote, role: Role) -> Result<met::Receipt> {
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
                "MET_REFERENCE_LIBRARY",
                "This review belongs to another library.",
            ));
        }
        let data: String = tx.query_row(
            "SELECT data FROM records WHERE id=?1",
            [&quote.record_id],
            |r| r.get(0),
        )?;
        let mut record: Record = serde_json::from_str(&data)?;
        let mut history = met::history(&record)?;
        if let Some(saved) = history
            .iter()
            .find(|r| r.quote.request.id == quote.request.id)
        {
            if saved.quote == *quote {
                return Ok(saved.clone());
            }
            return Err(error(
                "MET_REFERENCE_ID_CONFLICT",
                "This review ID already identifies another mapping.",
            ));
        }
        if revision != quote.revision
            || met::quote(&library_id, revision, &record, &quote.request)? != *quote
        {
            return Err(error(
                "STALE_REVISION",
                "The character or review changed; preview the MET mapping again.",
            ));
        }
        let mut list = crate::purchases::discipline_list(&record)?;
        list["Traits"][quote.request.trait_index] = quote.updated_trait.clone();
        let field = crate::creature_profiles::profile(&record.kind)
            .unwrap()
            .primary_power_list
            .clone();
        crate::sheet::write_field(&mut record, &field, list);
        let receipt = met::Receipt {
            quote: quote.clone(),
            committed_revision: revision
                .checked_add(1)
                .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?,
        };
        history.push(receipt.clone());
        record
            .private_fields
            .insert(met::FIELD.into(), serde_json::to_value(history)?);
        met::history(&record)?;
        advance(&tx, revision)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, record.id],
        )?;
        audit(&tx, "met_reminder_mapping", Some(&record.id), revision + 1)?;
        tx.commit()?;
        Ok(receipt)
    }
}
