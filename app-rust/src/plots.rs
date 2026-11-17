//! Private Plot workspace. Original reader payloads are evidence, never rewritten.
use crate::{
    model::{Language, Record, Result, error},
    xp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Write;

pub(crate) const FIELD: &str = "AppPlotV1";
const ROW_LIMIT: usize = 1000;
const TEXT_LIMIT: usize = 16_000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plot {
    pub name: String,
    pub narrator: String,
    pub outline: String,
    pub start: String,
    pub end: String,
    pub cast: Vec<Cast>,
    pub developments: Vec<Development>,
}

impl Plot {
    pub fn order_timeline(&mut self) {
        self.developments.sort_by(|a, b| a.day.cmp(&b.day));
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cast {
    pub label: String,
    pub value: String,
    pub note: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Development {
    pub day: String,
    pub event: String,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub revision: i64,
    pub plot: Plot,
    pub legacy: bool,
    pub retained: bool,
}

pub(crate) fn validate(p: &Plot) -> Result<()> {
    let bounded = |s: &str| s.len() <= TEXT_LIMIT && !s.contains('\0');
    if p.name.trim().is_empty()
        || p.name.len() > 512
        || !bounded(&p.name)
        || !bounded(&p.narrator)
        || !bounded(&p.outline)
        || p.cast.len() > ROW_LIMIT
        || p.developments.len() > ROW_LIMIT
        || p.cast.iter().any(|c| {
            c.label.trim().is_empty()
                || !bounded(&c.label)
                || !bounded(&c.value)
                || !bounded(&c.note)
        })
        || p.developments
            .iter()
            .any(|d| d.event.trim().is_empty() || !bounded(&d.event))
    {
        return Err(error(
            "PLOT_FIELDS_INVALID",
            "Enter a title, nonempty cast labels and events; keep text and row counts within the editor limits.",
        ));
    }
    if [&p.start, &p.end]
        .iter()
        .any(|d| !d.is_empty() && !xp::day_valid(d))
        || (!p.start.is_empty() && !p.end.is_empty() && p.start > p.end)
        || p.developments.iter().any(|d| !xp::day_valid(&d.day))
    {
        return Err(error(
            "PLOT_DATE_INVALID",
            "Use YYYY-MM-DD for dates. End must not precede start; every event needs a date.",
        ));
    }
    Ok(())
}

// VB XML used locale-dependent CStr(Date). A slash date is accepted only if
// its day/month order can be determined without assuming the author's locale.
// Binary dates are OLE days; truncate before conversion (negative OLE fractions
// encode positive time of day). Time and exact representation stay in original.
fn date(v: &Value, optional: bool) -> Option<String> {
    if let Some(n) = v.as_f64().or_else(|| v["ole_automation_days"].as_f64()) {
        if optional && n == 0.0 {
            return Some(String::new());
        }
        let n = n.trunc();
        if !n.is_finite() {
            return None;
        }
        return xp::ole_day(n as i64);
    }
    let s = v.as_str()?.trim();
    if optional && (s.is_empty() || s == "0") {
        return Some(String::new());
    }
    if xp::day_valid(s) {
        return Some(s.to_owned());
    }
    let (first, rem) = s.split_once('/')?;
    let (second, year) = rem.split_once('/')?;
    if year.len() != 4
        || ![first, second, year]
            .iter()
            .all(|s| s.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let (a, b) = (first.parse::<u8>().ok()?, second.parse::<u8>().ok()?);
    let (month, day) = if a > 12 {
        (b, a)
    } else if b > 12 || a == b {
        (a, b)
    } else {
        return None;
    };
    let day = format!("{year}-{month:02}-{day:02}");
    xp::day_valid(&day).then_some(day)
}

pub(crate) fn date_text(v: &Value, optional: bool) -> String {
    date(v, optional).unwrap_or_else(|| {
        v.as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| v.to_string())
    })
}

pub(crate) fn attr<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    node["attributes"]
        .as_array()?
        .iter()
        .find(|a| a["name"] == key)?
        .get("value")
}

pub(crate) fn children(node: &Value) -> impl Iterator<Item = &Value> {
    node["children"].as_array().into_iter().flatten()
}

pub(crate) fn node_text(node: &Value) -> String {
    children(node).filter_map(|n| n["text"].as_str()).collect()
}

pub(crate) fn read(r: &Record) -> Result<(Plot, bool, bool)> {
    let xml = r.original.get("xml");
    let binary = r.original.get("fields");
    let legacy = xml.is_some() || binary.is_some();
    if let Some(v) = r.private_fields.get(FIELD) {
        let mut p: Plot = serde_json::from_value(v.clone())?;
        p.name.clone_from(&r.name);
        return Ok((p, legacy, legacy));
    }
    let mut p = Plot {
        name: r.name.clone(),
        ..Default::default()
    };
    let mut retained = false;
    if let Some(x) = xml {
        for (key, field) in [("start", &mut p.start), ("end", &mut p.end)] {
            if let Some(v) = attr(x, key) {
                *field = date_text(v, true);
            }
        }
        p.narrator = attr(x, "narrator")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        for child in children(x) {
            match child["tag"].as_str() {
                Some("outline") => p.outline = node_text(child),
                Some("traitlist")
                    if attr(child, "name").and_then(Value::as_str) == Some("Key Characters") =>
                {
                    for row in children(child).filter(|n| n["tag"] == "trait") {
                        if p.cast.len() == ROW_LIMIT {
                            retained = true;
                            break;
                        }
                        let field = |key| {
                            attr(row, key)
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned()
                        };
                        p.cast.push(Cast {
                            label: field("name"),
                            value: field("val"),
                            note: field("note"),
                        });
                    }
                }
                Some("development") => {
                    if p.developments.len() == ROW_LIMIT {
                        retained = true;
                        break;
                    }
                    let day = attr(child, "date").map_or_else(String::new, |v| date_text(v, false));
                    let event = children(child)
                        .find(|n| n["tag"] == "event")
                        .map_or_else(String::new, node_text);
                    retained |= children(child).any(|n| n["tag"] == "linklist");
                    p.developments.push(Development { day, event });
                }
                Some(_) => retained = true,
                None => {}
            }
        }
    } else if let Some(b) = binary {
        let string = |key: &str| b[key].as_str().unwrap_or_default().to_owned();
        p.narrator = string("Narrator");
        p.outline = string("Outline");
        for (key, field) in [("StartDate", &mut p.start), ("EndDate", &mut p.end)] {
            if let Some(v) = b.get(key) {
                *field = date_text(v, true);
            }
        }
        for row in b["CastList"]["Traits"].as_array().into_iter().flatten() {
            if p.cast.len() == ROW_LIMIT {
                retained = true;
                break;
            }
            let field = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
            p.cast.push(Cast {
                label: field("Name"),
                value: field("Value"),
                note: field("Note"),
            });
        }
        for row in b["Developments"].as_array().into_iter().flatten() {
            if p.developments.len() == ROW_LIMIT {
                retained = true;
                break;
            }
            retained |= row["Effects"]["Entries"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
                || row["Effects"].as_array().is_some_and(|a| !a.is_empty());
            p.developments.push(Development {
                day: row
                    .get("DevDate")
                    .map_or_else(String::new, |v| date_text(v, false)),
                event: row["Development"].as_str().unwrap_or_default().to_owned(),
            });
        }
    }
    Ok((p, legacy, retained))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Pending,
    Active,
    Finished,
}

pub fn status(p: &Plot, day: &str) -> Result<Status> {
    if !xp::day_valid(day)
        || [&p.start, &p.end]
            .iter()
            .any(|d| !d.is_empty() && !xp::day_valid(d))
        || (!p.start.is_empty() && !p.end.is_empty() && p.start > p.end)
    {
        return Err(error(
            "PLOT_DATE_INVALID",
            "Correct the plot dates and enter an explicit report date (YYYY-MM-DD).",
        ));
    }
    Ok(if !p.start.is_empty() && day < p.start.as_str() {
        Status::Pending
    } else if !p.end.is_empty() && day > p.end.as_str() {
        Status::Finished
    } else {
        Status::Active
    })
}

pub fn html(s: &Snapshot, day: &str, language: Language, notes: bool) -> Result<String> {
    let p = &s.plot;
    validate(p)?;
    let pt = language == Language::Portuguese;
    let t = |en, pt_text| if pt { pt_text } else { en };
    let escape = crate::world_card::escape;
    let status = match status(p, day)? {
        Status::Pending => t("Pending", "Pendente"),
        Status::Active => t("Active", "Ativa"),
        Status::Finished => t("Finished", "Encerrada"),
    };
    let mut out = format!(
        "<!doctype html><html lang=\"{}\"><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{}</title><style>body{{font:16px system-ui;max-width:55rem;margin:2rem auto;padding:0 1rem}}p,li{{white-space:pre-wrap;overflow-wrap:anywhere}}li{{margin:.4rem 0}}article{{break-inside:avoid}}@media print{{body{{margin:0;max-width:none}}}}</style><h1>{}</h1><p><strong>{}</strong> · {}: {} · {}: {} · {} {}</p>",
        t("en", "pt-BR"),
        escape(&p.name),
        escape(&p.name),
        t(
            "Private Storyteller plot report",
            "Relatório privado de trama do Narrador"
        ),
        t("As of", "Data de referência"),
        day,
        t("Status", "Situação"),
        status,
        t("Library revision", "Revisão da biblioteca"),
        s.revision
    );
    let _ = write!(
        out,
        "<p>{}: {} · {}: {} · {}: {}</p><h2>{}</h2><p>{}</p>",
        t("Start", "Início"),
        escape(&p.start),
        t("End", "Fim"),
        escape(&p.end),
        t("Narrator", "Narrador"),
        escape(&p.narrator),
        t("Outline", "Resumo"),
        escape(&p.outline)
    );
    if s.legacy {
        let _ = write!(
            out,
            "<p>{}</p>",
            t(
                "Legacy source remains in the library. This report includes only the structured plot fields shown in the editor.",
                "A origem legada permanece na biblioteca. O relatório inclui apenas os campos estruturados exibidos no editor."
            )
        );
    }
    let _ = write!(out, "<h2>{}</h2><ul>", t("Cast labels", "Nomes do elenco"));
    for c in &p.cast {
        let _ = write!(
            out,
            "<li>{} {}{}</li>",
            escape(&c.label),
            escape(&c.value),
            if notes && !c.note.is_empty() {
                format!(" — {}", escape(&c.note))
            } else {
                String::new()
            }
        );
    }
    let _ = write!(
        out,
        "</ul><h2>{}</h2>",
        t("Full saved timeline", "Cronologia salva completa")
    );
    let mut events: Vec<_> = p.developments.iter().collect();
    events.sort_by(|a, b| a.day.cmp(&b.day));
    for d in events {
        let _ = write!(
            out,
            "<article><h3>{}</h3><p>{}</p></article>",
            d.day,
            escape(&d.event)
        );
    }
    out.push_str("</html>");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn binary_plot_zero_bound_and_ole_day_are_projected_without_rewriting_source() {
        let raw = json!({"fields":{
            "StartDate":25569.75,"EndDate":0.0,"Narrator":"Guide","Outline":"Plan",
            "CastList":{"Traits":[{"Name":"Witness","Value":"1","Note":"keep"}]},
            "Developments":[{"DevDate":25570.5,"Development":"News","Effects":[{"Target":2}]}]
        }});
        let r = Record {
            id: "p".into(),
            name: "Binary Plot".into(),
            kind: "Plot".into(),
            player_id: None,
            legacy_id: None,
            public_fields: Default::default(),
            private_fields: Default::default(),
            original: raw.clone(),
            source_sha256: "fixture".into(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        };
        let (p, legacy, retained) = read(&r).unwrap();
        assert!(legacy && retained);
        assert_eq!(p.start, "1970-01-01");
        assert_eq!(p.end, "");
        assert_eq!(p.developments[0].day, "1970-01-02");
        assert_eq!(p.cast[0].note, "keep");
        assert_eq!(r.original, raw);
        assert_eq!(date(&json!(-1.25), false).unwrap(), "1899-12-29");
        assert_eq!(date(&json!(-657434.0), false).unwrap(), "0100-01-01");
        assert!(date(&json!(-657435.0), false).is_none());
        assert!(date(&json!("11/10/2026"), false).is_none());
        assert!(date(&json!("31/2/2026"), false).is_none());
    }
}
