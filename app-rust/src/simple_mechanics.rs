//! Optional, source-qualified reminders. Never grants powers or computes activation permission.
use crate::{
    model::*,
    power_catalogue,
    printing::{DocLanguage, Row, Section, SheetDocument},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, sync::OnceLock};
pub const SOURCE_SHA: &str = "f6eae9fda388c51e23bfe73338c867123655d5efa7957020857da7c370bac574";
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub expenditure: String,
    pub challenge: String,
    pub retest: String,
    pub effect: String,
    pub conditions: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    pub id: String,
    pub printed_pages: Vec<u32>,
    pub pdf_pages: Vec<u32>,
    pub en: Text,
    pub pt: Text,
    pub retest_basis: String,
    pub status: String,
    pub issues: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub profile: String,
    pub source_sha256: String,
    pub translation_review: String,
    pub entries: Vec<Card>,
    pub coverage_notes: Vec<String>,
}
impl Catalogue {
    pub fn from_json(text: &str) -> Result<Self> {
        if text.len() > 2 * 1024 * 1024 {
            return Err(error(
                "MECHANICS_DATA",
                "Mechanics source data exceeds its limit.",
            ));
        }
        let data: Self = serde_json::from_str(text)?;
        let expected: HashSet<String> = power_catalogue::entries()
            .into_iter()
            .filter(|e| {
                e.template_scope == "Vampire"
                    && e.profile == Some(crate::pricing::PricingProfile::MetLawsOfTheNightRevised)
                    && e.legacy_power.is_some()
            })
            .map(|e| e.stable_id)
            .collect();
        let mut seen = HashSet::new();
        if expected.len() != 40
            || data.profile != "MetLawsOfTheNightRevised"
            || data.source_sha256 != SOURCE_SHA
            || data.translation_review != "equivalence_checked"
            || data.entries.len() != expected.len()
        {
            return Err(error(
                "MECHANICS_DATA",
                "The complete reviewed MET mechanics batch is required.",
            ));
        }
        for card in &data.entries {
            let source = power_catalogue::lookup(&card.id)
                .and_then(|e| e.source)
                .ok_or_else(|| error("MECHANICS_SOURCE", "Unknown MET mechanics identity."))?;
            if !expected.contains(&card.id)
                || !seen.insert(&card.id)
                || !matches!(
                    card.status.as_str(),
                    "reviewed" | "qualified" | "unresolved"
                )
                || !matches!(
                    card.retest_basis.as_str(),
                    "power_text" | "discipline_rule" | "not_stated" | "not_applicable"
                )
                || card.printed_pages.is_empty()
                || card.printed_pages.len() > 8
                || card.printed_pages.len() != card.pdf_pages.len()
                || !card.printed_pages.contains(&source.printed_page)
                || card
                    .printed_pages
                    .iter()
                    .zip(&card.pdf_pages)
                    .any(|(p, d)| *p == 0 || p.checked_add(1) != Some(*d))
                || [&card.en, &card.pt].iter().any(|t| {
                    [
                        &t.expenditure,
                        &t.challenge,
                        &t.retest,
                        &t.effect,
                        &t.conditions,
                    ]
                    .iter()
                    .any(|s| {
                        (card.status != "unresolved" && s.trim().is_empty()) || s.len() > 16000
                    })
                })
                || (card.status == "reviewed" && !card.issues.is_empty())
                || (card.status != "reviewed" && card.issues.is_empty())
            {
                return Err(error(
                    "MECHANICS_SOURCE",
                    "A mechanics card has incomplete or inconsistent source evidence.",
                ));
            }
        }
        Ok(data)
    }
}
pub fn catalogue() -> Result<&'static Catalogue> {
    static DATA: OnceLock<std::result::Result<Catalogue, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        Catalogue::from_json(include_str!("../data/simple_mechanics.json")).map_err(|e| e.message)
    })
    .as_ref()
    .map_err(|message| error("MECHANICS_DATA", message.clone()))
}
fn t(pt: bool, en: &str, translated: &str) -> String {
    if pt { translated.into() } else { en.into() }
}
pub fn append(record: &Record, visible: &[Value], document: &mut SheetDocument) -> Result<()> {
    if !document.options.include_simple_mechanics {
        return Ok(());
    }
    let data = catalogue()?;
    let pt = document.options.language == DocLanguage::Portuguese;
    let edit = crate::sheet::extract(record);
    let actual = edit
        .lists
        .get("DisciplineList")
        .and_then(|list| list["Traits"].as_array());
    let mut seen = HashSet::new();
    let mut omitted = 0;
    for row in visible {
        let owned = actual.and_then(|rows| {
            rows.iter()
                .find(|r| r["Name"] == row["Name"] && r["Value"] == row["Value"])
        });
        let Some(owned) = owned else {
            omitted += 1;
            continue;
        };
        let Some(id) = crate::creation_sources::mapped_power(record, owned) else {
            omitted += 1;
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(card) = data
            .entries
            .iter()
            .find(|c| c.id == id && c.status != "unresolved")
        else {
            omitted += 1;
            continue;
        };
        let entry = power_catalogue::lookup(&id)
            .ok_or_else(|| error("MECHANICS_SOURCE", "The MET source changed."))?;
        let text = if pt { &card.pt } else { &card.en };
        let basis = match card.retest_basis.as_str() {
            "power_text" => t(pt, "Power-specific rule", "Regra específica do poder"),
            "discipline_rule" => t(pt, "Discipline-wide rule", "Regra geral da Disciplina"),
            "not_applicable" => t(pt, "Not applicable", "Não se aplica"),
            _ => t(pt, "Not stated in the source", "Não indicado na fonte"),
        };
        let retest = format!("{basis}: {}", text.retest);
        let mut rows = vec![];
        if card.status == "qualified" {
            rows.push(Row {name:t(pt,"Source qualification","Ressalva da fonte"),value:t(pt,"The source leaves an application condition unspecified. Keep the stated rules and consult your storyteller about that condition.","A fonte não especifica uma condição de aplicação. Preserve as regras indicadas e consulte seu Narrador sobre essa condição."),note:None});
        }
        for (en, translated, value) in [
            ("Expenditure", "Dispêndio", &text.expenditure),
            ("Challenge", "Desafio", &text.challenge),
            ("Retest", "Reteste", &retest),
            ("Effect", "Efeito", &text.effect),
            ("Conditions", "Condições", &text.conditions),
        ] {
            rows.push(Row {
                name: t(pt, en, translated),
                value: value.clone(),
                note: None,
            });
        }
        if let Some(day) = owned.get("AppAvailableFrom").and_then(Value::as_str) {
            rows.push(Row {
                name: t(pt, "Declared availability", "Disponibilidade declarada"),
                value: if crate::xp::day_valid(day) {
                    day.into()
                } else {
                    t(
                        pt,
                        "Unreviewed date; consult staff",
                        "Data não revisada; consulte a equipe",
                    )
                },
                note: None,
            });
        }
        let pages = card
            .printed_pages
            .iter()
            .zip(&card.pdf_pages)
            .map(|(p, d)| format!("p.{p} / PDF{d}"))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(Row {
            name: t(pt, "Source", "Fonte"),
            value: format!("Laws of the Night (Revised), Mind's Eye Theatre: {pages}"),
            note: None,
        });
        rows.push(Row {name:t(pt,"Library","Biblioteca"),value:t(pt,"[For more detailed info check your library for \"Laws of the Night (Revised).\"]","[Para informações mais detalhadas, consulte sua biblioteca em \"Laws of the Night (Revised).\"]"),note:None});
        document.sections.push(Section {
            title: format!(
                "{}: {}",
                t(pt, "Simple mechanics", "Regras resumidas"),
                entry.name
            ),
            rows,
        });
    }
    if omitted > 0 {
        document.warnings.push(t(pt,&format!("Simple mechanics omitted for {omitted} unmapped or unresolved power(s); review the MET mapping and consult your library."),&format!("Regras resumidas omitidas para {omitted} poder(es) sem vínculo ou não resolvidos; revise o vínculo MET e consulte sua biblioteca.")));
    }
    document.warnings.push(t(
        pt,
        "Mechanics reminders do not determine eligibility, current availability or approval.",
        "Os lembretes de regras não determinam elegibilidade, disponibilidade atual ou aprovação.",
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dataset() -> Value {
        let entries=power_catalogue::entries().iter().map(|p| {
            let page=p.source.unwrap().printed_page;
            let text=serde_json::json!({"expenditure":"Synthetic activation","challenge":"Synthetic challenge","retest":"Not stated in synthetic fixture","effect":"Synthetic effect","conditions":"Synthetic condition"});
            serde_json::json!({"id":p.stable_id,"printed_pages":[page],"pdf_pages":[page+1],"en":text,"pt":text,"retest_basis":"not_stated","status":"reviewed","issues":[]})
        }).collect::<Vec<_>>();
        serde_json::json!({"profile":"MetLawsOfTheNightRevised","source_sha256":SOURCE_SHA,"translation_review":"equivalence_checked","entries":entries,"coverage_notes":[]})
    }
    #[test]
    fn incomplete_wrong_edition_source_and_bad_page_pairs_are_rejected() {
        let data = dataset();
        assert_eq!(
            Catalogue::from_json(&data.to_string())
                .unwrap()
                .entries
                .len(),
            40
        );
        let mut bad = data.clone();
        bad["entries"].as_array_mut().unwrap().pop();
        assert!(Catalogue::from_json(&bad.to_string()).is_err());
        let mut bad = data.clone();
        bad["profile"] = serde_json::json!("V20");
        assert!(Catalogue::from_json(&bad.to_string()).is_err());
        let mut bad = data.clone();
        bad["source_sha256"] = serde_json::json!("another edition");
        assert!(Catalogue::from_json(&bad.to_string()).is_err());
        let mut bad = data.clone();
        bad["entries"][0]["pdf_pages"][0] = serde_json::json!(1);
        assert!(Catalogue::from_json(&bad.to_string()).is_err());
        let mut bad = data;
        bad["entries"][1] = bad["entries"][0].clone();
        assert!(Catalogue::from_json(&bad.to_string()).is_err());
    }
    #[test]
    fn old_print_options_keep_mechanics_disabled() {
        let options: crate::printing::Options = serde_json::from_str("{}").unwrap();
        assert!(!options.include_simple_mechanics);
    }
}
