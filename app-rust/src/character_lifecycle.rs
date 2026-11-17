//! Reviewed local staff decisions; no trait grants, XP changes or network authority.
use crate::{model::*, purchases, sheet, vampire_creation, xp};
use serde::{Deserialize, Serialize};
pub const FIELD: &str = "_AppCharacterLifecycleV1";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    #[default]
    Activate,
    Retire,
}
impl Action {
    pub fn status(self) -> &'static str {
        match self {
            Self::Activate => "Active",
            Self::Retire => "Retired",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub action: Action,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub review_reference: String,
    pub confirmed: bool,
    pub confirm_legacy: bool,
    pub confirm_return: bool,
    pub confirm_post_creation: bool,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            action: Action::Activate,
            day: String::new(),
            actor: String::new(),
            reason: String::new(),
            review_reference: String::new(),
            confirmed: false,
            confirm_legacy: false,
            confirm_return: false,
            confirm_post_creation: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub library_id: String,
    pub record_id: String,
    pub revision: i64,
    pub record_hash: String,
    pub before: String,
    pub after: String,
    pub request: Request,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub quote: Quote,
    pub committed_revision: i64,
}
#[derive(Clone, Debug)]
pub struct Context {
    pub revision: i64,
    pub name: String,
    pub status: String,
    pub created_in_app: bool,
    pub has_xp_opening: bool,
    pub history: Vec<Receipt>,
    pub status_drift: bool,
}
pub fn status(record: &Record) -> &str {
    record
        .private_fields
        .get("Status")
        .or_else(|| record.public_fields.get("Status"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
}
pub fn scope(record: &Record) -> Result<()> {
    if !crate::sheet::is_character_kind(&record.kind) || record.build.is_some() {
        return Err(error(
            "LIFECYCLE_SCOPE",
            "Choose an original Vampire; build copies cannot make lifecycle decisions.",
        ));
    }
    if record
        .private_fields
        .get("Status")
        .or_else(|| record.public_fields.get("Status"))
        .is_some_and(|value| !value.is_string())
    {
        return Err(error(
            "LIFECYCLE_STATUS",
            "Preserve this nonstandard legacy status for review; it cannot be replaced by this workflow.",
        ));
    }
    Ok(())
}
pub fn history(record: &Record) -> Result<Vec<Receipt>> {
    if record.public_fields.contains_key(FIELD) {
        return Err(error(
            "LIFECYCLE_PRIVACY",
            "Lifecycle decision history must remain private.",
        ));
    }
    let Some(value) = record.private_fields.get(FIELD) else {
        return Ok(vec![]);
    };
    if serde_json::to_vec(value)?.len() > 8 * 1024 * 1024 {
        return Err(error(
            "LIFECYCLE_LIMIT",
            "Lifecycle history exceeds its supported limit.",
        ));
    }
    let entries: Vec<Receipt> = serde_json::from_value(value.clone())?;
    let mut ids = std::collections::HashSet::new();
    if entries.len() > 1024
        || entries.iter().any(|r| {
            r.quote.record_id != record.id
                || !ids.insert(&r.quote.request.id)
                || r.quote.revision.checked_add(1) != Some(r.committed_revision)
                || r.quote.after != r.quote.request.action.status()
        })
    {
        return Err(error(
            "LIFECYCLE_HISTORY",
            "Lifecycle decision identities or revisions are inconsistent.",
        ));
    }
    Ok(entries)
}
pub fn quote(library_id: &str, revision: i64, record: &Record, request: &Request) -> Result<Quote> {
    scope(record)?;
    history(record)?;
    if uuid::Uuid::parse_str(&request.id)
        .ok()
        .is_none_or(|u| u.to_string() != request.id)
        || !xp::day_valid(&request.day)
        || !request.confirmed
        || [&request.actor, &request.reason, &request.review_reference]
            .iter()
            .any(|v| v.trim().is_empty() || v.len() > 16000)
    {
        return Err(error(
            "LIFECYCLE_REVIEW",
            "Record a valid date, staff name, reason, review reference and explicit confirmation.",
        ));
    }
    let before = status(record);
    if before.eq_ignore_ascii_case(request.action.status()) {
        return Err(error(
            "LIFECYCLE_NO_CHANGE",
            "The character already has the selected status.",
        ));
    }
    if request.action == Action::Activate {
        if before.eq_ignore_ascii_case("Retired") && !request.confirm_return {
            return Err(error(
                "LIFECYCLE_RETURN",
                "Explicitly review returning this retired character to active play.",
            ));
        }
        if vampire_creation::draft(record)?.is_some() {
            return Err(error(
                "CREATION_UNFINISHED",
                "Finish or explicitly discard the saved creation draft before entering play.",
            ));
        }
        let ledger = record.xp_ledger.as_ref().ok_or_else(|| {
            error(
                "LIFECYCLE_XP",
                "Explicitly review and open XP before entering play.",
            )
        })?;
        xp::replay(ledger)?;
        if record.original["created_in_app"] == true {
            let creation = vampire_creation::history(record)?;
            let receipt = creation.last().ok_or_else(|| {
                error(
                    "LIFECYCLE_CREATION",
                    "Complete the initial creation review before entering play.",
                )
            })?;
            if receipt.managed_signature
                != vampire_creation::signature_for_quote(&receipt.quote, &sheet::extract(record))?
            {
                let has_spend = ledger
                    .entries
                    .iter()
                    .any(|e| e.command.kind == xp::XpKind::Spend);
                if !has_spend || !request.confirm_post_creation {
                    return Err(error(
                        "LIFECYCLE_CREATION_CHANGED",
                        "Initial traits changed since creation. Review later advancement and its XP evidence explicitly; an unchanged initial sheet is required before any XP spending.",
                    ));
                }
            }
        } else if !request.confirm_legacy {
            return Err(error(
                "LIFECYCLE_LEGACY",
                "Review this imported character's eligibility explicitly; the app does not reconstruct its original creation approval.",
            ));
        }
        if !matches!(
            before.to_ascii_lowercase().as_str(),
            "draft" | "inactive" | "retired"
        ) && !request.confirm_legacy
        {
            return Err(error(
                "LIFECYCLE_CUSTOM_STATUS",
                "Review the existing custom status explicitly before replacing it with Active.",
            ));
        }
    }
    Ok(Quote {
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        before: before.into(),
        after: request.action.status().into(),
        request: request.clone(),
    })
}
