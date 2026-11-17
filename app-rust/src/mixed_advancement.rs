//! Ordered composition of existing advancement commands; all prices remain source-owned.
use crate::{model::*, purchases, trait_advancement as traits, xp};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub const FIELD: &str = "_AppMixedAdvancementReceiptsV1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", deny_unknown_fields)]
pub enum Request {
    Power(Box<purchases::PurchaseRequest>),
    Trait(Box<traits::Request>),
}
impl Request {
    pub fn id(&self) -> &str {
        match self {
            Self::Power(r) => &r.id,
            Self::Trait(r) => &r.id,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "quote", deny_unknown_fields)]
pub enum Step {
    Power(Box<purchases::PurchaseQuote>),
    Trait(Box<traits::Quote>),
}
impl Step {
    pub fn request(&self) -> Request {
        match self {
            Self::Power(q) => Request::Power(Box::new(q.request.clone())),
            Self::Trait(q) => Request::Trait(Box::new(q.request.clone())),
        }
    }
    pub fn balances(&self) -> (&str, &str) {
        match self {
            Self::Power(q) => (&q.before_unspent, &q.after_unspent),
            Self::Trait(q) => (&q.before_unspent, &q.after_unspent),
        }
    }
    pub fn label(&self) -> &str {
        match self {
            Self::Power(q) => &q.price.name,
            Self::Trait(q) => &q.label,
        }
    }
    pub fn cost(&self) -> &str {
        match self {
            Self::Power(q) => &q.price.price.total,
            Self::Trait(q) => &q.price.cost,
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
    pub steps: Vec<Step>,
    pub before_unspent: String,
    pub after_unspent: String,
    pub total_spend: String,
}
impl Quote {
    pub fn requests(&self) -> Vec<Request> {
        self.steps.iter().map(Step::request).collect()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub quote: Quote,
    pub actual_steps: Vec<Step>,
    pub committed_revision: i64,
}
pub fn quote(
    library_id: &str,
    revision: i64,
    record: &Record,
    requests: &[Request],
) -> Result<Quote> {
    if requests.is_empty()
        || requests.len() > 32
        || serde_json::to_vec(requests)?.len() > 1024 * 1024
    {
        return Err(error(
            "MIXED_PLAN_LIMIT",
            "Select one to thirty-two supported steps with bounded review evidence.",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    let mut working = record.clone();
    let mut steps = vec![];
    for r in requests {
        if !ids.insert(r.id()) {
            return Err(error(
                "MIXED_PLAN_ID",
                "Every step requires a different request ID.",
            ));
        }
        if let Request::Power(p) = r
            && p.review_id.is_some()
        {
            return Err(error(
                "MIXED_PLAN_REVIEW",
                "Mixed plans require fresh per-step evidence, not a snapshot-bound saved purchase review.",
            ));
        }
        let step = step_quote(library_id, revision, &working, r)?;
        working = step_apply(&working, &step, 0)?;
        steps.push(step);
    }
    let before = steps[0].balances().0.to_owned();
    let after = steps.last().unwrap().balances().1.to_owned();
    let spent = xp::amount(&before)?
        .checked_sub(xp::amount(&after)?)
        .ok_or_else(|| {
            error(
                "MIXED_PLAN_XP",
                "Plan arithmetic exceeded supported bounds.",
            )
        })?;
    Ok(Quote {
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        steps,
        before_unspent: before,
        after_unspent: after,
        total_spend: xp::decimal(spent),
    })
}
fn step_quote(library_id: &str, revision: i64, record: &Record, r: &Request) -> Result<Step> {
    match r {
        Request::Power(r) => Ok(Step::Power(Box::new(purchases::quote(
            library_id, revision, record, r,
        )?))),
        Request::Trait(r) => Ok(Step::Trait(Box::new(traits::quote(
            library_id, revision, record, r,
        )?))),
    }
}
fn step_apply(record: &Record, step: &Step, timestamp: u64) -> Result<Record> {
    match step {
        Step::Power(q) => purchases::apply(record, q, timestamp),
        Step::Trait(q) => traits::apply(record, q, timestamp),
    }
}
/// Verify the complete deterministic plan, then recompute real timestamp-bound steps.
pub fn apply(record: &Record, q: &Quote, timestamp: u64) -> Result<(Record, Vec<Step>)> {
    if quote(&q.library_id, q.revision, record, &q.requests())? != *q {
        return Err(error(
            "MIXED_PLAN_CHANGED",
            "The source or plan changed; review the entire plan again.",
        ));
    }
    let mut working = record.clone();
    let mut actual = vec![];
    for simulated in &q.steps {
        let step = step_quote(&q.library_id, q.revision, &working, &simulated.request())?;
        let mut comparable = step.clone();
        match (&mut comparable, simulated) {
            (Step::Power(a), Step::Power(s)) => a.record_hash.clone_from(&s.record_hash),
            (Step::Trait(a), Step::Trait(s)) => a.record_hash.clone_from(&s.record_hash),
            _ => return Err(error("MIXED_PLAN_TYPE", "Plan step type changed.")),
        }
        if comparable != *simulated {
            return Err(error(
                "MIXED_PLAN_CHANGED",
                "A recomputed step differs from its reviewed simulation.",
            ));
        }
        working = step_apply(&working, &step, timestamp)?;
        actual.push(step);
    }
    Ok((working, actual))
}
pub fn hash(q: &Quote) -> Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(q)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
pub fn validate_receipt(row: &Receipt) -> Result<()> {
    if row.quote.steps.is_empty()
        || row.quote.steps.len() > 32
        || row.actual_steps.len() != row.quote.steps.len()
        || row.quote.revision.checked_add(1) != Some(row.committed_revision)
    {
        return Err(error(
            "MIXED_PLAN_HISTORY",
            "Mixed plan receipt has inconsistent steps or revision.",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    let mut balance = row.quote.before_unspent.as_str();
    for (a, s) in row.actual_steps.iter().zip(&row.quote.steps) {
        let (library, record, revision) = match s {
            Step::Power(q) => (&q.library_id, &q.record_id, q.revision),
            Step::Trait(q) => (&q.library_id, &q.record_id, q.revision),
        };
        if library != &row.quote.library_id
            || record != &row.quote.record_id
            || revision != row.quote.revision
            || s.balances().0 != balance
            || xp::amount(s.balances().0)?.checked_sub(xp::amount(s.balances().1)?)
                != Some(xp::amount(s.cost())?)
        {
            return Err(error(
                "MIXED_PLAN_HISTORY",
                "Mixed plan receipt source or cumulative balances changed.",
            ));
        }
        balance = s.balances().1;
        let mut comparable = a.clone();
        match (&mut comparable, s) {
            (Step::Power(a), Step::Power(s)) => a.record_hash.clone_from(&s.record_hash),
            (Step::Trait(a), Step::Trait(s)) => a.record_hash.clone_from(&s.record_hash),
            _ => {
                return Err(error(
                    "MIXED_PLAN_HISTORY",
                    "Mixed plan receipt changed step type.",
                ));
            }
        }
        if comparable != *s || !ids.insert(a.request().id().to_owned()) {
            return Err(error(
                "MIXED_PLAN_HISTORY",
                "Mixed plan receipt has inconsistent or repeated requests.",
            ));
        }
    }
    if balance != row.quote.after_unspent
        || xp::amount(&row.quote.before_unspent)?.checked_sub(xp::amount(balance)?)
            != Some(xp::amount(&row.quote.total_spend)?)
    {
        return Err(error(
            "MIXED_PLAN_HISTORY",
            "Mixed plan receipt total changed.",
        ));
    }
    Ok(())
}
