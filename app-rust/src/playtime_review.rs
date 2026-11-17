//! Private, library-local saved choices for one monthly award review.

use crate::{
    model::{Result, error},
    playtime::{self, AwardReview, Request},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

pub const FIELD: &str = "_AppPlaytimeAwardReviewsV1";
const HISTORY_LIMIT: usize = 128;
const HISTORY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedReview {
    pub id: String,
    pub library_id: String,
    pub record_id: String,
    pub start_month: String,
    pub end_month: String,
    pub award_reviews: Vec<AwardReview>,
    pub saved_revision: i64,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedReview {
    pub saved: SavedReview,
    pub input: Request,
    pub stale_reviews: usize,
    pub unclassified_awards: usize,
}

fn invalid_history() -> crate::model::AppError {
    error(
        "PLAYTIME_REVIEW_INVALID",
        "Saved private award review needs inspection; no changes were made.",
    )
}

pub(crate) fn canonical_reviews(rows: &[AwardReview]) -> Vec<AwardReview> {
    let mut rows = rows.to_vec();
    rows.sort_by(|left, right| {
        left.entry_id
            .cmp(&right.entry_id)
            .then_with(|| left.entry_hash.cmp(&right.entry_hash))
    });
    rows
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}

pub(crate) fn content_hash(review: &SavedReview) -> Result<String> {
    crate::legacy_profile::hash(&(
        "playtime-award-review-v1",
        review.id.as_str(),
        review.library_id.as_str(),
        review.record_id.as_str(),
        review.start_month.as_str(),
        review.end_month.as_str(),
        review.award_reviews.as_slice(),
        review.saved_revision,
    ))
}

fn validate_review(review: &SavedReview) -> Result<()> {
    if !valid_id(&review.id)
        || review.library_id.trim().is_empty()
        || review.library_id.len() > 128
        || review.record_id.trim().is_empty()
        || review.record_id.len() > 256
        || review.saved_revision < 1
        || review.award_reviews.is_empty()
        || review.award_reviews.len() > 10000
        || canonical_reviews(&review.award_reviews) != review.award_reviews
    {
        return Err(invalid_history());
    }
    playtime::validate_month_range(&review.start_month, &review.end_month)
        .map_err(|_| invalid_history())?;
    let mut ids = HashSet::new();
    if review.award_reviews.iter().any(|row| {
        row.entry_id.trim().is_empty()
            || row.entry_id.len() > 256
            || row.entry_hash.len() != 64
            || !row
                .entry_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || row.reason.trim().is_empty()
            || row.reference.trim().is_empty()
            || row.reason.len() > 16000
            || row.reference.len() > 16000
            || !ids.insert(row.entry_id.as_str())
    }) {
        return Err(invalid_history());
    }
    if review.content_hash.len() != 64 || review.content_hash != content_hash(review)? {
        return Err(invalid_history());
    }
    Ok(())
}

pub(crate) fn history(raw: Option<&Value>) -> Result<Vec<SavedReview>> {
    let Some(value) = raw.and_then(|value| value.get(FIELD)) else {
        return Ok(Vec::new());
    };
    if serde_json::to_vec(value)?.len() > HISTORY_BYTES {
        return Err(error(
            "PLAYTIME_REVIEW_LIMIT",
            "Saved private award review history exceeds its size limit.",
        ));
    }
    let rows: Vec<SavedReview> =
        serde_json::from_value(value.clone()).map_err(|_| invalid_history())?;
    if rows.len() > HISTORY_LIMIT {
        return Err(error(
            "PLAYTIME_REVIEW_LIMIT",
            "Saved private award review history exceeds its count limit.",
        ));
    }
    let mut ids = HashSet::new();
    for row in &rows {
        validate_review(row)?;
        if !ids.insert(row.id.as_str()) {
            return Err(invalid_history());
        }
    }
    Ok(rows)
}

pub(crate) fn validate_library(raw: Option<&Value>, library_id: &str) -> Result<Vec<SavedReview>> {
    let rows = history(raw)?;
    if rows.iter().any(|row| row.library_id != library_id) {
        return Err(error(
            "PLAYTIME_REVIEW_LIBRARY",
            "Saved private award review history belongs to another library.",
        ));
    }
    Ok(rows)
}

pub(crate) fn request_for(
    review: &SavedReview,
    current: &[playtime::AwardSnapshot],
) -> LoadedReview {
    let hashes = current
        .iter()
        .map(|row| (row.entry_id.as_str(), row.entry_hash.as_str()))
        .collect::<std::collections::HashMap<_, _>>();
    let matched = review
        .award_reviews
        .iter()
        .filter(|saved| hashes.get(saved.entry_id.as_str()) == Some(&saved.entry_hash.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let stale_reviews = review.award_reviews.len() - matched.len();
    let unclassified_awards = current.len() - matched.len();
    LoadedReview {
        saved: review.clone(),
        input: Request {
            start_month: review.start_month.clone(),
            end_month: review.end_month.clone(),
            award_reviews: matched,
            ..Request::default()
        },
        stale_reviews,
        unclassified_awards,
    }
}
