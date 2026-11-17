//! Typed, in-memory character searches and private snapshot reports.
//!
//! This module deliberately has no SQL or source-rule interpretation. Callers
//! prepare a bounded live-character input set and decide which fields are safe
//! to include in each snapshot.

use crate::model::{Language, Result, error};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_CLAUSES: usize = 32;
const MAX_QUERY_TEXT_BYTES: usize = 1_024;
// A storage-safety bound only; it is not a character or game-rule limit.
const MAX_QUERY_NUMBER: i64 = 1_000_000_000;
const MAX_REPORT_ROWS: usize = 10_000;
const MAX_REPORT_TEXT_BYTES: usize = 4_096;
const MAX_HTML_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    All,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Field {
    Name,
    Family,
    Player,
    Status,
    Group,
    Subgroup,
    Npc,
    Trait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Comparison {
    Equal,
    Contains,
    AtLeast,
    AtMost,
    Exists,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub mode: Mode,
    pub clauses: Vec<Clause>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clause {
    pub field: Field,
    pub comparison: Comparison,
    pub negate: bool,
    /// Text to match, or the canonical `sheet::LIST_FIELDS` key for Trait.
    pub value: String,
    /// Exact case-insensitive trait name for Trait clauses.
    pub trait_name: String,
    /// Rating threshold for Trait, or 0/1 for Npc.
    pub number: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// Internal identity for joining and selection. Reports never render this.
    pub id: String,
    pub name: String,
    pub family: String,
    pub status: String,
    pub group: String,
    pub subgroup: String,
    pub player: String,
    /// `None` means the imported NPC value was unavailable or unparseable.
    pub npc: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trait {
    pub name: String,
    /// Only a known integer rating is supplied; uncertain source values remain None.
    pub rating: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub row: Row,
    /// Keys are canonical values from `sheet::LIST_FIELDS`.
    pub traits: BTreeMap<String, Vec<Trait>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub title: String,
    pub revision: i64,
    pub query: Query,
    pub include_players: bool,
    pub rows: Vec<Row>,
}

fn invalid_query() -> crate::model::AppError {
    error(
        "SEARCH_QUERY_INVALID",
        "The saved search is invalid or exceeds its limits.",
    )
}

fn bounded_query_text(value: &str) -> bool {
    value.len() <= MAX_QUERY_TEXT_BYTES && !value.contains('\0')
}

/// Validates clause count, text bounds, numeric bounds, and each field/operator shape.
pub fn validate(query: &Query) -> Result<()> {
    if query.clauses.len() > MAX_CLAUSES {
        return Err(invalid_query());
    }

    for clause in &query.clauses {
        if !bounded_query_text(&clause.value)
            || !bounded_query_text(&clause.trait_name)
            || clause.number < 0
            || clause.number > MAX_QUERY_NUMBER
        {
            return Err(invalid_query());
        }

        match clause.field {
            Field::Name
            | Field::Family
            | Field::Player
            | Field::Status
            | Field::Group
            | Field::Subgroup => {
                if !clause.trait_name.is_empty() || clause.number != 0 {
                    return Err(invalid_query());
                }
                match clause.comparison {
                    Comparison::Equal | Comparison::Contains if !clause.value.is_empty() => {}
                    Comparison::Exists if clause.value.is_empty() => {}
                    _ => return Err(invalid_query()),
                }
            }
            Field::Trait => {
                if !crate::sheet::LIST_FIELDS.contains(&clause.value.as_str())
                    || clause.trait_name.trim().is_empty()
                {
                    return Err(invalid_query());
                }
                match clause.comparison {
                    Comparison::Equal | Comparison::AtLeast | Comparison::AtMost => {}
                    Comparison::Exists if clause.number == 0 => {}
                    _ => return Err(invalid_query()),
                }
            }
            Field::Npc => {
                if clause.comparison != Comparison::Equal
                    || !clause.value.is_empty()
                    || !clause.trait_name.is_empty()
                    || clause.number > 1
                {
                    return Err(invalid_query());
                }
            }
        }
    }
    Ok(())
}

fn folded(value: &str) -> String {
    value.to_lowercase()
}

fn text_for(row: &Row, field: Field) -> Option<&str> {
    match field {
        Field::Name => Some(&row.name),
        Field::Family => Some(&row.family),
        Field::Player => Some(&row.player),
        Field::Status => Some(&row.status),
        Field::Group => Some(&row.group),
        Field::Subgroup => Some(&row.subgroup),
        Field::Npc | Field::Trait => None,
    }
}

/// Returns None for absent or uncertain values so clause negation cannot match them.
fn clause_result(clause: &Clause, input: &Input) -> Option<bool> {
    match clause.field {
        Field::Name
        | Field::Family
        | Field::Player
        | Field::Status
        | Field::Group
        | Field::Subgroup => {
            let value = text_for(&input.row, clause.field)?;
            if value.trim().is_empty() {
                return None;
            }
            Some(match clause.comparison {
                Comparison::Equal => folded(value) == folded(&clause.value),
                Comparison::Contains => folded(value).contains(&folded(&clause.value)),
                Comparison::Exists => true,
                Comparison::AtLeast | Comparison::AtMost => return None,
            })
        }
        Field::Npc => {
            let value = input.row.npc?;
            match clause.comparison {
                Comparison::Equal => Some(i64::from(value) == clause.number),
                _ => None,
            }
        }
        Field::Trait => {
            let list = input.traits.get(&clause.value)?;
            let mut found_name = false;
            let mut found_rating = false;
            for item in list
                .iter()
                .filter(|item| folded(&item.name) == folded(&clause.trait_name))
            {
                found_name = true;
                match clause.comparison {
                    Comparison::Exists => return Some(true),
                    Comparison::Equal | Comparison::AtLeast | Comparison::AtMost => {
                        let Some(rating) = item
                            .rating
                            .filter(|rating| (0..=MAX_QUERY_NUMBER).contains(rating))
                        else {
                            continue;
                        };
                        found_rating = true;
                        let matched = match clause.comparison {
                            Comparison::Equal => rating == clause.number,
                            Comparison::AtLeast => rating >= clause.number,
                            Comparison::AtMost => rating <= clause.number,
                            _ => false,
                        };
                        if matched {
                            return Some(true);
                        }
                    }
                    Comparison::Contains => return None,
                }
            }
            match clause.comparison {
                Comparison::Exists if found_name => Some(true),
                Comparison::Exists => None,
                Comparison::Equal | Comparison::AtLeast | Comparison::AtMost if found_rating => {
                    Some(false)
                }
                _ => None,
            }
        }
    }
}

/// Matches one input against a validated query. An empty query includes every supplied row.
pub fn matches(query: &Query, input: &Input) -> bool {
    if query.clauses.is_empty() {
        return true;
    }

    let clause_matches = |clause: &Clause| match clause_result(clause, input) {
        Some(value) => value ^ clause.negate,
        None => false,
    };

    match query.mode {
        Mode::All => query.clauses.iter().all(clause_matches),
        Mode::Any => query.clauses.iter().any(clause_matches),
    }
}

fn localized<'a>(english: &'a str, portuguese: &'a str, language: Language) -> &'a str {
    match language {
        Language::English => english,
        Language::Portuguese => portuguese,
    }
}

fn report_error() -> crate::model::AppError {
    error(
        "SEARCH_REPORT_INVALID",
        "The search report exceeds its limits or contains invalid data.",
    )
}

fn report_text_valid(value: &str) -> bool {
    value.len() <= MAX_REPORT_TEXT_BYTES && !value.contains('\0')
}

pub(crate) fn append(out: &mut String, value: &str) -> Result<()> {
    if out
        .len()
        .checked_add(value.len())
        .is_none_or(|length| length > MAX_HTML_BYTES)
    {
        return Err(error(
            "SEARCH_REPORT_LIMIT",
            "The print report exceeds its output limit.",
        ));
    }
    out.push_str(value);
    Ok(())
}

pub(crate) fn append_escaped(out: &mut String, value: &str) -> Result<()> {
    for character in value.chars() {
        match character {
            '&' => append(out, "&amp;")?,
            '<' => append(out, "&lt;")?,
            '>' => append(out, "&gt;")?,
            '"' => append(out, "&quot;")?,
            '\'' => append(out, "&#39;")?,
            '\n' | '\r' | '\t' => {
                let mut encoded = [0; 4];
                append(out, character.encode_utf8(&mut encoded))?;
            }
            value if value.is_control() => append(out, "&#xfffd;")?,
            value => {
                let mut encoded = [0; 4];
                append(out, value.encode_utf8(&mut encoded))?;
            }
        }
    }
    Ok(())
}

fn unavailable(language: Language) -> &'static str {
    localized("Unavailable", "Indisponível", language)
}

fn display_value(value: &str, language: Language) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        unavailable(language)
    } else {
        trimmed
    }
}

fn field_label(field: Field, language: Language) -> &'static str {
    match field {
        Field::Name => localized("Name", "Nome", language),
        Field::Family => localized("Family", "Família", language),
        Field::Player => localized("Player", "Jogador", language),
        Field::Status => localized("Status", "Status", language),
        Field::Group => localized("Group", "Grupo", language),
        Field::Subgroup => localized("Subgroup", "Subgrupo", language),
        Field::Npc => "NPC",
        Field::Trait => localized("Trait", "Traço", language),
    }
}

fn list_label(key: &str, language: Language) -> &'static str {
    let (english, portuguese) = match key {
        "AbilityList" => ("Abilities", "Habilidades"),
        "ArcanoiList" => ("Arcanoi", "Arcanoi"),
        "ArtList" => ("Arts", "Artes"),
        "BackgroundList" => ("Backgrounds", "Antecedentes"),
        "BondList" => ("Bonds", "Vínculos"),
        "DerangementList" => ("Derangements", "Perturbações"),
        "DisciplineList" => ("Disciplines", "Disciplinas"),
        "EdgeList" => ("Edges", "Edges"),
        "EquipmentList" => ("Equipment", "Equipamento"),
        "FeatureList" => ("Features", "Características"),
        "FlawList" => ("Flaws", "Defeitos"),
        "GiftList" => ("Gifts", "Dons"),
        "GloryList" => ("Glory", "Glória"),
        "GuanxiList" => ("Guanxi", "Guanxi"),
        "HangoutList" => ("Hangouts", "Locais"),
        "HealthList" => ("Health", "Saúde"),
        "HekauList" => ("Hekau", "Hekau"),
        "HonorList" => ("Honor", "Honra"),
        "HumanityList" => ("Humanity", "Humanidade"),
        "InfluenceList" => ("Influence", "Influência"),
        "LoreList" => ("Lore", "Conhecimento"),
        "MentalList" => ("Mental Traits", "Características Mentais"),
        "MentalNegList" => (
            "Negative Mental Traits",
            "Características Mentais Negativas",
        ),
        "MeritList" => ("Merits", "Méritos"),
        "MiscellaneousList" => ("Miscellaneous", "Diversos"),
        "NuminaList" => ("Numina", "Numina"),
        "PhysicalList" => ("Physical Traits", "Características Físicas"),
        "PhysicalNegList" => (
            "Negative Physical Traits",
            "Características Físicas Negativas",
        ),
        "PowerList" => ("Powers", "Poderes"),
        "RealmList" => ("Realms", "Reinos"),
        "ReputationList" => ("Reputation", "Reputação"),
        "ResonanceList" => ("Resonance", "Ressonância"),
        "RiteList" => ("Rites", "Ritos"),
        "RitualList" => ("Rituals", "Rituais"),
        "RoteList" => ("Rotes", "Rotes"),
        "SocialList" => ("Social Traits", "Características Sociais"),
        "SocialNegList" => (
            "Negative Social Traits",
            "Características Sociais Negativas",
        ),
        "SpellList" => ("Spells", "Feitiços"),
        "SphereList" => ("Spheres", "Esferas"),
        "StatusList" => ("Status", "Status"),
        "TemperList" => ("Tempers", "Temperamentos"),
        "ThornList" => ("Thorns", "Espinhos"),
        "VisageList" => ("Visages", "Visages"),
        "WisdomList" => ("Wisdom", "Sabedoria"),
        _ => ("Traits", "Traços"),
    };
    localized(english, portuguese, language)
}

fn comparison_label(comparison: Comparison, language: Language) -> &'static str {
    match comparison {
        Comparison::Equal => localized("equals", "é igual a", language),
        Comparison::Contains => localized("contains", "contém", language),
        Comparison::AtLeast => localized("at least", "pelo menos", language),
        Comparison::AtMost => localized("at most", "no máximo", language),
        Comparison::Exists => localized("is present", "está disponível", language),
    }
}

fn clause_description(clause: &Clause, include_players: bool, language: Language) -> String {
    let negation = if clause.negate {
        localized("NOT ", "NÃO ", language)
    } else {
        ""
    };

    if clause.field == Field::Player && !include_players {
        return format!(
            "{negation}{} {} ({})",
            field_label(Field::Player, language),
            comparison_label(clause.comparison, language),
            localized("private filter", "filtro privado", language)
        );
    }

    match clause.field {
        Field::Name
        | Field::Family
        | Field::Player
        | Field::Status
        | Field::Group
        | Field::Subgroup => match clause.comparison {
            Comparison::Exists => format!(
                "{negation}{} {}",
                field_label(clause.field, language),
                comparison_label(clause.comparison, language)
            ),
            _ => format!(
                "{negation}{} {} \"{}\"",
                field_label(clause.field, language),
                comparison_label(clause.comparison, language),
                clause.value
            ),
        },
        Field::Trait => {
            let category = list_label(&clause.value, language);
            match clause.comparison {
                Comparison::Exists => format!(
                    "{negation}{}: {} \"{}\"",
                    category,
                    localized("has trait", "tem o traço", language),
                    clause.trait_name
                ),
                Comparison::Equal | Comparison::AtLeast | Comparison::AtMost => format!(
                    "{negation}{}: \"{}\" {} {}",
                    category,
                    clause.trait_name,
                    comparison_label(clause.comparison, language),
                    clause.number
                ),
                Comparison::Contains => String::new(),
            }
        }
        Field::Npc => format!(
            "{negation}{} {} {}",
            field_label(Field::Npc, language),
            comparison_label(clause.comparison, language),
            if clause.number == 1 {
                localized("Yes", "Sim", language)
            } else {
                localized("No", "Não", language)
            }
        ),
    }
}

fn append_header(
    out: &mut String,
    snapshot: &Snapshot,
    language: Language,
    report_name: &str,
) -> Result<()> {
    let fallback_title = report_name;
    let title = if snapshot.title.trim().is_empty() {
        fallback_title
    } else {
        snapshot.title.as_str()
    };
    let lang = match language {
        Language::English => "en",
        Language::Portuguese => "pt-BR",
    };

    append(out, "<!doctype html><html lang=\"")?;
    append(out, lang)?;
    append(
        out,
        "\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\"><title>",
    )?;
    append_escaped(out, title)?;
    append(out, " — ")?;
    append_escaped(out, report_name)?;
    append(
        out,
        "</title><style>body{font:16px/1.45 system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;color:#17212b}h1,h2,h3{line-height:1.2}section{margin:1.5rem 0}table{border-collapse:collapse;width:100%;margin:.5rem 0 1rem;table-layout:fixed}th,td{border:1px solid #9ba8b3;padding:.4rem .55rem;text-align:left;vertical-align:top;white-space:pre-wrap;overflow-wrap:anywhere}th{background:#eef2f5}thead{display:table-header-group}tr{break-inside:avoid}.meta,.note,.empty{color:#44515e}.criteria li{margin:.35rem 0}@page{size:auto;margin:14mm}@media print{body{max-width:none;margin:0;padding:0;color:#000}section{break-inside:auto}h1,h2,h3{break-after:avoid}}</style></head><body><header><h1>",
    )?;
    append_escaped(out, title)?;
    append(out, "</h1><p>")?;
    append_escaped(out, report_name)?;
    append(out, "</p><p class=\"private\">")?;
    append_escaped(
        out,
        localized("Private Storyteller report", "Relatório privado", language),
    )?;
    append(out, "</p><p class=\"meta\">")?;
    append_escaped(
        out,
        localized("Current snapshot", "Instantâneo atual", language),
    )?;
    append(out, " · ")?;
    append_escaped(out, localized("Revision", "Revisão", language))?;
    append(out, ": ")?;
    append(out, &snapshot.revision.to_string())?;
    append(out, " · ")?;
    append_escaped(
        out,
        localized("Total characters", "Total de personagens", language),
    )?;
    append(out, ": ")?;
    append(out, &snapshot.rows.len().to_string())?;
    append(out, " · ")?;
    append_escaped(
        out,
        localized("Match mode", "Modo de correspondência", language),
    )?;
    append(out, ": ")?;
    append_escaped(
        out,
        match snapshot.query.mode {
            Mode::All => localized("All criteria", "Todos os critérios", language),
            Mode::Any => localized("Any criterion", "Qualquer critério", language),
        },
    )?;
    append(out, "</p></header>")
}

fn append_criteria(out: &mut String, snapshot: &Snapshot, language: Language) -> Result<()> {
    append(out, "<section><h2>")?;
    append_escaped(out, localized("Criteria", "Critérios", language))?;
    append(out, "</h2>")?;
    if snapshot.query.clauses.is_empty() {
        append(out, "<p class=\"note\">")?;
        append_escaped(
            out,
            localized(
                "Includes all saved characters.",
                "Inclui todos os personagens salvos.",
                language,
            ),
        )?;
        append(out, "</p></section>")?;
        return Ok(());
    }

    append(out, "<p>")?;
    append_escaped(
        out,
        match snapshot.query.mode {
            Mode::All => localized("All of these", "Todos estes", language),
            Mode::Any => localized("Any of these", "Qualquer um destes", language),
        },
    )?;
    append(out, "</p><ul class=\"criteria\">")?;
    for clause in &snapshot.query.clauses {
        append(out, "<li>")?;
        append_escaped(
            out,
            &clause_description(clause, snapshot.include_players, language),
        )?;
        append(out, "</li>")?;
    }
    append(out, "</ul></section>")
}

pub(crate) fn append_search_rows(
    out: &mut String,
    snapshot: &Snapshot,
    language: Language,
) -> Result<()> {
    append(out, "<section><h2>")?;
    append_escaped(
        out,
        localized(
            "Matching characters",
            "Personagens correspondentes",
            language,
        ),
    )?;
    append(out, "</h2><table><thead><tr>")?;
    for heading in [
        localized("Name", "Nome", language),
        localized("Family", "Família", language),
        localized("Status", "Status", language),
        localized("Group", "Grupo", language),
        localized("Subgroup", "Subgrupo", language),
        "NPC",
    ] {
        append(out, "<th scope=\"col\">")?;
        append_escaped(out, heading)?;
        append(out, "</th>")?;
    }
    if snapshot.include_players {
        append(out, "<th scope=\"col\">")?;
        append_escaped(out, localized("Player", "Jogador", language))?;
        append(out, "</th>")?;
    }
    append(out, "</tr></thead><tbody>")?;
    if snapshot.rows.is_empty() {
        append(out, "<tr><td colspan=\"")?;
        append(out, if snapshot.include_players { "7" } else { "6" })?;
        append(out, "\" class=\"empty\">")?;
        append_escaped(
            out,
            localized(
                "No matching characters.",
                "Nenhum personagem correspondente.",
                language,
            ),
        )?;
        append(out, "</td></tr>")?;
    } else {
        for row in &snapshot.rows {
            append(out, "<tr>")?;
            for value in [
                display_value(&row.name, language),
                display_value(&row.family, language),
                display_value(&row.status, language),
                display_value(&row.group, language),
                display_value(&row.subgroup, language),
                match row.npc {
                    Some(true) => localized("Yes", "Sim", language),
                    Some(false) => localized("No", "Não", language),
                    None => unavailable(language),
                },
            ] {
                append(out, "<td>")?;
                append_escaped(out, value)?;
                append(out, "</td>")?;
            }
            if snapshot.include_players {
                append(out, "<td>")?;
                append_escaped(out, display_value(&row.player, language))?;
                append(out, "</td>")?;
            }
            append(out, "</tr>")?;
        }
    }
    append(out, "</tbody></table></section>")
}

fn string_buckets<'a>(
    rows: &'a [Row],
    value: impl Fn(&'a Row) -> &'a str,
    missing_label: &str,
) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<Option<String>, (String, usize)> = BTreeMap::new();
    for row in rows {
        let raw = value(row).trim();
        if raw.is_empty() {
            let entry = counts
                .entry(None)
                .or_insert_with(|| (missing_label.to_owned(), 0));
            entry.1 += 1;
        } else {
            let key = folded(raw);
            let entry = counts
                .entry(Some(key))
                .or_insert_with(|| (raw.to_owned(), 0));
            entry.1 += 1;
            if raw < entry.0.as_str() {
                entry.0 = raw.to_owned();
            }
        }
    }
    counts.into_values().collect()
}

fn npc_buckets(rows: &[Row], language: Language) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::new();
    for row in rows {
        let label = match row.npc {
            Some(false) => localized("No", "Não", language),
            Some(true) => localized("Yes", "Sim", language),
            None => unavailable(language),
        };
        *counts.entry(label.to_owned()).or_insert(0usize) += 1;
    }
    let mut buckets: Vec<_> = counts.into_iter().collect();
    buckets.sort_by(|a, b| {
        let order = |label: &str| {
            if label == localized("No", "Não", language) {
                0
            } else if label == localized("Yes", "Sim", language) {
                1
            } else {
                2
            }
        };
        order(&a.0).cmp(&order(&b.0)).then_with(|| a.0.cmp(&b.0))
    });
    buckets
}

fn append_statistics_table(
    out: &mut String,
    dimension: &str,
    buckets: &[(String, usize)],
    total: usize,
    language: Language,
) -> Result<()> {
    append(out, "<section><h3>")?;
    append_escaped(out, dimension)?;
    append(out, "</h3><table><thead><tr><th scope=\"col\">")?;
    append_escaped(out, localized("Value", "Valor", language))?;
    append(out, "</th><th scope=\"col\">")?;
    append_escaped(out, localized("Count", "Contagem", language))?;
    append(out, "</th><th scope=\"col\">")?;
    append_escaped(
        out,
        localized(
            "Percent of matched characters",
            "Percentual de personagens correspondentes",
            language,
        ),
    )?;
    append(out, "</th></tr></thead><tbody>")?;
    if buckets.is_empty() {
        append(out, "<tr><td colspan=\"3\" class=\"empty\">")?;
        append_escaped(
            out,
            localized(
                "No matching characters.",
                "Nenhum personagem correspondente.",
                language,
            ),
        )?;
        append(out, "</td></tr>")?;
    } else {
        for (label, count) in buckets {
            let percent = if total == 0 {
                0.0
            } else {
                (*count as f64) * 100.0 / (total as f64)
            };
            let percent = format!("{percent:.1}");
            let percent = if language == Language::Portuguese {
                percent.replace('.', ",")
            } else {
                percent
            };
            append(out, "<tr><td>")?;
            append_escaped(out, label)?;
            append(out, "</td><td>")?;
            append(out, &count.to_string())?;
            append(out, "</td><td>")?;
            append(out, &percent)?;
            append(out, "%</td></tr>")?;
        }
    }
    append(out, "</tbody></table></section>")
}

fn append_statistics(out: &mut String, snapshot: &Snapshot, language: Language) -> Result<()> {
    append(out, "<section><h2>")?;
    append_escaped(
        out,
        localized(
            "Population statistics",
            "Estatísticas populacionais",
            language,
        ),
    )?;
    append(out, "</h2><p class=\"note\">")?;
    append_escaped(
        out,
        localized(
            "Counts and percentages describe this current library snapshot.",
            "As contagens e percentuais descrevem este instantâneo atual da biblioteca.",
            language,
        ),
    )?;
    append(out, "</p></section>")?;

    let missing = unavailable(language);
    let family = string_buckets(&snapshot.rows, |row| &row.family, missing);
    let status = string_buckets(&snapshot.rows, |row| &row.status, missing);
    let group = string_buckets(&snapshot.rows, |row| &row.group, missing);
    let npc = npc_buckets(&snapshot.rows, language);
    let total = snapshot.rows.len();
    append_statistics_table(
        out,
        localized("Family", "Família", language),
        &family,
        total,
        language,
    )?;
    append_statistics_table(
        out,
        localized("Status", "Status", language),
        &status,
        total,
        language,
    )?;
    append_statistics_table(
        out,
        localized("Group", "Grupo", language),
        &group,
        total,
        language,
    )?;
    append_statistics_table(out, "NPC", &npc, total, language)
}

/// Renders a bounded, escaped, offline HTML result or current-population report.
///
/// Player names and Player clause values are omitted whenever `include_players` is false.
pub(crate) fn report_start(snapshot: &Snapshot, language: Language, name: &str) -> Result<String> {
    if snapshot.revision < 0
        || snapshot.rows.len() > MAX_REPORT_ROWS
        || !report_text_valid(&snapshot.title)
        || snapshot.rows.iter().any(|row| {
            !report_text_valid(&row.name)
                || !report_text_valid(&row.family)
                || !report_text_valid(&row.status)
                || !report_text_valid(&row.group)
                || !report_text_valid(&row.subgroup)
                || (snapshot.include_players && !report_text_valid(&row.player))
        })
    {
        return Err(report_error());
    }
    validate(&snapshot.query)?;

    let mut out = String::new();
    append_header(&mut out, snapshot, language, name)?;
    append_criteria(&mut out, snapshot, language)?;
    Ok(out)
}

pub fn html(snapshot: &Snapshot, language: Language, statistics: bool) -> Result<String> {
    let name = if statistics {
        localized(
            "Population statistics",
            "Estatísticas populacionais",
            language,
        )
    } else {
        localized("Character search", "Busca de personagens", language)
    };
    let mut out = report_start(snapshot, language, name)?;
    if statistics {
        append_statistics(&mut out, snapshot, language)?;
    } else {
        append_search_rows(&mut out, snapshot, language)?;
    }
    append(&mut out, "</body></html>")?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str) -> Row {
        Row {
            id: "internal-row-id".into(),
            name: name.into(),
            family: "Mage".into(),
            status: "Active".into(),
            group: "North".into(),
            subgroup: "Blue".into(),
            player: "Player One".into(),
            npc: Some(false),
        }
    }

    fn input(row: Row) -> Input {
        Input {
            row,
            traits: BTreeMap::new(),
        }
    }

    fn text_clause(field: Field, comparison: Comparison, value: &str, negate: bool) -> Clause {
        Clause {
            field,
            comparison,
            negate,
            value: value.into(),
            trait_name: String::new(),
            number: 0,
        }
    }

    fn query(mode: Mode, clauses: Vec<Clause>) -> Query {
        Query { mode, clauses }
    }

    #[test]
    fn all_any_and_clause_negation_work() {
        let candidate = input(row("Mira Vale"));
        let clauses = vec![
            text_clause(Field::Name, Comparison::Contains, "mira", false),
            text_clause(Field::Family, Comparison::Equal, "Vampire", false),
        ];
        assert!(!matches(&query(Mode::All, clauses.clone()), &candidate));
        assert!(matches(&query(Mode::Any, clauses), &candidate));

        let not_north = text_clause(Field::Group, Comparison::Equal, "South", true);
        assert!(matches(&query(Mode::All, vec![not_north]), &candidate));
    }

    #[test]
    fn missing_values_stay_nonmatches_under_not() {
        let mut candidate = row("Mira");
        candidate.group.clear();
        candidate.npc = None;
        let mut candidate = input(candidate);
        candidate.traits.insert(
            "DisciplineList".into(),
            vec![Trait {
                name: "Auspex".into(),
                rating: None,
            }],
        );

        assert!(!matches(
            &query(
                Mode::Any,
                vec![text_clause(
                    Field::Group,
                    Comparison::Contains,
                    "north",
                    true
                )]
            ),
            &candidate
        ));
        assert!(!matches(
            &query(
                Mode::Any,
                vec![Clause {
                    field: Field::Npc,
                    comparison: Comparison::Equal,
                    negate: true,
                    value: String::new(),
                    trait_name: String::new(),
                    number: 1,
                }]
            ),
            &candidate
        ));
        assert!(!matches(
            &query(
                Mode::Any,
                vec![Clause {
                    field: Field::Trait,
                    comparison: Comparison::AtLeast,
                    negate: true,
                    value: "DisciplineList".into(),
                    trait_name: "Auspex".into(),
                    number: 2,
                }]
            ),
            &candidate
        ));
        assert!(matches(
            &query(
                Mode::Any,
                vec![Clause {
                    field: Field::Trait,
                    comparison: Comparison::Exists,
                    negate: false,
                    value: "DisciplineList".into(),
                    trait_name: "Auspex".into(),
                    number: 0,
                }]
            ),
            &candidate
        ));
    }

    #[test]
    fn validates_bounds_and_supported_shapes() {
        assert!(validate(&query(Mode::All, vec![])).is_ok());
        assert!(
            validate(&query(
                Mode::All,
                vec![text_clause(Field::Name, Comparison::Equal, "Mira", false)]
            ))
            .is_ok()
        );

        let too_many = vec![text_clause(Field::Name, Comparison::Contains, "x", false); 33];
        assert!(validate(&query(Mode::Any, too_many)).is_err());
        assert!(
            validate(&query(
                Mode::All,
                vec![text_clause(
                    Field::Name,
                    Comparison::Equal,
                    &"x".repeat(1_025),
                    false
                )]
            ))
            .is_err()
        );
        assert!(
            validate(&query(
                Mode::All,
                vec![text_clause(
                    Field::Name,
                    Comparison::Contains,
                    "bad\0value",
                    false
                )]
            ))
            .is_err()
        );
        assert!(
            validate(&query(
                Mode::All,
                vec![text_clause(Field::Name, Comparison::AtLeast, "3", false)]
            ))
            .is_err()
        );
        assert!(
            validate(&query(
                Mode::All,
                vec![Clause {
                    field: Field::Trait,
                    comparison: Comparison::Equal,
                    negate: false,
                    value: "UnknownList".into(),
                    trait_name: "Auspex".into(),
                    number: 2,
                }]
            ))
            .is_err()
        );
        assert!(
            validate(&query(
                Mode::All,
                vec![Clause {
                    field: Field::Npc,
                    comparison: Comparison::Equal,
                    negate: false,
                    value: String::new(),
                    trait_name: String::new(),
                    number: 2,
                }]
            ))
            .is_err()
        );
    }

    #[test]
    fn text_and_trait_matching_support_unicode_case() {
        let candidate = input(Row {
            name: "Mágico".into(),
            ..row("unused")
        });
        assert!(matches(
            &query(
                Mode::All,
                vec![text_clause(Field::Name, Comparison::Contains, "MÁG", false)]
            ),
            &candidate
        ));

        let mut candidate = candidate;
        candidate.traits.insert(
            "AbilityList".into(),
            vec![Trait {
                name: "Café".into(),
                rating: Some(2),
            }],
        );
        assert!(matches(
            &query(
                Mode::All,
                vec![Clause {
                    field: Field::Trait,
                    comparison: Comparison::AtLeast,
                    negate: false,
                    value: "AbilityList".into(),
                    trait_name: "CAFÉ".into(),
                    number: 2,
                }]
            ),
            &candidate
        ));
    }

    #[test]
    fn empty_query_matches_all_supplied_rows() {
        let candidate = input(row("Mira"));
        assert!(matches(&query(Mode::Any, vec![]), &candidate));
    }

    #[test]
    fn html_escapes_data_and_hides_players_and_ids_when_disabled() {
        let snapshot = Snapshot {
            title: "Chronicle <script>alert(\"x\")</script>".into(),
            revision: 9,
            query: query(
                Mode::All,
                vec![text_clause(
                    Field::Player,
                    Comparison::Contains,
                    "PLAYER_QUERY_SECRET",
                    false,
                )],
            ),
            include_players: false,
            rows: vec![Row {
                id: "INTERNAL_ID_SECRET".into(),
                name: "Mira <img src=x>".into(),
                player: "PLAYER_ROW_SECRET".into(),
                ..row("unused")
            }],
        };
        let rendered = html(&snapshot, Language::English, false).unwrap();
        assert!(rendered.contains("default-src 'none'"));
        assert!(rendered.contains("&lt;script&gt;"));
        assert!(rendered.contains("&lt;img src=x&gt;"));
        assert!(rendered.contains("private filter"));
        assert!(!rendered.contains("<script>"));
        assert!(!rendered.contains("PLAYER_QUERY_SECRET"));
        assert!(!rendered.contains("PLAYER_ROW_SECRET"));
        assert!(!rendered.contains("INTERNAL_ID_SECRET"));
    }

    #[test]
    fn statistics_use_current_rows_and_label_unknown_dimensions() {
        let snapshot = Snapshot {
            title: "Population".into(),
            revision: 21,
            query: query(Mode::All, vec![]),
            include_players: false,
            rows: vec![
                Row {
                    family: "Vampire".into(),
                    status: "Active".into(),
                    group: "North".into(),
                    npc: Some(false),
                    ..row("A")
                },
                Row {
                    family: "vampire".into(),
                    status: "Active".into(),
                    group: "North".into(),
                    npc: Some(true),
                    ..row("B")
                },
                Row {
                    family: "Mage".into(),
                    status: String::new(),
                    group: "South".into(),
                    npc: None,
                    ..row("C")
                },
                Row {
                    family: String::new(),
                    status: "Active".into(),
                    group: "North".into(),
                    npc: Some(true),
                    ..row("D")
                },
            ],
        };
        let rendered = html(&snapshot, Language::English, true).unwrap();
        assert!(rendered.contains("Current snapshot"));
        assert!(rendered.contains("Revision</strong>") || rendered.contains("Revision: 21"));
        assert!(rendered.contains("50.0%"));
        assert!(rendered.contains("Unavailable"));
        assert!(rendered.contains("this current library snapshot"));

        let portuguese = html(&snapshot, Language::Portuguese, true).unwrap();
        assert!(portuguese.contains("lang=\"pt-BR\""));
        assert!(portuguese.contains("Indisponível"));
        assert!(portuguese.contains(",0%"));
    }

    #[test]
    fn report_row_and_output_bounds_fail_closed() {
        let mut snapshot = Snapshot {
            title: "Bounded".into(),
            revision: 0,
            query: query(Mode::All, vec![]),
            include_players: false,
            rows: vec![row("Mira"); MAX_REPORT_ROWS + 1],
        };
        assert!(html(&snapshot, Language::English, false).is_err());
        snapshot.rows.truncate(1);
        snapshot.title = "x".repeat(MAX_REPORT_TEXT_BYTES + 1);
        assert!(html(&snapshot, Language::English, false).is_err());
    }
}
