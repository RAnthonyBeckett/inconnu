//! Explicitly reviewed date-only plans from retained legacy calendar evidence.
use crate::{
    game_sessions::{self, SessionRequest},
    model::{Result, error},
    xp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;
pub const FIELD: &str = "_AppLegacyCalendarReviewsV1";
const SOURCE_LIMIT: usize = 4096;
const BATCH_LIMIT: usize = 64;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub index: usize,
    pub raw_date: String,
    pub time: String,
    pub place: String,
    pub notes: String,
    pub proposed_day: Option<String>,
    pub issue: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub entry_index: usize,
    pub day: String,
    pub title: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub source_hash: String,
    pub choices: Vec<Choice>,
    pub review_day: String,
    pub actor: String,
    pub reason: String,
    pub review_reference: String,
    pub confirmed: bool,
    pub confirm_past: bool,
    pub confirm_overlap: bool,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            source_hash: String::new(),
            choices: vec![],
            review_day: String::new(),
            actor: String::new(),
            reason: String::new(),
            review_reference: String::new(),
            confirmed: false,
            confirm_past: false,
            confirm_overlap: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub entry_index: usize,
    pub request: SessionRequest,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub source_name: String,
    pub entry: Entry,
    pub imported_day: String,
    pub imported_title: String,
}
#[derive(Clone, Debug)]
pub struct Source {
    pub hash: String,
    pub name: String,
    pub entries: usize,
    pub issue: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Context {
    pub library_id: String,
    pub revision: i64,
    pub source_hash: String,
    pub source_name: String,
    pub source_metadata_hash: String,
    pub profile_hash: String,
    pub entries: Vec<Entry>,
    pub existing_days: Vec<String>,
    pub history: Vec<Receipt>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub library_id: String,
    pub revision: i64,
    pub source_name: String,
    pub source_metadata_hash: String,
    pub profile_hash: String,
    pub request_hash: String,
    pub request: Request,
    pub plans: Vec<Plan>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub library_id: String,
    pub request: Request,
    pub source_name: String,
    pub source_metadata_hash: String,
    pub quote_hash: String,
    pub plans: Vec<Plan>,
    pub committed_revision: i64,
}
fn source_error() -> crate::model::AppError {
    error(
        "LEGACY_CALENDAR_SOURCE",
        "Retained calendar metadata is unsupported or ambiguous; original evidence is preserved.",
    )
}
fn history_error() -> crate::model::AppError {
    error(
        "LEGACY_CALENDAR_HISTORY",
        "Private calendar review history is malformed. Restore a trusted backup.",
    )
}
fn text(v: Option<&Value>) -> String {
    let s = v.and_then(Value::as_str).unwrap_or_default();
    if s.chars().count() > 4096 {
        format!(
            "{} [... original retained]",
            s.chars().take(4096).collect::<String>()
        )
    } else {
        s.into()
    }
}
fn iso(y: i64, m: i64, d: i64) -> Option<String> {
    let day = format!("{y:04}-{m:02}-{d:02}");
    xp::day_valid(&day).then_some(day)
}
fn oa_day(value: f64) -> Option<String> {
    if !value.is_finite() || value.fract() != 0.0 || !(-657434.0..=2958465.0).contains(&value) {
        return None;
    }
    // Gregorian civil day from the integral OA day. Fractional times need manual review.
    let z = value as i64 - 25569 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    if m <= 2 {
        y += 1;
    }
    iso(y, m, d)
}
fn propose(raw: &Value) -> Option<String> {
    if let Some(object) = raw.as_object() {
        return (object.len() == 1)
            .then(|| {
                object
                    .get("ole_automation_days")
                    .and_then(Value::as_f64)
                    .and_then(oa_day)
            })
            .flatten();
    }
    let raw = raw.as_str()?.trim();
    if raw.len() > 128 {
        return None;
    }
    if xp::day_valid(raw) {
        return Some(raw.into());
    }
    let parts = raw.split('/').collect::<Vec<_>>();
    if parts.len() != 3
        || parts[2].len() != 4
        || parts[..2].iter().any(|s| s.is_empty() || s.len() > 2)
        || parts.iter().any(|s| !s.bytes().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let a = parts[0].parse::<i64>().ok()?;
    let b = parts[1].parse::<i64>().ok()?;
    let y = parts[2].parse::<i64>().ok()?;
    match (iso(y, a, b), iso(y, b, a)) {
        (Some(a), Some(b)) if a == b => Some(a),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        _ => None,
    }
}
fn attribute<'a>(node: &'a Value, name: &str) -> Result<Option<&'a Value>> {
    let attrs = node
        .get("attributes")
        .and_then(Value::as_array)
        .ok_or_else(source_error)?;
    let matches = attrs
        .iter()
        .filter(|a| a.get("name").and_then(Value::as_str) == Some(name))
        .collect::<Vec<_>>();
    if matches.len() > 1
        || matches
            .iter()
            .any(|a| !a.get("namespace").unwrap_or(&Value::Null).is_null())
    {
        return Err(source_error());
    }
    Ok(matches.first().and_then(|a| a.get("value")))
}
fn flat(node: Option<&Value>) -> String {
    let Some(node) = node else {
        return String::new();
    };
    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return "[Unsupported retained text]".into();
    };
    let full = children
        .iter()
        .filter_map(|child| child.get("text").and_then(Value::as_str))
        .collect::<String>();
    text(Some(&Value::String(full)))
}
pub fn candidates(report: &Value) -> Result<(Vec<Entry>, String)> {
    let meta = report.get("metadata").unwrap_or(&Value::Null);
    let binary = meta.get("Calendar");
    let xml = meta.get("calendar");
    if binary.is_some() && xml.is_some() {
        return Err(source_error());
    }
    if let Some(raw) = report.get("raw_xml").and_then(Value::as_str) {
        let doc = roxmltree::Document::parse(raw).map_err(|_| source_error())?;
        if doc
            .root_element()
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "calendar")
            .count()
            > 1
        {
            return Err(source_error());
        }
    }
    let mut entries = vec![];
    let mut push = |raw: Value, time: String, place: String, notes: String| {
        let proposed_day = propose(&raw);
        let issue=proposed_day.is_none().then(||"No unambiguous date proposal. Enter the exact reviewed ISO day; no locale, century or fractional time is assumed.".into());
        let raw_date = raw
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| raw.to_string());
        entries.push(Entry {
            index: entries.len(),
            raw_date,
            time,
            place,
            notes,
            proposed_day,
            issue,
        });
    };
    if let Some(calendar) = binary {
        let rows = calendar
            .get("Entries")
            .and_then(Value::as_array)
            .ok_or_else(source_error)?;
        if rows.len() > SOURCE_LIMIT {
            return Err(source_error());
        }
        for row in rows {
            if !row.is_object() {
                return Err(source_error());
            }
            push(
                row.get("GameDate").cloned().unwrap_or(Value::Null),
                text(row.get("Time")),
                text(row.get("Place")),
                text(row.get("Notes")),
            );
        }
    } else if let Some(calendar) = xml {
        if calendar.get("tag").and_then(Value::as_str) != Some("calendar")
            || !calendar.get("namespace").unwrap_or(&Value::Null).is_null()
        {
            return Err(source_error());
        }
        let rows = calendar
            .get("children")
            .and_then(Value::as_array)
            .ok_or_else(source_error)?;
        if rows.len() > SOURCE_LIMIT * 3 {
            return Err(source_error());
        }
        for row in rows {
            if row.get("tag").and_then(Value::as_str) != Some("game") {
                continue;
            }
            if !row.get("namespace").unwrap_or(&Value::Null).is_null() {
                return Err(source_error());
            }
            let raw = attribute(row, "date")?.cloned().unwrap_or(Value::Null);
            let time = text(attribute(row, "time")?);
            let children = row
                .get("children")
                .and_then(Value::as_array)
                .ok_or_else(source_error)?;
            let named = |name: &str| -> Result<Option<&Value>> {
                let found = children
                    .iter()
                    .filter(|v| v.get("tag").and_then(Value::as_str) == Some(name))
                    .collect::<Vec<_>>();
                if found.len() > 1 {
                    return Err(source_error());
                }
                Ok(found.first().copied())
            };
            push(raw, time, flat(named("place")?), flat(named("notes")?));
        }
    }
    if entries.len() > SOURCE_LIMIT {
        return Err(source_error());
    }
    Ok((entries, crate::legacy_profile::hash(meta)?))
}
fn validate_request(r: &Request) -> Result<()> {
    if Uuid::parse_str(&r.id)
        .ok()
        .is_none_or(|u| u.to_string() != r.id)
        || r.source_hash.len() != 64
        || !r.source_hash.bytes().all(|c| c.is_ascii_hexdigit())
        || !xp::day_valid(&r.review_day)
        || !r.confirmed
        || r.choices.is_empty()
        || r.choices.len() > BATCH_LIMIT
        || [&r.actor, &r.reason, &r.review_reference]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 16000)
    {
        return Err(error(
            "LEGACY_CALENDAR_REVIEW",
            "Select bounded source entries and record exact ISO days, review date, staff name, reason, reference and confirmation.",
        ));
    }
    let mut seen = HashSet::new();
    for c in &r.choices {
        if c.entry_index >= SOURCE_LIMIT
            || !seen.insert(c.entry_index)
            || !xp::day_valid(&c.day)
            || c.title.trim().is_empty()
            || c.title.len() > 512
            || c.title.contains('\0')
        {
            return Err(error(
                "LEGACY_CALENDAR_CHOICE",
                "Each source entry needs one valid ISO day and neutral plan title.",
            ));
        }
        if c.day < r.review_day && !r.confirm_past {
            return Err(error(
                "LEGACY_CALENDAR_PAST",
                "Explicitly review past dates: a plan does not establish attendance or completion.",
            ));
        }
    }
    Ok(())
}
fn plan(library_id: &str, r: &Request, c: &Choice) -> Plan {
    let id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "grapevine-calendar:{library_id}:{}:{}",
            r.source_hash, c.entry_index
        )
        .as_bytes(),
    )
    .to_string();
    Plan {
        entry_index: c.entry_index,
        request: SessionRequest {
            id,
            title: c.title.clone(),
            day: c.day.clone(),
            actor: r.actor.clone(),
            reason: r.reason.clone(),
            evidence: format!(
                "Retained calendar source {}; entry {}; review {}",
                r.source_hash, c.entry_index, r.review_reference
            ),
            attendance: vec![],
        },
    }
}
pub fn quote(c: &Context, r: &Request) -> Result<Quote> {
    validate_request(r)?;
    if r.source_hash != c.source_hash {
        return Err(error(
            "LEGACY_CALENDAR_SOURCE_CHANGED",
            "Choose the exact retained calendar source again.",
        ));
    }
    let mut days = c.existing_days.clone();
    let mut plans = vec![];
    for choice in &r.choices {
        if !c.entries.iter().any(|e| e.index == choice.entry_index) {
            return Err(error(
                "LEGACY_CALENDAR_CHOICE",
                "A selected source entry no longer exists.",
            ));
        }
        if c.history.iter().any(|h| {
            h.request.source_hash == r.source_hash
                && h.plans.iter().any(|p| p.entry_index == choice.entry_index)
        }) {
            return Err(error(
                "LEGACY_CALENDAR_IMPORTED",
                "This source entry was already imported. Existing, cancelled or completed plans are not recreated.",
            ));
        }
        if days.contains(&choice.day) && !r.confirm_overlap {
            return Err(error(
                "LEGACY_CALENDAR_OVERLAP",
                "Review dates shared with another plan or selected source entry. Entries are never merged by date.",
            ));
        }
        days.push(choice.day.clone());
        let p = plan(&c.library_id, r, choice);
        game_sessions::validate_header(&c.library_id, c.revision, &p.request)?;
        plans.push(p);
    }
    Ok(Quote {
        library_id: c.library_id.clone(),
        revision: c.revision,
        source_name: c.source_name.clone(),
        source_metadata_hash: c.source_metadata_hash.clone(),
        profile_hash: c.profile_hash.clone(),
        request_hash: crate::legacy_profile::hash(r)?,
        request: r.clone(),
        plans,
    })
}
pub fn history(raw: Option<&Value>) -> Result<Vec<Receipt>> {
    let Some(value) = raw.and_then(|v| v.get(FIELD)) else {
        return Ok(vec![]);
    };
    if serde_json::to_vec(value)?.len() > 2 * 1024 * 1024 {
        return Err(history_error());
    }
    let rows: Vec<Receipt> = serde_json::from_value(value.clone()).map_err(|_| history_error())?;
    if rows.len() > 128 {
        return Err(history_error());
    }
    let mut ids = HashSet::new();
    let mut entries = HashSet::new();
    for r in &rows {
        validate_request(&r.request).map_err(|_| history_error())?;
        if r.library_id.is_empty()
            || r.committed_revision < 1
            || r.quote_hash.len() != 64
            || !r.quote_hash.bytes().all(|c| c.is_ascii_hexdigit())
            || r.source_metadata_hash.len() != 64
            || !r
                .source_metadata_hash
                .bytes()
                .all(|c| c.is_ascii_hexdigit())
            || !ids.insert(&r.request.id)
            || r.plans.len() != r.request.choices.len()
        {
            return Err(history_error());
        }
        for (choice, p) in r.request.choices.iter().zip(&r.plans) {
            game_sessions::validate_header(&r.library_id, r.committed_revision, &p.request)
                .map_err(|_| history_error())?;
            if *p != plan(&r.library_id, &r.request, choice)
                || !entries.insert((&r.request.source_hash, p.entry_index))
            {
                return Err(history_error());
            }
        }
    }
    Ok(rows)
}
