use super::*;
use crate::playtime_review::{self as saved_review, LoadedReview, SavedReview};

impl Library {
    pub fn save_playtime_award_review(
        &mut self,
        record_id: &str,
        review_id: &str,
        input: &crate::playtime::Request,
        expected: i64,
        role: Role,
    ) -> Result<SavedReview> {
        require_st(role)?;
        if !saved_review::valid_id(review_id) {
            return Err(error(
                "PLAYTIME_REVIEW_ID",
                "Enter a valid private award review ID.",
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        let (profile, raw) = super::chronicle_store::read_profile(&tx)?;
        let mut history = saved_review::validate_library(raw.as_ref(), &library_id)?;
        let canonical = saved_review::canonical_reviews(&input.award_reviews);
        if let Some(existing) = history.iter().find(|row| row.id == review_id) {
            if existing.library_id == library_id
                && existing.record_id == record_id
                && existing.start_month == input.start_month
                && existing.end_month == input.end_month
                && existing.award_reviews == canonical
            {
                let existing = existing.clone();
                tx.commit()?;
                return Ok(existing);
            }
            return Err(error(
                "PLAYTIME_REVIEW_ID_CONFLICT",
                "This private award review ID already identifies different saved choices.",
            ));
        }
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; reload the monthly award review before saving.",
            ));
        }
        if input.award_reviews.is_empty() {
            return Err(error(
                "PLAYTIME_REVIEW_EMPTY",
                "Save at least one explicit award classification.",
            ));
        }
        if history.len() >= 128 {
            return Err(error(
                "PLAYTIME_REVIEW_LIMIT",
                "Private award review history is full; no saved review was changed.",
            ));
        }
        let record = read_record(&tx, record_id)?;
        let sanitized = crate::playtime::Request {
            start_month: input.start_month.clone(),
            end_month: input.end_month.clone(),
            award_reviews: canonical.clone(),
            ..crate::playtime::Request::default()
        };
        crate::playtime::assess(&record, &sanitized)?;
        let saved_revision = expected.checked_add(1).ok_or_else(|| {
            error(
                "REVISION_LIMIT",
                "The library revision cannot be advanced further.",
            )
        })?;
        let mut saved = SavedReview {
            id: review_id.into(),
            library_id,
            record_id: record_id.into(),
            start_month: input.start_month.clone(),
            end_month: input.end_month.clone(),
            award_reviews: canonical,
            saved_revision,
            content_hash: String::new(),
        };
        saved.content_hash = saved_review::content_hash(&saved)?;
        history.push(saved.clone());

        let mut data: Value =
            serde_json::from_str(&super::chronicle_store::merged_data(&profile, raw)?)?;
        data[saved_review::FIELD] = serde_json::to_value(&history)?;
        saved_review::validate_library(Some(&data), &saved.library_id)?;
        super::advance(&tx, expected)?;
        tx.execute(
            "INSERT INTO chronicle_profile(singleton,data) VALUES(1,?1)
             ON CONFLICT(singleton) DO UPDATE SET data=excluded.data",
            [serde_json::to_string(&data)?],
        )?;
        super::audit(
            &tx,
            "playtime_award_review_save",
            Some(record_id),
            saved_revision,
        )?;
        tx.commit()?;
        Ok(saved)
    }

    pub fn load_playtime_award_review(
        &self,
        record_id: &str,
        review_id: &str,
        expected: i64,
        role: Role,
    ) -> Result<LoadedReview> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; reload the monthly award review.",
            ));
        }
        let record = read_record(&tx, record_id)?;
        let (_, raw) = super::chronicle_store::read_profile(&tx)?;
        let history = saved_review::validate_library(raw.as_ref(), &library_id)?;
        let saved = history
            .iter()
            .find(|row| row.id == review_id && row.record_id == record_id)
            .ok_or_else(|| {
                error(
                    "PLAYTIME_REVIEW_NOT_FOUND",
                    "Choose a saved private review for this character.",
                )
            })?;
        let current = crate::playtime::awards(&record, &saved.start_month, &saved.end_month)?;
        let loaded = saved_review::request_for(saved, &current);
        let final_revision: i64 =
            tx.query_row("SELECT revision FROM library", [], |row| row.get(0))?;
        if final_revision != revision {
            return Err(error(
                "STALE_REVISION",
                "Library changed while the saved monthly review was loading.",
            ));
        }
        tx.commit()?;
        Ok(loaded)
    }

    pub fn playtime_award_reviews(&self, record_id: &str, role: Role) -> Result<Vec<SavedReview>> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        read_record(&tx, record_id)?;
        let (_, raw) = super::chronicle_store::read_profile(&tx)?;
        let history = saved_review::validate_library(raw.as_ref(), &library_id)?;
        let result = history
            .into_iter()
            .filter(|row| row.record_id == record_id)
            .collect();
        let final_revision: i64 =
            tx.query_row("SELECT revision FROM library", [], |row| row.get(0))?;
        if final_revision != revision {
            return Err(error(
                "STALE_REVISION",
                "Library changed while saved monthly reviews were loading.",
            ));
        }
        tx.commit()?;
        Ok(result)
    }
}

fn read_record(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<Record> {
    let data: Option<String> = tx
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    serde_json::from_str(&data.ok_or_else(|| error("NOT_FOUND", "Character was not found."))?)
        .map_err(Into::into)
}
