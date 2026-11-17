use super::*;
use crate::{advancement_draft as draft, purchase_batch, purchases};

impl Library {
    pub fn save_advancement_draft(
        &mut self,
        source_id: &str,
        requests: &[purchases::PurchaseRequest],
        expected: i64,
        role: Role,
    ) -> Result<String> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; review the plan before saving a draft.",
            ));
        }
        let source = transaction_record(&tx, source_id)?;
        let quote = purchase_batch::quote(&library_id, revision, &source, requests)?;
        let mut record = source.clone();
        record.id = Uuid::new_v4().to_string();
        record.build = Some(BuildInfo {
            source_id: source.id.clone(),
            source_revision: revision,
            kind: BuildKind::Draft,
        });
        record.name = format!("{} — advancement draft", source.name);
        let name = record.name.clone();
        set_sheet_field(&mut record, "Name", json!(name));
        record.private_fields.remove("Status");
        record.private_fields.remove(crate::associations::FIELD);
        record.private_fields.remove(crate::boon_book::FIELD);
        record
            .private_fields
            .remove(crate::published_reference::FIELD);
        record.private_fields.remove(crate::met_references::FIELD);
        record
            .private_fields
            .remove(crate::mixed_advancement::FIELD);
        record
            .private_fields
            .remove(crate::character_lifecycle::FIELD);
        record.private_fields.remove(crate::vampire_creation::FIELD);
        record
            .private_fields
            .remove(crate::vampire_creation::DRAFT_FIELD);
        record
            .public_fields
            .insert("Status".into(), json!("Draft — private advancement plan"));
        record.private_fields.remove(draft::FIELD);
        let metadata = draft::make(&source, &record, &quote)?;
        record
            .private_fields
            .insert(draft::FIELD.into(), serde_json::to_value(metadata)?);
        advance(&tx, revision)?;
        insert(&tx, &record)?;
        audit(
            &tx,
            "advancement_draft_save",
            Some(&record.id),
            revision + 1,
        )?;
        tx.commit()?;
        Ok(record.id)
    }

    pub fn review_advancement_draft(
        &self,
        id: &str,
        expected: i64,
        role: Role,
    ) -> Result<draft::Review> {
        require_st(role)?;
        // One SELECT binds the live source and draft to the same library revision.
        let snapshot:Option<(String,i64,String,String)>=self.conn.query_row(
            "SELECT library.id,library.revision,d.data,s.data FROM library JOIN records d ON d.id=?1 JOIN records s ON s.id=json_extract(d.data,'$.build.source_id')",
            [id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
        ).optional()?;
        let (library_id, revision, draft_data, source_data) = snapshot.ok_or_else(|| {
            error(
                "DRAFT_SOURCE_MISSING",
                "The saved draft or its live source is missing.",
            )
        })?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; prepare a new draft review.",
            ));
        }
        draft::review(
            &library_id,
            revision,
            &serde_json::from_str(&source_data)?,
            &serde_json::from_str(&draft_data)?,
        )
    }

    pub fn apply_advancement_draft(
        &mut self,
        review: &draft::Review,
        role: Role,
    ) -> Result<purchase_batch::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        let mut record = transaction_record(&tx, &review.draft_id)?;
        let mut metadata = draft::metadata(&record)?;
        if metadata.source_id != review.quote.record_id
            || metadata.draft_hash != review.draft_hash
            || metadata.source_hash != review.source_hash
            || review.quote.library_id != library_id
        {
            return Err(error(
                "DRAFT_REVIEW_CHANGED",
                "The draft review no longer matches its saved plan.",
            ));
        }
        if let Some(applied) = &metadata.applied {
            if applied != &review.quote {
                return Err(error(
                    "DRAFT_ALREADY_APPLIED",
                    "This draft already records a different applied plan.",
                ));
            }
            return super::purchase_batch_store::commit_in_transaction(&tx, &review.quote);
        }
        let source = transaction_record(&tx, &metadata.source_id)?;
        if draft::review(&library_id, revision, &source, &record)? != *review {
            return Err(error(
                "DRAFT_REVIEW_CHANGED",
                "The character or review changed. Review the saved draft again.",
            ));
        }
        let receipt = super::purchase_batch_store::commit_in_transaction(&tx, &review.quote)?;
        metadata.applied = Some(review.quote.clone());
        record
            .private_fields
            .insert(draft::FIELD.into(), serde_json::to_value(metadata)?);
        draft::metadata(&record)?;
        if tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, record.id],
        )? != 1
        {
            return Err(error(
                "DRAFT_SOURCE_MISSING",
                "The saved draft no longer exists.",
            ));
        }
        audit(
            &tx,
            "advancement_draft_apply",
            Some(&record.id),
            revision + 1,
        )?;
        tx.commit()?;
        Ok(receipt)
    }
}

fn transaction_record(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<Record> {
    let data: Option<String> = tx
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    serde_json::from_str(&data.ok_or_else(|| {
        error(
            "DRAFT_SOURCE_MISSING",
            "The saved draft or live character no longer exists.",
        )
    })?)
    .map_err(Into::into)
}
