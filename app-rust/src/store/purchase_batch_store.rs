use super::*;
use crate::{purchase_batch as batch, purchases};

impl Library {
    /// Capture one live-record/revision snapshot, then simulate the entire plan.
    pub fn quote_purchase_batch(
        &self,
        id: &str,
        requests: &[purchases::PurchaseRequest],
        expected: i64,
        role: Role,
    ) -> Result<batch::Quote> {
        require_st(role)?;
        let snapshot: Option<(String, i64, String)> = self.conn.query_row(
            "SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",
            [id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional()?;
        let (library_id, revision, data) =
            snapshot.ok_or_else(|| error("NOT_FOUND", "Record no longer exists."))?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; review the advancement plan again.",
            ));
        }
        let record = serde_json::from_str(&data)?;
        let (profile, _) = super::chronicle_store::read_profile(&self.conn)?;
        for request in requests {
            crate::owbn_bylaws::check_purchase(&profile, &record, request)?;
        }
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing advancement.",
            ));
        }
        batch::quote(&library_id, revision, &record, requests)
    }

    /// One transaction preserves the current live record and all-or-nothing XP/traits/receipts.
    pub fn commit_purchase_batch(
        &mut self,
        quote: &batch::Quote,
        role: Role,
    ) -> Result<batch::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let receipt = commit_in_transaction(&tx, quote)?;
        tx.commit()?;
        Ok(receipt)
    }
}

pub(super) fn commit_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    quote: &batch::Quote,
) -> Result<batch::Receipt> {
    if quote.steps.is_empty() || quote.steps.len() > 32 {
        return Err(error(
            "BATCH_LIMIT",
            "An advancement plan requires1to32purchases.",
        ));
    }
    let hash = batch::hash(quote)?;
    let (library_id, revision): (String, i64) =
        tx.query_row("SELECT id,revision FROM library", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    if library_id != quote.library_id {
        return Err(error(
            "STALE_PURCHASE",
            "This plan belongs to a different library.",
        ));
    }
    let mut prior = Vec::new();
    let mut ids = HashSet::new();
    for (index, step) in quote.steps.iter().enumerate() {
        if !ids.insert(&step.request.id) {
            return Err(error(
                "BATCH_ID_CONFLICT",
                "Purchase IDs in a plan must be distinct.",
            ));
        }
        let saved: Option<(String, String)> = tx
            .query_row(
                "SELECT record_id,data FROM purchase_receipts WHERE id=?1",
                [&step.request.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((record_id, data)) = saved {
            let receipt: purchases::PurchaseReceipt = serde_json::from_str(&data)?;
            validate_receipt_binding(&receipt, &step.request.id, &record_id, &library_id)?;
            let binding = batch::Binding {
                quote_hash: hash.clone(),
                index,
                count: quote.steps.len(),
            };
            let mut comparable = receipt.quote.clone();
            // Intermediate full-record hashes contain actual commit timestamps.
            // The group hash binds the reviewed deterministic simulation instead.
            comparable.record_hash.clone_from(&step.record_hash);
            if receipt.batch.as_ref() != Some(&binding)
                || comparable != *step
                || record_id != quote.record_id
            {
                return Err(error(
                    "BATCH_ID_CONFLICT",
                    "A plan purchase ID already identifies another receipt.",
                ));
            }
            prior.push(receipt);
        }
    }
    if !prior.is_empty() {
        if prior.len() != quote.steps.len() {
            return Err(error(
                "BATCH_ID_CONFLICT",
                "Some plan IDs already exist. Nothing was applied.",
            ));
        }
        // Exact retries return saved receipts even after later edits/cancellation.
        return Ok(batch::Receipt {
            quote: quote.clone(),
            receipts: prior,
        });
    }
    if revision != quote.revision {
        return Err(error(
            "STALE_PURCHASE",
            "Library changed after the plan. Review it again.",
        ));
    }
    for step in &quote.steps {
        let reserved: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE json_extract(data,'$.cancellation.quote.request.id')=?1)",
                [&step.request.id], |row| row.get(0),
            )?;
        if reserved {
            return Err(error(
                "PURCHASE_ID_RESERVED",
                "A purchase ID belongs to a saved cancellation.",
            ));
        }
    }
    let data: String = tx.query_row(
        "SELECT data FROM records WHERE id=?1",
        [&quote.record_id],
        |row| row.get(0),
    )?;
    let mut record: Record = serde_json::from_str(&data)?;
    let requests: Vec<_> = quote
        .steps
        .iter()
        .map(|step| step.request.clone())
        .collect();
    if batch::quote(&library_id, revision, &record, &requests)? != *quote {
        return Err(error(
            "PURCHASE_QUOTE_CHANGED",
            "The plan differs from the current character or approved inputs. Review it again.",
        ));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let committed_revision = revision
        .checked_add(1)
        .ok_or_else(|| error("REVISION_LIMIT", "Library revision limit reached."))?;
    let (profile, _) = super::chronicle_store::read_profile(tx)?;
    let mut receipts = Vec::with_capacity(requests.len());
    for (index, request) in requests.iter().enumerate() {
        crate::owbn_bylaws::check_purchase(&profile, &record, request)?;
        let actual = purchases::quote(&library_id, revision, &record, request)?;
        record = purchases::apply(&record, &actual, now)?;
        let mut receipt = purchases::receipt(&actual, committed_revision);
        receipt.batch = Some(batch::Binding {
            quote_hash: hash.clone(),
            index,
            count: requests.len(),
        });
        receipts.push(receipt);
    }
    advance(tx, revision)?;
    if tx.execute(
        "UPDATE records SET name=?1,data=?2 WHERE id=?3",
        params![record.name, serde_json::to_string(&record)?, record.id],
    )? != 1
    {
        return Err(error("STALE_PURCHASE", "The character no longer exists."));
    }
    for receipt in &receipts {
        tx.execute(
            "INSERT INTO purchase_receipts(id,record_id,data) VALUES(?1,?2,?3)",
            params![
                receipt.quote.request.id,
                receipt.quote.record_id,
                serde_json::to_string(receipt)?
            ],
        )?;
    }
    audit(
        tx,
        "purchase_batch_commit",
        Some(&quote.record_id),
        committed_revision,
    )?;
    Ok(batch::Receipt {
        quote: quote.clone(),
        receipts,
    })
}
