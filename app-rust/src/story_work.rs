//! Private downtime drafts and scoped print projection; no recipient evaluation.
use crate::{
    actions::Action,
    model::{Language, Result, error},
    rumors::Rumor,
    world_card::escape,
    xp,
};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Action,
    Rumor,
}

impl Kind {
    pub fn record_kind(self) -> &'static str {
        match self {
            Self::Action => "Action",
            Self::Rumor => "Rumor",
        }
    }
    pub(crate) fn field(self) -> &'static str {
        match self {
            Self::Action => crate::actions::FIELD,
            Self::Rumor => crate::rumors::FIELD,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Draft {
    Action(Action),
    Rumor(Rumor),
}

impl Draft {
    pub fn new(kind: Kind) -> Self {
        match kind {
            Kind::Action => Self::Action(Action::default()),
            Kind::Rumor => Self::Rumor(Rumor::default()),
        }
    }
    pub fn kind(&self) -> Kind {
        match self {
            Self::Action(_) => Kind::Action,
            Self::Rumor(_) => Kind::Rumor,
        }
    }
    pub fn day(&self) -> &str {
        match self {
            Self::Action(a) => &a.day,
            Self::Rumor(r) => &r.day,
        }
    }
    pub fn label(&self) -> &str {
        match self {
            Self::Action(a) => &a.character,
            Self::Rumor(r) => &r.title,
        }
    }
    pub fn ready(&self) -> bool {
        match self {
            Self::Action(a) => a.is_complete(),
            Self::Rumor(r) => r.reviewed,
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        match self {
            Self::Action(a) => crate::actions::validate(a),
            Self::Rumor(r) => crate::rumors::validate(r),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Snapshot {
    pub revision: i64,
    pub draft: Draft,
    pub legacy: bool,
    pub retained: bool,
}

#[derive(Clone, Debug)]
pub struct Master {
    pub revision: i64,
    pub kind: Kind,
    pub from: String,
    pub through: String,
    pub incomplete_only: bool,
    pub undated_omitted: usize,
    pub records: Vec<Snapshot>,
}

pub(crate) fn validate_range(from: &str, through: &str) -> Result<()> {
    if !xp::day_valid(from) || !xp::day_valid(through) || from > through {
        return Err(error(
            "STORY_REPORT_DATES",
            "Enter an inclusive YYYY-MM-DD date range in chronological order.",
        ));
    }
    Ok(())
}

pub fn category_label(code: i32, l: Language) -> &'static str {
    let pt = l == Language::Portuguese;
    match (code, pt) {
        (0, false) => "General",
        (0, true) => "Geral",
        (1, false) => "Influence",
        (1, true) => "Influência",
        (2, false) => "Race",
        (2, true) => "Raça",
        (3, false) => "Group",
        (3, true) => "Grupo",
        (4, false) => "Subgroup",
        (4, true) => "Subgrupo",
        (5, false) => "Personal",
        (5, true) => "Pessoal",
        (_, false) => "Unrecognized source category",
        (_, true) => "Categoria de origem desconhecida",
    }
}

pub(crate) fn header(title: &str, lang: Language, rev: i64) -> String {
    let pt = lang == Language::Portuguese;
    let t = |en, pr| if pt { pr } else { en };
    format!(
        "<!doctype html><html lang=\"{}\"><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{}</title><style>body{{font:16px system-ui;margin:2rem auto;max-width:62rem;padding:0 1rem}}p,dd{{white-space:pre-wrap;overflow-wrap:anywhere}}section{{border-top:1px solid #777}}h2,h3{{break-after:avoid}}@media print{{body{{margin:0;max-width:none}}}}</style><h1>{}</h1><p><strong>{}</strong> · {} {rev}</p><p>{}</p>",
        t("en", "pt-BR"),
        escape(title),
        escape(title),
        t(
            "Private Storyteller report",
            "Relatório privado do Narrador"
        ),
        t("Library revision", "Revisão da biblioteca"),
        t(
            "Saved drafts only. No rules were applied and nothing was delivered.",
            "Somente rascunhos salvos. Nenhuma regra foi aplicada e nada foi enviado."
        )
    )
}

pub(crate) fn section(out: &mut String, s: &Snapshot, lang: Language) {
    let t = |en, pt| if lang == Language::Portuguese { pt } else { en };
    let _ = write!(
        out,
        "<section><h2>{} · {}</h2>",
        escape(s.draft.day()),
        escape(s.draft.label())
    );
    if s.legacy || s.retained {
        let _ = write!(
            out,
            "<p>{}</p>",
            t(
                "Only editable fields are shown. Original legacy data remains in the library.",
                "Somente campos editáveis são exibidos. Os dados legados originais permanecem na biblioteca."
            )
        );
    }
    match &s.draft {
        Draft::Action(a) => {
            let _ = write!(
                out,
                "<p>{}</p>",
                if a.is_complete() {
                    t(
                        "Action and result text complete",
                        "Textos da ação e do resultado completos",
                    )
                } else {
                    t("Incomplete action text", "Texto da ação incompleto")
                }
            );
            for slot in &a.slots {
                let _ = write!(
                    out,
                    "<h3>{}</h3><p>{}: {} · {}: {} · {}: {} · {}: {}</p><dl><dt>{}</dt><dd>{}</dd><dt>{}</dt><dd>{}</dd></dl>",
                    escape(&slot.name),
                    t("Level", "Nível"),
                    escape(&slot.level),
                    t("Unused", "Não usado"),
                    escape(&slot.unused),
                    t("Total", "Total"),
                    escape(&slot.total),
                    t("Growth", "Crescimento"),
                    escape(&slot.growth),
                    t("Action", "Ação"),
                    escape(&slot.action),
                    t("Result", "Resultado"),
                    escape(&slot.result)
                );
            }
        }
        Draft::Rumor(r) => {
            let _ = write!(
                out,
                "<p>{} · {}</p><p>{}: {} · {}: {} / {}</p>",
                category_label(r.category, lang),
                if r.reviewed {
                    t("Locally reviewed", "Revisado localmente")
                } else {
                    t("Unreviewed draft", "Rascunho sem revisão")
                },
                t("Audience label", "Nome do público"),
                escape(&r.audience_label),
                t("Influence labels", "Nomes de influência"),
                escape(&r.influence_list),
                escape(&r.influence_trait)
            );
            for entry in &r.entries {
                let _ = write!(
                    out,
                    "<h3>{}: {}</h3><p>{}</p>",
                    t("Level", "Nível"),
                    escape(&entry.level),
                    escape(&entry.body)
                );
            }
        }
    }
    out.push_str("</section>");
}

pub fn html(s: &Snapshot, lang: Language) -> Result<String> {
    s.draft.validate()?;
    let title = match (s.draft.kind(), lang) {
        (Kind::Action, Language::English) => "Action and result",
        (Kind::Action, Language::Portuguese) => "Ação e resultado",
        (Kind::Rumor, Language::English) => "Rumor draft",
        (Kind::Rumor, Language::Portuguese) => "Rascunho de rumor",
    };
    let mut out = header(title, lang, s.revision);
    section(&mut out, s, lang);
    out.push_str("</html>");
    Ok(out)
}

pub fn master_html(m: &Master, lang: Language) -> Result<String> {
    validate_range(&m.from, &m.through)?;
    let pt = lang == Language::Portuguese;
    let t = |en, pr| if pt { pr } else { en };
    let title = match m.kind {
        Kind::Action => t("Master action report", "Relatório geral de ações"),
        Kind::Rumor => t("Master rumor report", "Relatório geral de rumores"),
    };
    let mut out = header(title, lang, m.revision);
    let _ = write!(
        out,
        "<p>{}: {} – {} · {} · {}: {} · {}: {}</p>",
        t("Inclusive date range", "Intervalo de datas inclusivo"),
        escape(&m.from),
        escape(&m.through),
        if m.incomplete_only {
            t("Incomplete only", "Somente incompletos")
        } else {
            t("All drafts", "Todos os rascunhos")
        },
        t("Records shown", "Registros exibidos"),
        m.records.len(),
        t(
            "Unresolved dates excluded",
            "Datas não resolvidas excluídas"
        ),
        m.undated_omitted
    );
    for s in &m.records {
        if s.revision != m.revision
            || s.draft.kind() != m.kind
            || !xp::day_valid(s.draft.day())
            || (m.incomplete_only && s.draft.ready())
            || s.draft.day() < m.from.as_str()
            || s.draft.day() > m.through.as_str()
        {
            return Err(error(
                "STORY_REPORT_SCOPE",
                "A record does not belong to this report snapshot.",
            ));
        }
        section(&mut out, s, lang);
        if out.len() > 8 * 1024 * 1024 {
            return Err(error(
                "STORY_REPORT_LIMIT",
                "The report is too large. Narrow the date range.",
            ));
        }
    }
    out.push_str("</html>");
    Ok(out)
}
