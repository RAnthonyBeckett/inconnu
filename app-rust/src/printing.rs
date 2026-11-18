//! Pure printable character-sheet projection. This module does not mutate records,
//! reveal unknown source data, or add game mechanics or approval decisions.
use crate::{
    importer,
    model::{BuildKind, RecordView, Result, error},
    sheet,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const LIBRARY_PROMPT: &str =
    "[For more detailed info check your library for \"Laws of the Night (Revised).\"]";
const SOURCE_ONLY_WARNING_EN: &str = "Source references list cited power names and pages only; they contain no mechanics summary and do not imply approval.";
const SOURCE_ONLY_WARNING_PT: &str = "As referências listam apenas nomes de poderes e páginas; não resumem regras nem implicam aprovação.";
const OMITTED_WARNING_EN: &str =
    "Some sheet fields were omitted because their structure is unsupported.";
const OMITTED_WARNING_PT: &str =
    "Alguns campos da ficha foram omitidos porque têm uma estrutura não compatível.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocLanguage {
    #[default]
    English,
    Portuguese,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintScope {
    #[default]
    Public,
    PrivateStoryteller,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub language: DocLanguage,
    pub scope: PrintScope,
    pub large_text: bool,
    pub include_notes: bool,
    pub include_temporary: bool,
    pub include_references: bool,
    pub include_simple_mechanics: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_owbn_mechanics: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_challenge_math: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            language: DocLanguage::English,
            scope: PrintScope::Public,
            large_text: false,
            include_notes: false,
            include_temporary: true,
            include_references: true,
            include_simple_mechanics: false,
            include_owbn_mechanics: false,
            include_challenge_math: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetDocument {
    pub title: String,
    pub kind: String,
    pub status: String,
    pub player_name: String,
    pub scope: PrintScope,
    pub build: Option<BuildKind>,
    pub sections: Vec<Section>,
    pub references: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published_references: Vec<PublishedReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub challenge_pools: Vec<crate::challenge_math::Breakdown>,
    pub warnings: Vec<String>,
    pub options: Options,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub name: String,
    pub value: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub family: String,
    pub power: String,
    pub book: String,
    pub edition: String,
    pub printed_page: u32,
    pub pdf_page: u32,
    pub library_prompt: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedReference {
    pub recorded_name: String,
    pub recorded_value: String,
    pub source: crate::published_powers::Reference,
}

fn localized(key: &str, language: DocLanguage) -> String {
    let (en, pt) = match key {
        "Identity" => ("Identity", "Identidade"),
        "Resources" => ("Resources", "Recursos"),
        "Notes" => ("Notes", "Anotações"),
        "Storyteller notes" => ("Storyteller notes", "Anotações do Narrador"),
        "Source references" => ("Source references", "Referências de fonte"),
        "Clan" => ("Clan", "Clã"),
        "Sect" => ("Sect", "Seita"),
        "Nature" => ("Nature", "Natureza"),
        "Demeanor" => ("Demeanor", "Comportamento"),
        "Tribe" => ("Tribe", "Tribo"),
        "Kith" => ("Kith", "Kith"),
        "Guild" => ("Guild", "Guilda"),
        "Faction" => ("Faction", "Facção"),
        "Title" => ("Title", "Título"),
        "Path" => ("Humanity / Path", "Humanidade / Trilha"),
        "Player" => ("Player", "Jogador"),
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
        _ => {
            let base = key.strip_suffix("List").unwrap_or(key);
            return humanize(base);
        }
    };
    match language {
        DocLanguage::English => en.to_string(),
        DocLanguage::Portuguese => pt.to_string(),
    }
}

fn humanize(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    let mut previous_lower = false;
    for character in value.chars() {
        if character.is_uppercase() && previous_lower {
            out.push(' ');
        }
        out.push(character);
        previous_lower = character.is_lowercase() || character.is_ascii_digit();
    }
    out
}

fn top_text(value: &str, scope: PrintScope) -> String {
    if scope == PrintScope::Public {
        importer::redact(&Value::String(value.into()))
            .as_str()
            .unwrap_or_default()
            .to_string()
    } else {
        value.to_string()
    }
}

fn public_fields(fields: &Map<String, Value>, scope: PrintScope) -> Value {
    let value = Value::Object(fields.clone());
    if scope == PrintScope::Public {
        importer::redact(&value)
    } else {
        value
    }
}

fn list_rows(value: &Value, include_notes: bool) -> Option<Vec<Row>> {
    let traits = value.get("Traits")?.as_array()?;
    let mut rows = Vec::with_capacity(traits.len());
    for item in traits {
        let object = item.as_object()?;
        let name = object.get("Name")?.as_str()?;
        let value = object.get("Value")?.as_str()?;
        let note = if include_notes {
            object
                .get("Note")
                .and_then(Value::as_str)
                .filter(|note| !note.is_empty())
                .map(ToString::to_string)
        } else {
            None
        };
        rows.push(Row {
            name: name.into(),
            value: value.into(),
            note,
        });
    }
    Some(rows)
}

fn resource_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) if value.as_i64().is_some() => Some(value.to_string()),
        _ => None,
    }
}

fn reference_from_definition(
    power: &crate::power_catalogue::Definition,
    profiles: &crate::creature_profiles::Catalogue,
) -> Option<Reference> {
    if !power.source_reviewed {
        return None;
    }
    let (book, edition, prompt) = if power.legacy_power.is_some() {
        (
            "Laws of the Night".to_owned(),
            "Revised".to_owned(),
            LIBRARY_PROMPT.to_owned(),
        )
    } else {
        let source = profiles
            .profile(&power.template_scope)?
            .checked_cost_source()
            .ok()?;
        if source.original_pdf_sha256 != power.original_pdf_sha256 {
            return None;
        }
        let prompt = format!(
            "[For more detailed info check your library for \"{} ({}).\"]",
            source.book, source.edition
        );
        (source.book, source.edition, prompt)
    };
    Some(Reference {
        family: power.family.clone(),
        power: power.name.clone(),
        book,
        edition,
        printed_page: power.printed_page,
        pdf_page: power.pdf_page_1based,
        library_prompt: prompt,
    })
}
fn reference_list(
    kind: &str,
    fields: &Map<String, Value>,
    verified: &[String],
    catalogue: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Vec<Reference> {
    if !sheet::is_character_kind(kind) {
        return Vec::new();
    }
    catalogue
        .entries
        .iter()
        .filter(|power| {
            power.template_scope == kind
                && verified.contains(&power.stable_id)
                && profiles
                    .profile(kind)
                    .is_some_and(|profile| profile.owns_power_list(&power.target_list))
                && fields
                    .get(&power.target_list)
                    .and_then(|list| list_rows(list, false))
                    .is_some_and(|rows| rows.iter().any(|r| r.name == power.name))
        })
        .filter_map(|power| reference_from_definition(power, profiles))
        .collect()
}
pub(crate) fn mapped_reference(id: &str) -> Option<Reference> {
    reference_from_definition(
        crate::power_catalogue::builtin().lookup(id)?,
        crate::creature_profiles::builtin(),
    )
}

pub fn document(
    view: &RecordView,
    verified_power_ids: &[String],
    options: &Options,
) -> Result<SheetDocument> {
    document_from_catalogue(
        view,
        verified_power_ids,
        options,
        crate::power_catalogue::builtin(),
        crate::creature_profiles::builtin(),
    )
}
/// Shared projection seam for reviewed catalogue data and fabricated regression fixtures.
pub fn document_from_catalogue(
    view: &RecordView,
    verified_power_ids: &[String],
    options: &Options,
    catalogue: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<SheetDocument> {
    if !sheet::is_character_kind(&view.kind) {
        return Err(error(
            "PRINT_UNSUPPORTED_CHARACTER",
            "Choose a supported character sheet before preparing a print document.",
        ));
    }

    let mut effective_options = options.clone();
    if effective_options.scope == PrintScope::Public {
        effective_options.include_notes = false;
    }
    let fields = public_fields(&view.fields, effective_options.scope);
    let Some(fields) = fields.as_object() else {
        return Err(error(
            "PRINT_INVALID_VIEW",
            "The projected sheet fields could not be read safely.",
        ));
    };
    let mut warnings = Vec::new();
    let mut omitted_unsupported = false;
    let mut sections = Vec::new();

    let mut identity_rows = Vec::new();
    for &field in sheet::IDENTITY_FIELDS {
        if let Some(value) = fields.get(field) {
            if let Some(value) = value.as_str() {
                if !value.is_empty() {
                    identity_rows.push(Row {
                        name: localized(field, effective_options.language),
                        value: value.into(),
                        note: None,
                    });
                }
            } else {
                omitted_unsupported = true;
            }
        }
    }
    if !view.player_name.is_empty() {
        identity_rows.push(Row {
            name: localized("Player", effective_options.language),
            value: top_text(&view.player_name, effective_options.scope),
            note: None,
        });
    }
    if !identity_rows.is_empty() {
        sections.push(Section {
            title: localized("Identity", effective_options.language),
            rows: identity_rows,
        });
    }

    let mut resource_rows = Vec::new();
    for field in sheet::RESOURCE_FIELDS {
        if !effective_options.include_temporary && field.starts_with("Temp") {
            continue;
        }
        if let Some(value) = fields.get(*field) {
            if let Some(value) = resource_text(value) {
                resource_rows.push(Row {
                    name: resource_label(field, effective_options.language),
                    value,
                    note: None,
                });
            } else {
                omitted_unsupported = true;
            }
        }
    }
    if !resource_rows.is_empty() {
        sections.push(Section {
            title: localized("Resources", effective_options.language),
            rows: resource_rows,
        });
    }

    for field in sheet::LIST_FIELDS {
        if let Some(value) = fields.get(*field) {
            match list_rows(
                value,
                effective_options.scope == PrintScope::PrivateStoryteller
                    && effective_options.include_notes,
            ) {
                Some(rows) => sections.push(Section {
                    title: localized(field, effective_options.language),
                    rows,
                }),
                None => omitted_unsupported = true,
            }
        }
    }

    if effective_options.scope == PrintScope::PrivateStoryteller
        && effective_options.include_notes
        && !view.notes.is_empty()
    {
        sections.push(Section {
            title: localized("Storyteller notes", effective_options.language),
            rows: vec![Row {
                name: localized("Notes", effective_options.language),
                value: view.notes.clone(),
                note: None,
            }],
        });
    }

    let references = if effective_options.include_references {
        warnings.push(match effective_options.language {
            DocLanguage::English => SOURCE_ONLY_WARNING_EN.to_string(),
            DocLanguage::Portuguese => SOURCE_ONLY_WARNING_PT.to_string(),
        });
        reference_list(&view.kind, fields, verified_power_ids, catalogue, profiles)
    } else {
        warnings.push(match effective_options.language {
            DocLanguage::English => "Source references were omitted by the print options.".into(),
            DocLanguage::Portuguese => {
                "As referências de fonte foram omitidas pelas opções de impressão.".into()
            }
        });
        Vec::new()
    };
    if omitted_unsupported {
        warnings.push(match effective_options.language {
            DocLanguage::English => OMITTED_WARNING_EN.to_string(),
            DocLanguage::Portuguese => OMITTED_WARNING_PT.to_string(),
        });
    }

    Ok(SheetDocument {
        challenge_pools: Vec::new(),
        published_references: Vec::new(),
        title: top_text(&view.name, effective_options.scope),
        kind: top_text(&view.kind, effective_options.scope),
        status: top_text(&view.status, effective_options.scope),
        player_name: top_text(&view.player_name, effective_options.scope),
        scope: effective_options.scope,
        build: view.build,
        sections,
        references,
        warnings,
        options: effective_options,
    })
}

fn resource_label(field: &str, language: DocLanguage) -> String {
    if let Some(base) = field.strip_prefix("Temp") {
        let label = resource_label(base, language);
        return match language {
            DocLanguage::English => format!("Temporary {label}"),
            DocLanguage::Portuguese => format!("{label} temporário"),
        };
    }
    if language == DocLanguage::Portuguese {
        let translated = match field {
            "Generation" => Some("Geração"),
            "Blood" => Some("Sangue"),
            "Willpower" => Some("Força de Vontade"),
            "Conscience" => Some("Consciência"),
            "Conviction" => Some("Convicção"),
            "SelfControl" => Some("Autocontrole"),
            "Instinct" => Some("Instinto"),
            "Courage" => Some("Coragem"),
            "PathTraits" => Some("Características de Humanidade / Trilha"),
            _ => None,
        };
        if let Some(label) = translated {
            return label.into();
        }
    }
    humanize(field)
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(character),
        }
    }
    out
}

fn watermark(doc: &SheetDocument) -> String {
    let mut parts = Vec::new();
    parts.push(match doc.scope {
        PrintScope::Public => match doc.options.language {
            DocLanguage::English => "PUBLIC SHEET".to_string(),
            DocLanguage::Portuguese => "FICHA PÚBLICA".to_string(),
        },
        PrintScope::PrivateStoryteller => match doc.options.language {
            DocLanguage::English => "PRIVATE STORYTELLER COPY".to_string(),
            DocLanguage::Portuguese => "CÓPIA PRIVADA DO NARRADOR".to_string(),
        },
    });
    if let Some(build) = doc.build {
        parts.push(match (doc.options.language, build) {
            (DocLanguage::English, BuildKind::Draft) => "DRAFT COPY".to_string(),
            (DocLanguage::English, BuildKind::Test) => "TEST COPY".to_string(),
            (DocLanguage::Portuguese, BuildKind::Draft) => "CÓPIA DE RASCUNHO".to_string(),
            (DocLanguage::Portuguese, BuildKind::Test) => "CÓPIA DE TESTE".to_string(),
        });
    }
    parts.push(match doc.options.language {
        DocLanguage::English => "REFERENCE ONLY · NO APPROVAL IMPLIED".to_string(),
        DocLanguage::Portuguese => "APENAS REFERÊNCIA · NÃO IMPLICA APROVAÇÃO".to_string(),
    });
    parts.join(" · ")
}

fn html_section(section: &Section) -> String {
    let mut out = String::new();
    out.push_str("<section class=\"sheet-section\"><h2>");
    out.push_str(&escape_html(&section.title));
    out.push_str("</h2>");
    if !section.rows.is_empty() {
        out.push_str("<dl>");
        for row in &section.rows {
            out.push_str("<div class=\"row\"><dt>");
            out.push_str(&escape_html(&row.name));
            out.push_str("</dt><dd>");
            out.push_str(&escape_html(&row.value));
            if let Some(note) = &row.note {
                out.push_str("<p class=\"note\">");
                out.push_str(&escape_html(note));
                out.push_str("</p>");
            }
            out.push_str("</dd></div>");
        }
        out.push_str("</dl>");
    }
    out.push_str("</section>");
    out
}

pub fn html(doc: &SheetDocument) -> Result<String> {
    let large = if doc.options.large_text {
        "large"
    } else {
        "normal"
    };
    let language = match doc.options.language {
        DocLanguage::English => "en",
        DocLanguage::Portuguese => "pt",
    };
    let labels = match doc.options.language {
        DocLanguage::English => (
            "Type",
            "Status",
            "Player",
            "Warnings",
            "printed p.",
            "PDF p.",
        ),
        DocLanguage::Portuguese => (
            "Tipo",
            "Status",
            "Jogador",
            "Avisos",
            "p. impressa",
            "PDF p.",
        ),
    };
    let mut out = format!(
        "<!doctype html><html lang=\"{language}\"><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><style>:root{{font-family:Arial,Helvetica,sans-serif;color:#17212b;background:#fff}}body{{max-width:960px;margin:2rem auto;padding:0 1.2rem;font-size:11pt;line-height:1.4}}body.large{{font-size:16pt}}h1{{font-size:1.7em;margin:.2em 0}}h2{{font-size:1.2em;border-bottom:1px solid #8895a3;padding-bottom:.2em;margin:.6em 0 .35em}}.watermark{{border:2px solid #526273;padding:.45em .7em;font-weight:700;letter-spacing:.06em;text-align:center;margin-bottom:1em}}.row{{display:grid;grid-template-columns:minmax(9em, 28%) 1fr;gap:.6em;border-bottom:1px solid #d7dde3;padding:.35em 0;break-inside:avoid;page-break-inside:avoid}}dt{{font-weight:700}}dd{{margin:0;overflow-wrap:anywhere}}.note,.warning{{white-space:pre-wrap;overflow-wrap:anywhere}}.note{{font-size:.92em;color:#3d4a57;margin:.3em 0 0}}.warning{{border-left:3px solid #a27116;padding:.35em .6em;margin:.35em 0}}.sheet-section{{break-inside:auto;page-break-inside:auto;margin:1em 0}}.references li{{margin:.55em 0;break-inside:avoid;page-break-inside:avoid}}@page{{size:A4;margin:14mm}}@media print{{body{{max-width:none;margin:0;padding:0}}.sheet-section{{margin:.5em 0}}h1,h2{{break-after:avoid-page;page-break-after:avoid}}}}
</style></head><body class=\"{large}\"><div class=\"watermark\">{}</div><h1>{}</h1><dl class=\"metadata\"><div class=\"row\"><dt>{}</dt><dd>{}</dd></div><div class=\"row\"><dt>{}</dt><dd>{}</dd></div><div class=\"row\"><dt>{}</dt><dd>{}</dd></div></dl>",
        escape_html(&doc.title),
        escape_html(&watermark(doc)),
        escape_html(&doc.title),
        labels.0,
        escape_html(&doc.kind),
        labels.1,
        escape_html(&doc.status),
        labels.2,
        escape_html(&doc.player_name),
    );
    for section in &doc.sections {
        out.push_str(&html_section(section));
    }
    if !doc.references.is_empty() {
        out.push_str("<section class=\"sheet-section references\"><h2>");
        out.push_str(&escape_html(&localized(
            "Source references",
            doc.options.language,
        )));
        out.push_str("</h2><ul>");
        for reference in &doc.references {
            out.push_str("<li><strong>");
            out.push_str(&escape_html(&reference.family));
            out.push_str(" · ");
            out.push_str(&escape_html(&reference.power));
            out.push_str("</strong><br>");
            out.push_str(&escape_html(&reference.book));
            out.push_str(" (");
            out.push_str(&escape_html(&reference.edition));
            out.push_str("), ");
            out.push_str(labels.4);
            out.push(' ');
            out.push_str(&reference.printed_page.to_string());
            out.push_str(" / ");
            out.push_str(labels.5);
            out.push(' ');
            out.push_str(&reference.pdf_page.to_string());
            out.push_str("<br><span>");
            out.push_str(&escape_html(&reference.library_prompt));
            out.push_str("</span></li>");
        }
        out.push_str("</ul></section>");
    }
    if !doc.published_references.is_empty() {
        let pt = doc.options.language == DocLanguage::Portuguese;
        out.push_str("<section class=\"sheet-section references\"><h2>");
        out.push_str(if pt {
            "Referências de níveis publicados"
        } else {
            "Published-level source references"
        });
        out.push_str("</h2><p>");
        out.push_str(if pt {"Níveis publicados são separados de níveis MET, gastos de XP e aprovação de aprendizado."}else{"Published levels are separate from MET tiers, XP spending and learning approval."});
        out.push_str("</p><ul>");
        for citation in &doc.published_references {
            let reference = &citation.source;
            out.push_str("<li>");
            out.push_str(if pt {
                "Característica salva: "
            } else {
                "Recorded trait: "
            });
            out.push_str(&escape_html(&citation.recorded_name));
            out.push_str(" · ");
            out.push_str(&escape_html(&citation.recorded_value));
            out.push_str("<br>");
            out.push_str("<strong>");
            out.push_str(&escape_html(&reference.power.discipline));
            if let Some(path) = &reference.power.path {
                out.push_str(" / ");
                out.push_str(&escape_html(path));
            }
            out.push_str(" · ");
            out.push_str(&escape_html(&reference.power.name));
            out.push_str("</strong> · ");
            out.push_str(if pt {
                "nível publicado "
            } else {
                "published level "
            });
            out.push_str(&reference.power.level.to_string());
            out.push_str("<br>");
            out.push_str(&escape_html(&reference.book_title));
            out.push_str(" · ");
            out.push_str(&escape_html(reference.edition.as_str()));
            if let Some(page) = reference.power.printed_page {
                out.push_str(" · ");
                out.push_str(labels.4);
                out.push(' ');
                out.push_str(&page.to_string());
            }
            out.push_str(" / ");
            out.push_str(labels.5);
            out.push(' ');
            out.push_str(&reference.power.pdf_page.to_string());
            out.push_str("<br>");
            out.push_str(&escape_html(&format!(
                "[For more detailed info check your library for \"{}.\"]",
                reference.book_title
            )));
            out.push_str("</li>");
        }
        out.push_str("</ul></section>");
    }
    if !doc.warnings.is_empty() {
        out.push_str("<section class=\"sheet-section\"><h2>");
        out.push_str(labels.3);
        out.push_str("</h2>");
        for warning in &doc.warnings {
            out.push_str("<p class=\"warning\">");
            out.push_str(&escape_html(warning));
            out.push_str("</p>");
        }
        out.push_str("</section>");
    }
    out.push_str("</body></html>");
    Ok(out)
}
