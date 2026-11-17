//! One reviewed ordinary Vampire trait increment, recorded in the existing XP ledger.
use crate::{
    model::*,
    pricing::PricingProfile,
    purchases, sheet,
    trait_pricing::{self, Category},
    xp,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug)]
pub struct Context {
    pub revision: i64,
    pub record_id: String,
    pub record_name: String,
    pub sheet: SheetEdit,
    pub unspent: Option<String>,
    pub history: Vec<Quote>,
    pub corrected_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Target {
    Trait {
        field: String,
        index: Option<usize>,
        name: String,
    },
    Resource {
        field: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub category: Category,
    pub target: Target,
    pub profile: Option<PricingProfile>,
    pub show_on_player_sheet: bool,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub policy_version: String,
    pub review_reference: String,
    pub review_confirmed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub library_id: String,
    pub record_id: String,
    pub revision: i64,
    pub record_hash: String,
    pub request: Request,
    pub field: String,
    pub label: String,
    pub index: Option<usize>,
    pub price: trait_pricing::Quote,
    pub before_unspent: String,
    pub after_unspent: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub quote: Quote,
    pub committed_revision: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
enum Evidence {
    #[serde(rename = "trait_advancement_v1")]
    Advancement { quote: Quote },
}

pub fn recorded_quote(command: &xp::XpCommand) -> Option<Quote> {
    let Evidence::Advancement { quote } = serde_json::from_str(&command.evidence).ok()?;
    (command.kind == xp::XpKind::Spend
        && command.id == quote.request.id
        && command.amount == quote.price.cost)
        .then_some(quote)
}

pub fn evidence_summary(command: &xp::XpCommand, portuguese: bool) -> Option<String> {
    let Some(q) = recorded_quote(command) else {
        return crate::trait_corrections::evidence_summary(command, portuguese);
    };
    let (review, policy) = if portuguese {
        ("Revisão", "Política")
    } else {
        ("Review", "Policy")
    };
    Some(format!(
        "{}: {} -> {} · {} XP; {} ({}), p.{} / PDF{}. {}: {}. {}: {}. {}",
        q.label,
        q.price.current,
        q.price.next,
        q.price.cost,
        q.price.citation.book,
        q.price.citation.edition,
        q.price.citation.printed_page,
        q.price.citation.pdf_page_1based,
        review,
        q.request.review_reference,
        policy,
        q.request.policy_version,
        q.price.library_prompt
    ))
}

fn resolve(
    record: &Record,
    request: &Request,
) -> Result<(String, String, Option<usize>, i64, Value)> {
    let fields = sheet::extract(record);
    match &request.target {
        Target::Resource { field } => {
            let valid = match request.category {
                Category::Willpower => field == "Willpower",
                Category::PermanentResource => {
                    crate::sheet::RESOURCE_FIELDS.contains(&field.as_str())
                        && !field.starts_with("Temp")
                        && crate::creature_profiles::profile(&record.kind)
                            .is_some_and(|p| p.trait_xp_costs.contains_key(field))
                }
                Category::HumanityPath => matches!(field.as_str(), "Humanity" | "PathTraits"),
                Category::Virtue => matches!(
                    field.as_str(),
                    "Conscience" | "Conviction" | "Courage" | "SelfControl" | "Instinct"
                ),
                _ => false,
            };
            if !valid {
                return Err(error(
                    "TRAIT_TARGET",
                    "Choose a supported permanent resource for this category.",
                ));
            }
            let value = record
                .private_fields
                .get(field)
                .or_else(|| record.public_fields.get(field));
            let current = match value {
                Some(v) => v.as_i64().ok_or_else(|| {
                    error(
                        "TRAIT_VALUE",
                        "A descriptive resource cannot be advanced numerically.",
                    )
                })?,
                None => 0,
            };
            Ok((field.clone(), field.clone(), None, current, json!(current)))
        }
        Target::Trait { field, index, name } => {
            let valid = match request.category {
                Category::Attribute => {
                    matches!(field.as_str(), "PhysicalList" | "SocialList" | "MentalList")
                }
                Category::Ability => field == "AbilityList",
                Category::Background => field == "BackgroundList",
                _ => false,
            };
            if !valid || name.trim().is_empty() || name.len() > 1000 {
                return Err(error(
                    "TRAIT_TARGET",
                    "Choose a supported list and a named trait.",
                ));
            }
            let list = if let Some(list) = fields.lists.get(field) {
                list.clone()
            } else {
                let existing = record
                    .private_fields
                    .get(field)
                    .or_else(|| record.public_fields.get(field));
                let legacy = record
                    .private_fields
                    .get("UnmappedTraitLists")
                    .or_else(|| record.public_fields.get("UnmappedTraitLists"));
                let ambiguous = legacy.is_some_and(|value| {
                    let rows = value
                        .as_array()
                        .map(|v| v.iter().collect::<Vec<_>>())
                        .unwrap_or_else(|| vec![value]);
                    rows.iter().any(|v| {
                        v["Name"].as_str().and_then(crate::importer::xml_list_key)
                            == Some(field.as_str())
                    })
                });
                if existing.is_some() || ambiguous {
                    return Err(error(
                        "TRAIT_STRUCTURE",
                        "Existing or ambiguous legacy traits need review before advancement.",
                    ));
                }
                sheet::empty_list(field)
            };
            let rows = list["Traits"]
                .as_array()
                .ok_or_else(|| error("TRAIT_STRUCTURE", "Reload a supported trait list."))?;
            let matching = rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row["Name"]
                        .as_str()
                        .is_some_and(|v| v.trim().to_lowercase() == name.trim().to_lowercase())
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            let current = if let Some(index) = index {
                if matching != [*index]
                    || rows.get(*index).and_then(|r| r["Name"].as_str()) != Some(name.as_str())
                {
                    return Err(error(
                        "TRAIT_AMBIGUOUS",
                        "Reload an unambiguous exact trait name before advancing.",
                    ));
                }
                let value = rows[*index]["Value"].as_str().ok_or_else(|| {
                    error("TRAIT_VALUE", "The current trait value is not numeric.")
                })?;
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(error(
                        "TRAIT_VALUE",
                        "Only explicit nonnegative numeric trait counts can be advanced.",
                    ));
                }
                value.parse().map_err(|_| {
                    error(
                        "TRAIT_VALUE",
                        "The current trait count exceeds storage bounds.",
                    )
                })?
            } else {
                if !matching.is_empty() || rows.len() >= 4096 {
                    return Err(error(
                        "TRAIT_AMBIGUOUS",
                        "Choose the existing trait or resolve duplicate names; no duplicate is created.",
                    ));
                }
                0
            };
            let label = if index.is_some() {
                name.clone()
            } else {
                name.trim().into()
            };
            Ok((field.clone(), label, *index, current, list))
        }
    }
}

fn command(q: &Quote) -> Result<xp::XpCommand> {
    Ok(xp::XpCommand {
        id: q.request.id.clone(),
        kind: xp::XpKind::Spend,
        amount: q.price.cost.clone(),
        day: q.request.day.clone(),
        actor: q.request.actor.clone(),
        reason: q.request.reason.clone(),
        evidence: serde_json::to_string(&Evidence::Advancement { quote: q.clone() })?,
        reversal_of: None,
        cost: Some(xp::CostEvidence {
            affiliation_name: String::new(),
            power_family: format!("Ordinary {:?} / {}", q.request.category, q.label),
            approval_reference: q.request.review_reference.clone(),
            base: q.price.cost.clone(),
            affiliation_adjustment: "0".into(),
            affiliation: "Not a Discipline purchase".into(),
            source: q.price.citation.book.clone(),
            edition: q.price.citation.edition.clone(),
            page: q.price.citation.printed_page.to_string(),
            policy_version: q.request.policy_version.clone(),
        }),
    })
}
fn debit(record: &Record, quote: &Quote, timestamp: u64) -> Result<Record> {
    let mut updated = record.clone();
    let ledger = updated.xp_ledger.as_mut().ok_or_else(|| {
        error(
            "XP_OPENING_REQUIRED",
            "Declare the recorded XP opening first; this is not a creation budget.",
        )
    })?;
    let command = xp::canonical_command(ledger, &command(quote)?)?;
    if ledger
        .entries
        .iter()
        .any(|entry| entry.command.id == command.id)
    {
        return Err(error(
            "XP_DUPLICATE_ID",
            "This advancement ID is already recorded.",
        ));
    }
    ledger.entries.push(xp::Entry {
        command,
        recorded_unix_seconds: timestamp,
        origin_library_id: quote.library_id.clone(),
    });
    xp::replay(ledger)?;
    Ok(updated)
}

pub fn quote(library_id: &str, revision: i64, record: &Record, request: &Request) -> Result<Quote> {
    if !crate::sheet::is_character_kind(&record.kind) || record.build.is_some() {
        return Err(error(
            "TRAIT_SCOPE",
            "Only the current Vampire record can spend XP; Draft/Test copies remain separate.",
        ));
    }
    if uuid::Uuid::parse_str(&request.id)
        .ok()
        .is_none_or(|id| id.to_string() != request.id)
    {
        return Err(error("TRAIT_ID", "Use a canonical advancement ID."));
    }
    if !request.review_confirmed
        || [
            (&request.actor, 200),
            (&request.reason, 512),
            (&request.policy_version, 128),
            (&request.review_reference, 512),
        ]
        .iter()
        .any(|(value, max)| value.trim().is_empty() || value.len() > *max)
    {
        return Err(error(
            "TRAIT_REVIEW",
            "Record the ST, reason, local policy and eligibility/maximum/approval review before spending.",
        ));
    }
    let (field, label, index, current, _) = resolve(record, request)?;
    let price = if request.category == Category::PermanentResource {
        trait_pricing::preview_resource(&record.kind, request.profile, &field, current)?
    } else {
        trait_pricing::preview(&record.kind, request.profile, request.category, current)?
    };
    let ledger = record.xp_ledger.as_ref().ok_or_else(|| {
        error(
            "XP_OPENING_REQUIRED",
            "Declare the recorded XP opening first.",
        )
    })?;
    let (_, before) = xp::replay(ledger)?;
    let mut q = Quote {
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        request: request.clone(),
        field,
        label,
        index,
        after_unspent: xp::decimal(before - xp::amount(&price.cost)?),
        before_unspent: xp::decimal(before),
        price,
    };
    let updated = debit(record, &q, 0)?;
    q.after_unspent = xp::decimal(xp::replay(updated.xp_ledger.as_ref().unwrap())?.1);
    Ok(q)
}

pub fn apply(record: &Record, q: &Quote, timestamp: u64) -> Result<Record> {
    if quote(&q.library_id, q.revision, record, &q.request)? != *q {
        return Err(error(
            "TRAIT_QUOTE_CHANGED",
            "Reload and review this advancement again.",
        ));
    }
    let (_, _, index, _, mut value) = resolve(record, &q.request)?;
    if matches!(q.request.target, Target::Trait { .. }) {
        let rows = value["Traits"].as_array_mut().unwrap();
        if let Some(index) = index {
            rows[index]["Value"] = json!(q.price.next.to_string());
            rows[index]["AppTraitAdvancementId"] = json!(q.request.id);
        } else {
            rows.push(json!({"Name":q.label,"Value":q.price.next.to_string(),"Note":"","AppTraitAdvancementId":q.request.id}));
        }
        let mut edit = SheetEdit::default();
        edit.lists.insert(q.field.clone(), value.clone());
        sheet::validate(&edit)?;
    } else {
        value = json!(q.price.next);
    }
    let mut updated = debit(record, q, timestamp)?;
    if q.request.show_on_player_sheet {
        updated
            .public_fields
            .entry(q.field.clone())
            .or_insert(Value::Null);
    } else {
        updated.public_fields.remove(&q.field);
    }
    sheet::write_field(&mut updated, &q.field, value);
    Ok(updated)
}

pub fn findings(record: &Record) -> Vec<xp::Finding> {
    let Some(ledger) = &record.xp_ledger else {
        return vec![];
    };
    if record.build.is_some() {
        return vec![];
    }
    let mut latest = std::collections::BTreeMap::new();
    let corrected = crate::trait_corrections::corrected_ids(ledger);
    for entry in &ledger.entries {
        if let Some(q) = recorded_quote(&entry.command) {
            if corrected.contains(&q.request.id) {
                continue;
            }
            latest.insert((q.field.clone(), q.label.clone()), q);
        }
    }
    latest.values().filter_map(|q| {
        let fields = sheet::extract(record);
        let expected_count = q.price.next.to_string();
        let matches = if matches!(q.request.target, Target::Resource { .. }) { fields.resources.get(&q.field) == Some(&q.price.next) }
        else { fields.lists.get(&q.field).and_then(|v| v["Traits"].as_array()).is_some_and(|rows| rows.iter().filter(|row| row["Name"].as_str().is_some_and(|name| name.trim().to_lowercase() == q.label.trim().to_lowercase())).count() == 1 && rows.iter().filter(|row| row["AppTraitAdvancementId"] == q.request.id && row["Name"] == q.label && row["Value"].as_str() == Some(expected_count.as_str())).count() == 1) };
        (!matches).then(|| xp::Finding { code:"TRAIT_ADVANCEMENT_SHEET_DRIFT".into(), severity:"review".into(), entry_id:Some(q.request.id.clone()), message:"An ordinary-trait advancement no longer matches the recorded sheet; review later edits and corrections.".into() })
    }).collect()
}
