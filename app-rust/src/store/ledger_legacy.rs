//! Report-only XML legacy history projection. No baseline, ledger or source rewrite.
use super::*;
use crate::{plots, xp};

pub(super) fn report_legacy(record: &Record, mut view: xp::XpView) -> xp::XpView {
    let current = record
        .private_fields
        .get("Experience")
        .or_else(|| record.public_fields.get("Experience"));
    if current.and_then(|e| e["History"].as_array()).is_some() {
        return view;
    }
    let Some(root) = record.original.get("xml") else {
        return view;
    };
    let generic = importer::xml_fields(root);
    if current.is_some() && current != generic.get("Experience") {
        return view;
    }
    let experiences: Vec<_> = plots::children(root)
        .filter(|n| n["tag"] == "experience")
        .collect();
    if experiences.len() != 1 {
        return view;
    }
    let experience = experiences[0];
    let string = |key| {
        plots::attr(experience, key)
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    view.legacy_earned = string("earned");
    view.legacy_unspent = string("unspent");
    let mut rows = Vec::new();
    let mut partial = false;
    for node in plots::children(experience) {
        if node["tag"] != "entry" {
            if node.get("tag").is_some() {
                partial = true;
            }
            continue;
        }
        let text = |key| {
            plots::attr(node, key)
                .and_then(Value::as_str)
                .unwrap_or("Unknown")
                .to_owned()
        };
        let kind = match text("type").as_str() {
            "0" => "Earned",
            "1" => "Deducted",
            "2" => "Set earned",
            "3" => "Spent",
            "4" => "Refund/unspent",
            "5" => "Set unspent",
            "6" => "Comment",
            _ => "Unknown event",
        };
        rows.push(xp::LegacyEvent {
            id: format!("xmllegacy:{}", rows.len()),
            date: plots::attr(node, "date")
                .map_or_else(|| "Unknown date".into(), |d| plots::date_text(d, false)),
            kind: kind.into(),
            amount: text("change"),
            earned: text("earned"),
            unspent: text("unspent"),
            reason: plots::attr(node, "reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
        });
        if rows.len() == 100000 {
            partial = true;
            break;
        }
    }
    view.legacy_history_count = rows.len();
    view.legacy_history = rows;
    if !partial {
        view.findings
            .retain(|f| f.code != "LEGACY_HISTORY_UNAVAILABLE");
    }
    view
}
