//! Local editorial workspace. Audience labels do not resolve or deliver recipients.
use crate::{
    model::{Record, Result, error},
    plots::{attr, children, date_text, node_text},
    xp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const FIELD: &str = "AppRumorV1";
pub const ROW_LIMIT: usize = 1000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rumor {
    pub title: String,
    pub day: String,
    /// Legacy values from PublicTypes.bas::RumorCategoryType.
    pub category: i32,
    pub audience_label: String,
    pub influence_list: String,
    pub influence_trait: String,
    pub reviewed: bool,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Kept as draft text so an invalid legacy value can be inspected and corrected.
    pub level: String,
    pub body: String,
}

pub fn validate(r: &Rumor) -> Result<()> {
    let bounded = |s: &str, len| s.len() <= len && !s.contains('\0');
    if r.title.trim().is_empty()
        || !bounded(&r.title, 512)
        || !bounded(&r.audience_label, 512)
        || !bounded(&r.influence_list, 512)
        || !bounded(&r.influence_trait, 512)
        || r.entries.len() > ROW_LIMIT
        || r.entries.iter().any(|e| !bounded(&e.body, 16000))
    {
        return Err(error(
            "RUMOR_FIELDS_INVALID",
            "Enter a title and keep the text and rows within the editor limits.",
        ));
    }
    if !xp::day_valid(&r.day) {
        return Err(error(
            "RUMOR_DATE_INVALID",
            "Use YYYY-MM-DD for the rumor date.",
        ));
    }
    if !(0..=5).contains(&r.category)
        || r.entries
            .iter()
            .any(|e| e.level.parse::<i16>().map_or(true, |n| n < 0))
    {
        return Err(error(
            "RUMOR_LEVEL_INVALID",
            "Choose a category and use whole nonnegative levels within the legacy range.",
        ));
    }
    if r.reviewed && (r.entries.is_empty() || r.entries.iter().any(|e| e.body.trim().is_empty())) {
        return Err(error(
            "RUMOR_REVIEW_INCOMPLETE",
            "Fill in each draft entry before marking this rumor reviewed.",
        ));
    }
    Ok(())
}

fn number(v: &Value) -> String {
    v.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| v.to_string())
}

pub(crate) fn read(r: &Record) -> Result<(Rumor, bool, bool)> {
    let xml = r.original.get("xml");
    let binary = r.original.get("fields");
    let legacy = xml.is_some() || binary.is_some();
    if let Some(value) = r.private_fields.get(FIELD) {
        let mut saved: Rumor = serde_json::from_value(value.clone())?;
        saved.title.clone_from(&r.name);
        return Ok((saved, legacy, legacy));
    }
    let mut draft = Rumor {
        title: r.name.clone(),
        ..Rumor::default()
    };
    let mut retained = false;
    // RumorClass.cls::InputFromFile and RumorNode.cls::InputFromFile.
    if let Some(x) = xml {
        let string = |name| {
            attr(x, name)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        draft.day = attr(x, "date").map_or_else(String::new, |d| date_text(d, false));
        draft.category = attr(x, "category").map_or(0, |v| number(v).parse().unwrap_or(-1));
        draft.influence_list = string("list");
        draft.influence_trait = string("trait");
        retained |= attr(x, "done").is_some();
        for node in children(x) {
            match node["tag"].as_str() {
                Some("subrumor") => {
                    if draft.entries.len() == ROW_LIMIT {
                        retained = true;
                        break;
                    }
                    draft.entries.push(Entry {
                        level: attr(node, "level").map_or_else(|| "0".into(), number),
                        body: children(node)
                            .find(|n| n["tag"] == "body")
                            .map_or_else(String::new, node_text),
                    });
                }
                Some(_) => retained = true,
                None => {}
            }
        }
    } else if let Some(b) = binary {
        let string = |name: &str| b[name].as_str().unwrap_or_default().to_owned();
        draft.day = b
            .get("RumorDate")
            .map_or_else(String::new, |d| date_text(d, false));
        draft.category = b["Category"]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(-1);
        draft.influence_list = string("MultiKey");
        draft.influence_trait = string("MultiMatch");
        retained |= b.get("Query").is_some() || b.get("Done").is_some();
        for node in b["SubRumors"].as_array().into_iter().flatten() {
            if draft.entries.len() == ROW_LIMIT {
                retained = true;
                break;
            }
            draft.entries.push(Entry {
                level: number(&node["Level"]),
                body: node["Rumor"].as_str().unwrap_or_default().to_owned(),
            });
        }
    }
    // An imported Done flag is evidence, not local review or delivery.
    Ok((draft, legacy, retained))
}
