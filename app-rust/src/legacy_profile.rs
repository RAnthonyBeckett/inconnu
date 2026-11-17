//! Explicit, publication-aware reconciliation of retained legacy game details.
use crate::{chronicle::Profile, importer, model::*, xp};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
pub const FIELD: &str = "_AppLegacyChronicleReviewsV1";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Field {
    Title,
    Website,
    Email,
    Phone,
    MeetingPlace,
    MeetingTime,
    Description,
}
impl Field {
    pub const ALL: [Self; 7] = [
        Self::Title,
        Self::Website,
        Self::Email,
        Self::Phone,
        Self::MeetingPlace,
        Self::MeetingTime,
        Self::Description,
    ];
    pub fn keys(self) -> (&'static str, &'static str) {
        match self {
            Self::Title => ("ChronicleTitle", "chronicle"),
            Self::Website => ("Website", "website"),
            Self::Email => ("EMail", "email"),
            Self::Phone => ("Phone", "phone"),
            Self::MeetingPlace => ("UsualPlace", "usualplace"),
            Self::MeetingTime => ("UsualTime", "usualtime"),
            Self::Description => ("Description", "description"),
        }
    }
    pub fn get(self, p: &Profile) -> &str {
        match self {
            Self::Title => &p.title,
            Self::Website => &p.website,
            Self::Email => &p.email,
            Self::Phone => &p.phone,
            Self::MeetingPlace => &p.meeting_place,
            Self::MeetingTime => &p.meeting_time,
            Self::Description => &p.description,
        }
    }
    pub fn set(self, p: &mut Profile, s: String) {
        match self {
            Self::Title => p.title = s,
            Self::Website => p.website = s,
            Self::Email => p.email = s,
            Self::Phone => p.phone = s,
            Self::MeetingPlace => p.meeting_place = s,
            Self::MeetingTime => p.meeting_time = s,
            Self::Description => p.description = s,
        }
    }
    pub fn public(self, p: &Profile) -> bool {
        match self {
            Self::Title => true,
            Self::Email | Self::Phone => p.publish_contacts,
            _ => p.publish_details,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Candidate {
    pub field: Field,
    pub current: String,
    pub proposed: Option<String>,
    pub private_text_removed: bool,
    pub would_be_public: bool,
}
#[derive(Clone, Debug)]
pub struct Source {
    pub hash: String,
    pub name: String,
    pub available_fields: usize,
    pub issue: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Context {
    pub library_id: String,
    pub revision: i64,
    pub source_hash: String,
    pub source_name: String,
    pub source_metadata_hash: String,
    pub profile_before_hash: String,
    pub profile: Profile,
    pub fields: Vec<Candidate>,
    pub history: Vec<Receipt>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub source_hash: String,
    pub selected: Vec<Field>,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub review_reference: String,
    pub confirmed: bool,
    pub confirm_overwrite: bool,
    pub confirm_publication: bool,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_hash: String::new(),
            selected: vec![],
            day: String::new(),
            actor: String::new(),
            reason: String::new(),
            review_reference: String::new(),
            confirmed: false,
            confirm_overwrite: false,
            confirm_publication: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub field: Field,
    pub before: String,
    pub after: String,
    pub would_be_public: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub request_hash: String,
    pub library_id: String,
    pub revision: i64,
    pub source_name: String,
    pub source_metadata_hash: String,
    pub profile_before_hash: String,
    pub request: Request,
    pub changes: Vec<Change>,
    pub proposed: Profile,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub request: Request,
    pub source_name: String,
    pub source_metadata_hash: String,
    pub profile_before_hash: String,
    pub quote_hash: String,
    pub changes: Vec<Change>,
    pub committed_revision: i64,
}
pub fn hash<T: Serialize>(value: &T) -> Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(value)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
pub fn candidates(report: &Value, current: &Profile) -> Result<(Vec<Candidate>, String)> {
    let meta = &report["metadata"];
    let game = meta.get("Game");
    let attrs = meta.get("XmlRootAttributes").and_then(Value::as_array);
    if game.is_some() && attrs.is_some() {
        return Err(error(
            "LEGACY_PROFILE_AMBIGUOUS",
            "Source contains competing binary and XML game metadata.",
        ));
    }
    let attr = |name: &str| -> Result<Option<String>> {
        let found: Vec<_> = attrs
            .into_iter()
            .flatten()
            .filter(|a| a["name"] == name && a["namespace"].is_null())
            .collect();
        if found.len() > 1 {
            return Err(error(
                "LEGACY_PROFILE_AMBIGUOUS",
                "Source contains repeated game attributes.",
            ));
        }
        Ok(found
            .first()
            .and_then(|a| a["value"].as_str())
            .map(str::to_owned))
    };
    let marker = |key: &str, xml: &str, default: &str| -> Result<String> {
        Ok(game
            .and_then(|g| g[key].as_str())
            .map(str::to_owned)
            .or(attr(xml)?)
            .unwrap_or_else(|| default.into()))
    };
    let start = marker("STCommentStart", "stcommentstart", "[ST]")?;
    let end = marker("STCommentEnd", "stcommentend", "[/ST]")?;
    let mut fields = vec![];
    for field in Field::ALL {
        let (binary, xml) = field.keys();
        let raw = if let Some(game) = game {
            game[binary].as_str().map(str::to_owned)
        } else if matches!(field, Field::MeetingPlace | Field::Description) {
            meta.get(xml)
                .and_then(|n| {
                    n["children"].as_array().filter(|_| {
                        n["namespace"].is_null()
                            && n["tag"] == xml
                            && n["attributes"].as_array().is_some_and(|a| a.is_empty())
                    })
                })
                .and_then(|children| {
                    children
                        .iter()
                        .map(|c| c.get("text").and_then(Value::as_str))
                        .collect::<Option<Vec<_>>>()
                })
                .map(|parts| parts.join(""))
        } else {
            attr(xml)?
        };
        let safe = raw.as_ref().map(|s| {
            let mut v = importer::redact(&Value::String(s.clone()));
            if !start.is_empty() && !end.is_empty() {
                v = importer::redact_custom(&v, &start, &end);
            }
            let text = v.as_str().unwrap_or_default();
            if field == Field::Title {
                text.trim().to_owned()
            } else {
                text.to_owned()
            }
        });
        let removed = raw.as_ref().zip(safe.as_ref()).is_some_and(|(a, b)| a != b);
        fields.push(Candidate {
            field,
            current: field.get(current).into(),
            proposed: safe,
            private_text_removed: removed,
            would_be_public: field.public(current),
        });
    }
    Ok((fields, hash(meta)?))
}
pub fn quote(context: &Context, request: &Request) -> Result<Quote> {
    if request.source_hash != context.source_hash
        || uuid::Uuid::parse_str(&request.id)
            .ok()
            .is_none_or(|u| u.to_string() != request.id)
        || !xp::day_valid(&request.day)
        || !request.confirmed
        || request.selected.is_empty()
        || [&request.actor, &request.reason, &request.review_reference]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 16000)
    {
        return Err(error(
            "LEGACY_PROFILE_REVIEW",
            "Choose source fields and record a valid date, staff name, reason, reference and confirmation.",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut proposed = context.profile.clone();
    let mut changes = vec![];
    for field in &request.selected {
        if !seen.insert(field) {
            return Err(error(
                "LEGACY_PROFILE_FIELD",
                "Each field can be selected once.",
            ));
        }
        let candidate = context
            .fields
            .iter()
            .find(|c| &c.field == field)
            .ok_or_else(|| error("LEGACY_PROFILE_FIELD", "Unsupported profile field."))?;
        let after = candidate.proposed.as_ref().ok_or_else(|| {
            error(
                "LEGACY_PROFILE_FIELD",
                "A selected source field is absent or unsupported.",
            )
        })?;
        if candidate.current == *after {
            continue;
        }
        if !candidate.current.is_empty() && !request.confirm_overwrite {
            return Err(error(
                "LEGACY_PROFILE_OVERWRITE",
                "Explicitly review replacing existing profile values.",
            ));
        }
        if candidate.would_be_public && !request.confirm_publication {
            return Err(error(
                "LEGACY_PROFILE_PUBLICATION",
                "Review the selected values that would be visible under current publication settings.",
            ));
        }
        field.set(&mut proposed, after.clone());
        changes.push(Change {
            field: *field,
            before: candidate.current.clone(),
            after: after.clone(),
            would_be_public: candidate.would_be_public,
        });
    }
    if changes.is_empty() {
        return Err(error(
            "LEGACY_PROFILE_NO_CHANGE",
            "Selected values already match; nothing needs to be applied.",
        ));
    }
    Ok(Quote {
        request_hash: hash(request)?,
        library_id: context.library_id.clone(),
        revision: context.revision,
        source_name: context.source_name.clone(),
        source_metadata_hash: context.source_metadata_hash.clone(),
        profile_before_hash: context.profile_before_hash.clone(),
        request: request.clone(),
        changes,
        proposed,
    })
}
pub fn history(raw: Option<&Value>) -> Result<Vec<Receipt>> {
    let Some(value) = raw.and_then(|r| r.get(FIELD)) else {
        return Ok(vec![]);
    };
    if serde_json::to_vec(value)?.len() > 1024 * 1024 {
        return Err(error(
            "LEGACY_PROFILE_LIMIT",
            "Legacy profile review history exceeds its supported limit.",
        ));
    }
    let rows: Vec<Receipt> = serde_json::from_value(value.clone()).map_err(|_| {
        error(
            "LEGACY_PROFILE_HISTORY",
            "Saved private legacy profile review history is malformed.",
        )
    })?;
    let mut ids = std::collections::HashSet::new();
    if rows.len() > 512
        || rows.iter().any(|r| {
            !ids.insert(&r.request.id)
                || r.changes.is_empty()
                || r.changes.len() > 7
                || r.quote_hash.len() != 64
        })
    {
        return Err(error(
            "LEGACY_PROFILE_HISTORY",
            "Legacy profile review identities are inconsistent.",
        ));
    }
    Ok(rows)
}
