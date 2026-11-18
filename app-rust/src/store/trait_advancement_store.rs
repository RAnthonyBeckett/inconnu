use super::*;
use crate::trait_advancement as traits;

impl Library {
    pub fn trait_advancement_context(&self, id: &str, role: Role) -> Result<traits::Context> {
        require_st(role)?;
        let (revision, data): (i64, String) = self.conn.query_row(
            "SELECT library.revision,records.data FROM library JOIN records ON records.id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let record: Record = serde_json::from_str(&data)?;
        if !crate::sheet::is_character_kind(&record.kind) || record.build.is_some() {
            return Err(error(
                "TRAIT_SCOPE",
                "Choose a current Vampire record; copies cannot spend XP.",
            ));
        }
        Ok(traits::Context {
            corrected_ids: record
                .xp_ledger
                .as_ref()
                .map(|l| {
                    crate::trait_corrections::corrected_ids(l)
                        .into_iter()
                        .collect()
                })
                .unwrap_or_default(),
            revision,
            record_id: record.id.clone(),
            record_name: record.name.clone(),
            sheet: crate::sheet::extract(&record),
            unspent: crate::xp::view(&record).unspent,
            history: record
                .xp_ledger
                .as_ref()
                .map(|l| {
                    l.entries
                        .iter()
                        .filter_map(|entry| traits::recorded_quote(&entry.command))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
    pub fn quote_trait_advancement(
        &self,
        id: &str,
        request: &traits::Request,
        expected: i64,
        role: Role,
    ) -> Result<traits::Quote> {
        require_st(role)?;
        let (library_id, revision, data): (String,i64,String) = self.conn.query_row(
            "SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1", [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Reload the current traits before reviewing advancement.",
            ));
        }
        let record = serde_json::from_str(&data)?;
        let (profile, _) = super::chronicle_store::read_profile(&self.conn)?;
        crate::owbn_bylaws::check_advancement(&profile, &record)?;
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing advancement.",
            ));
        }
        traits::quote(&library_id, revision, &record, request)
    }
    pub fn commit_trait_advancement(
        &mut self,
        quote: &traits::Quote,
        role: Role,
    ) -> Result<traits::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        if library_id != quote.library_id {
            return Err(error(
                "TRAIT_LIBRARY",
                "This review belongs to another library.",
            ));
        }
        let data: String = tx.query_row(
            "SELECT data FROM records WHERE id=?1",
            [&quote.record_id],
            |row| row.get(0),
        )?;
        let record: Record = serde_json::from_str(&data)?;
        if let Some(entry) = record
            .xp_ledger
            .as_ref()
            .and_then(|l| l.entries.iter().find(|e| e.command.id == quote.request.id))
        {
            if traits::recorded_quote(&entry.command).as_ref() == Some(quote) {
                return Ok(traits::Receipt {
                    quote: quote.clone(),
                    committed_revision: quote
                        .revision
                        .checked_add(1)
                        .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?,
                });
            }
            return Err(error(
                "TRAIT_ID_CONFLICT",
                "This advancement ID already identifies another entry.",
            ));
        }
        let reserved: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE id=?1 OR json_extract(data,'$.cancellation.quote.request.id')=?1)", [&quote.request.id], |row| row.get(0))?;
        if reserved {
            return Err(error(
                "PURCHASE_ID_RESERVED",
                "This ID belongs to a Discipline receipt.",
            ));
        }
        if revision != quote.revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed; review this advancement again.",
            ));
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let updated = traits::apply(&record, quote, now)?;
        let (profile, _) = super::chronicle_store::read_profile(&tx)?;
        crate::owbn_bylaws::check_advancement(&profile, &record)?;
        crate::owbn_bylaws::check_advancement(&profile, &updated)?;
        advance(&tx, revision)?;
        if tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&updated)?, record.id],
        )? != 1
        {
            return Err(error("NOT_FOUND", "The character no longer exists."));
        }
        audit(&tx, "trait_advancement", Some(&record.id), revision + 1)?;
        tx.commit()?;
        Ok(traits::Receipt {
            quote: quote.clone(),
            committed_revision: revision + 1,
        })
    }
}

impl Library {
    pub fn quote_trait_correction(
        &self,
        id: &str,
        request: &crate::trait_corrections::Request,
        expected: i64,
        role: Role,
    ) -> Result<crate::trait_corrections::Quote> {
        require_st(role)?;
        let (library_id,revision,data):(String,i64,String)=self.conn.query_row("SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing a trait correction.",
            ));
        }
        crate::trait_corrections::quote(
            &library_id,
            revision,
            &serde_json::from_str(&data)?,
            request,
        )
    }
    pub fn commit_trait_correction(
        &mut self,
        q: &crate::trait_corrections::Quote,
        role: Role,
    ) -> Result<crate::trait_corrections::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        if library_id != q.library_id {
            return Err(error(
                "TRAIT_CORRECTION_ORIGIN",
                "This correction belongs to another library.",
            ));
        }
        let data: String = tx.query_row(
            "SELECT data FROM records WHERE id=?1",
            [&q.record_id],
            |r| r.get(0),
        )?;
        let record: Record = serde_json::from_str(&data)?;
        if let Some(entry) = record
            .xp_ledger
            .as_ref()
            .and_then(|l| l.entries.iter().find(|e| e.command.id == q.request.id))
        {
            if crate::trait_corrections::recorded_quote(&entry.command).as_ref() == Some(q) {
                return Ok(crate::trait_corrections::Receipt {
                    quote: q.clone(),
                    committed_revision: q
                        .revision
                        .checked_add(1)
                        .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?,
                });
            }
            return Err(error(
                "TRAIT_ID_CONFLICT",
                "This correction ID identifies another entry.",
            ));
        }
        let reserved:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE id=?1 OR json_extract(data,'$.cancellation.quote.request.id')=?1)",[&q.request.id],|r|r.get(0))?;
        if reserved {
            return Err(error(
                "PURCHASE_ID_RESERVED",
                "This ID belongs to a Discipline receipt.",
            ));
        }
        if revision != q.revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed; review this correction again.",
            ));
        }
        let next = revision
            .checked_add(1)
            .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let updated = crate::trait_corrections::apply(&record, q, now)?;
        advance(&tx, revision)?;
        if tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&updated)?, record.id],
        )? != 1
        {
            return Err(error("NOT_FOUND", "The character no longer exists."));
        }
        audit(&tx, "trait_correction", Some(&record.id), next)?;
        tx.commit()?;
        Ok(crate::trait_corrections::Receipt {
            quote: q.clone(),
            committed_revision: next,
        })
    }
}
