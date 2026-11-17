//! Private, read-only projections of posted and imported XP history.
use crate::{
    model::{Language, Result, error},
    player_points, xp,
};

const MAX_PROJECTION_BYTES: usize = 8 * 1024 * 1024;
const MAX_HTML_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    CharacterExperience,
    PlayerPoints,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CurrentSnapshot {
    Unavailable,
    CharacterExperience { earned: String, unspent: String },
    PlayerPoints { balance: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opening {
    pub day: String,
    pub earned: Option<String>,
    pub unspent: Option<String>,
    pub balance: Option<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Award,
    Spend,
    Adjustment,
    Reversal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostedDelta {
    CharacterExperience { earned: String, unspent: String },
    PlayerPoints { balance: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostedRow {
    pub day: String,
    pub kind: EntryKind,
    pub delta: PostedDelta,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyRow {
    pub date: String,
    pub kind: String,
    pub amount: String,
    pub earned: String,
    pub unspent: String,
    pub reason: Option<String>,
}

/// Imported field values are retained as raw strings and are never replayed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyTotals {
    pub earned: Option<String>,
    pub unspent: Option<String>,
}

/// Language-neutral data for a private operational history report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    pub pool: Pool,
    pub subject: String,
    pub revision: i64,
    pub from: String,
    pub through: String,
    pub snapshot: CurrentSnapshot,
    pub opening: Option<Opening>,
    pub posted: Vec<PostedRow>,
    pub legacy: Vec<LegacyRow>,
    /// Legacy entries whose date cannot be placed in the requested ISO range.
    /// They remain visible as imported evidence and are counted as excluded.
    pub legacy_uncertain: Vec<LegacyRow>,
    pub legacy_unavailable: bool,
    pub legacy_totals: Option<LegacyTotals>,
    pub posted_excluded: usize,
    pub legacy_outside_range: usize,
    pub legacy_uncertain_excluded: usize,
}

#[derive(Default)]
struct ProjectionBudget {
    bytes: usize,
}

impl ProjectionBudget {
    fn add(&mut self, value: &str) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(value.len())
            .filter(|size| *size <= MAX_PROJECTION_BYTES)
            .ok_or_else(|| {
                error(
                    "HISTORY_TOO_LARGE",
                    "This history is too large to prepare as a report.",
                )
            })?;
        Ok(())
    }

    fn add_optional(&mut self, value: Option<&str>) -> Result<()> {
        if let Some(value) = value {
            self.add(value)?;
        }
        Ok(())
    }
}

fn validate_window(from: &str, through: &str) -> Result<()> {
    if !xp::day_valid(from) || !xp::day_valid(through) || from > through {
        return Err(error(
            "HISTORY_DATE_RANGE",
            "Choose an inclusive date range using valid YYYY-MM-DD dates.",
        ));
    }
    Ok(())
}

fn in_window(day: &str, from: &str, through: &str) -> bool {
    from <= day && day <= through
}

fn legacy_day(date: &str) -> Option<&str> {
    let day = date.get(..10)?;
    if !xp::day_valid(day) {
        return None;
    }
    if date.len() > 10 && !matches!(date.as_bytes().get(10), Some(b' ' | b'T')) {
        return None;
    }
    Some(day)
}

fn legacy_totals(view: &xp::XpView, budget: &mut ProjectionBudget) -> Result<Option<LegacyTotals>> {
    if view.legacy_earned.is_none() && view.legacy_unspent.is_none() {
        return Ok(None);
    }
    budget.add_optional(view.legacy_earned.as_deref())?;
    budget.add_optional(view.legacy_unspent.as_deref())?;
    Ok(Some(LegacyTotals {
        earned: view.legacy_earned.clone(),
        unspent: view.legacy_unspent.clone(),
    }))
}

fn legacy_row(
    event: &xp::LegacyEvent,
    reasons: bool,
    budget: &mut ProjectionBudget,
) -> Result<LegacyRow> {
    budget.add(&event.date)?;
    budget.add(&event.kind)?;
    budget.add(&event.amount)?;
    budget.add(&event.earned)?;
    budget.add(&event.unspent)?;
    budget.add_optional(reasons.then_some(event.reason.as_str()))?;
    Ok(LegacyRow {
        date: event.date.clone(),
        kind: event.kind.clone(),
        amount: event.amount.clone(),
        earned: event.earned.clone(),
        unspent: event.unspent.clone(),
        reason: reasons.then(|| event.reason.clone()),
    })
}

fn project_legacy(
    view: &xp::XpView,
    from: &str,
    through: &str,
    reasons: bool,
    budget: &mut ProjectionBudget,
) -> Result<(Vec<LegacyRow>, Vec<LegacyRow>, usize, usize)> {
    let mut included = Vec::<(String, usize, LegacyRow)>::new();
    let mut uncertain = Vec::<(usize, LegacyRow)>::new();
    let mut outside_range = 0;

    for (sequence, event) in view.legacy_history.iter().enumerate() {
        match legacy_day(&event.date) {
            Some(day) if in_window(day, from, through) => {
                included.push((
                    day.to_owned(),
                    sequence,
                    legacy_row(event, reasons, budget)?,
                ));
            }
            Some(_) => outside_range += 1,
            None => uncertain.push((sequence, legacy_row(event, reasons, budget)?)),
        }
    }

    included.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    uncertain.sort_by_key(|(sequence, _)| *sequence);
    let rows = included.into_iter().map(|(_, _, row)| row).collect();
    let uncertain_rows: Vec<LegacyRow> = uncertain.into_iter().map(|(_, row)| row).collect();
    let uncertain_count = uncertain_rows.len();
    Ok((rows, uncertain_rows, outside_range, uncertain_count))
}

fn character_kind(kind: xp::XpKind) -> EntryKind {
    match kind {
        xp::XpKind::Award => EntryKind::Award,
        xp::XpKind::Spend => EntryKind::Spend,
        xp::XpKind::Adjustment => EntryKind::Adjustment,
        xp::XpKind::Reversal => EntryKind::Reversal,
    }
}

fn project_character_entries(
    ledger: &xp::Ledger,
    from: &str,
    through: &str,
    reasons: bool,
    budget: &mut ProjectionBudget,
) -> Result<(Vec<PostedRow>, usize)> {
    let by_id = ledger
        .entries
        .iter()
        .map(|entry| (entry.command.id.as_str(), &entry.command))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut selected = Vec::<(String, usize, PostedRow)>::new();
    let mut excluded = 0;

    for (sequence, entry) in ledger.entries.iter().enumerate() {
        let command = &entry.command;
        if !in_window(&command.day, from, through) {
            excluded += 1;
            continue;
        }
        let (earned_delta, unspent_delta) = if command.kind == xp::XpKind::Reversal {
            let target_id = command.reversal_of.as_deref().ok_or_else(|| {
                error(
                    "XP_CORRUPT_LEDGER",
                    "A reversal target is unavailable for this report.",
                )
            })?;
            let target = by_id.get(target_id).copied().ok_or_else(|| {
                error(
                    "XP_CORRUPT_LEDGER",
                    "A reversal target is unavailable for this report.",
                )
            })?;
            let (earned, unspent) = xp::ordinary_delta(target)?;
            (
                earned.checked_neg().ok_or_else(|| {
                    error(
                        "HISTORY_DELTA_OVERFLOW",
                        "A report delta exceeds its exact range.",
                    )
                })?,
                unspent.checked_neg().ok_or_else(|| {
                    error(
                        "HISTORY_DELTA_OVERFLOW",
                        "A report delta exceeds its exact range.",
                    )
                })?,
            )
        } else {
            xp::ordinary_delta(command)?
        };
        let earned = xp::decimal(earned_delta);
        let unspent = xp::decimal(unspent_delta);
        let reason = reasons.then(|| command.reason.clone());
        budget.add(&command.day)?;
        budget.add(&earned)?;
        budget.add(&unspent)?;
        budget.add_optional(reason.as_deref())?;
        selected.push((
            command.day.clone(),
            sequence,
            PostedRow {
                day: command.day.clone(),
                kind: character_kind(command.kind),
                delta: PostedDelta::CharacterExperience { earned, unspent },
                reason,
            },
        ));
    }

    selected.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    Ok((
        selected.into_iter().map(|(_, _, row)| row).collect(),
        excluded,
    ))
}

fn points_kind(kind: player_points::Kind) -> EntryKind {
    match kind {
        player_points::Kind::Award => EntryKind::Award,
        player_points::Kind::Spend => EntryKind::Spend,
        player_points::Kind::Adjustment => EntryKind::Adjustment,
        player_points::Kind::Reversal => EntryKind::Reversal,
    }
}

fn project_player_entries(
    ledger: &player_points::Ledger,
    from: &str,
    through: &str,
    reasons: bool,
    budget: &mut ProjectionBudget,
) -> Result<(Vec<PostedRow>, usize)> {
    let mut selected = Vec::<(String, usize, PostedRow)>::new();
    let mut excluded = 0;

    for (sequence, command) in ledger.entries.iter().enumerate() {
        if !in_window(&command.day, from, through) {
            excluded += 1;
            continue;
        }
        let delta = xp::decimal(player_points::delta(command)?);
        let reason = reasons.then(|| command.reason.clone());
        budget.add(&command.day)?;
        budget.add(&delta)?;
        budget.add_optional(reason.as_deref())?;
        selected.push((
            command.day.clone(),
            sequence,
            PostedRow {
                day: command.day.clone(),
                kind: points_kind(command.kind),
                delta: PostedDelta::PlayerPoints { balance: delta },
                reason,
            },
        ));
    }

    selected.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    Ok((
        selected.into_iter().map(|(_, _, row)| row).collect(),
        excluded,
    ))
}

pub fn character(
    view: &xp::XpView,
    revision: i64,
    from: &str,
    through: &str,
    reasons: bool,
) -> Result<History> {
    validate_window(from, through)?;
    let mut budget = ProjectionBudget::default();
    budget.add(&view.record_name)?;
    budget.add(from)?;
    budget.add(through)?;

    let (snapshot, opening, posted, posted_excluded) = match &view.ledger {
        Some(ledger) => {
            // Replay first so every entry, including entries outside the selected dates,
            // is validated before any row is projected.
            let (earned, unspent) = xp::replay(ledger)?;
            let baseline = &ledger.baseline;
            let earned_text = xp::decimal(earned);
            let unspent_text = xp::decimal(unspent);
            budget.add(&baseline.day)?;
            budget.add(&baseline.earned)?;
            budget.add(&baseline.unspent)?;
            budget.add_optional(reasons.then_some(baseline.reason.as_str()))?;
            let opening = Opening {
                day: baseline.day.clone(),
                earned: Some(baseline.earned.clone()),
                unspent: Some(baseline.unspent.clone()),
                balance: None,
                reason: reasons.then(|| baseline.reason.clone()),
            };
            let (posted, excluded) =
                project_character_entries(ledger, from, through, reasons, &mut budget)?;
            (
                CurrentSnapshot::CharacterExperience {
                    earned: earned_text,
                    unspent: unspent_text,
                },
                Some(opening),
                posted,
                excluded,
            )
        }
        None => (CurrentSnapshot::Unavailable, None, Vec::new(), 0),
    };

    let legacy_totals = legacy_totals(view, &mut budget)?;
    let (legacy, legacy_uncertain, legacy_outside_range, legacy_uncertain_excluded) =
        project_legacy(view, from, through, reasons, &mut budget)?;
    Ok(History {
        pool: Pool::CharacterExperience,
        subject: view.record_name.clone(),
        revision,
        from: from.to_owned(),
        through: through.to_owned(),
        snapshot,
        opening,
        posted,
        legacy,
        legacy_uncertain,
        legacy_unavailable: view
            .findings
            .iter()
            .any(|finding| finding.code == "LEGACY_HISTORY_UNAVAILABLE"),
        legacy_totals,
        posted_excluded,
        legacy_outside_range,
        legacy_uncertain_excluded,
    })
}

pub fn player(
    name: &str,
    ledger: Option<&player_points::Ledger>,
    legacy: &xp::XpView,
    revision: i64,
    from: &str,
    through: &str,
    reasons: bool,
) -> Result<History> {
    validate_window(from, through)?;
    let mut budget = ProjectionBudget::default();
    budget.add(name)?;
    budget.add(from)?;
    budget.add(through)?;

    let (snapshot, opening, posted, posted_excluded) = match ledger {
        Some(ledger) => {
            // Validate the complete Player Points ledger before applying the date filter.
            let balance = player_points::replay(ledger)?;
            let baseline = &ledger.baseline;
            budget.add(&baseline.day)?;
            budget.add(&baseline.balance)?;
            budget.add_optional(reasons.then_some(baseline.reason.as_str()))?;
            let opening = Opening {
                day: baseline.day.clone(),
                earned: None,
                unspent: None,
                balance: Some(baseline.balance.clone()),
                reason: reasons.then(|| baseline.reason.clone()),
            };
            let (posted, excluded) =
                project_player_entries(ledger, from, through, reasons, &mut budget)?;
            (
                CurrentSnapshot::PlayerPoints { balance },
                Some(opening),
                posted,
                excluded,
            )
        }
        None => (CurrentSnapshot::Unavailable, None, Vec::new(), 0),
    };

    let legacy_totals = legacy_totals(legacy, &mut budget)?;
    let (legacy_rows, legacy_uncertain, legacy_outside_range, legacy_uncertain_excluded) =
        project_legacy(legacy, from, through, reasons, &mut budget)?;
    Ok(History {
        pool: Pool::PlayerPoints,
        subject: name.to_owned(),
        revision,
        from: from.to_owned(),
        through: through.to_owned(),
        snapshot,
        opening,
        posted,
        legacy: legacy_rows,
        legacy_uncertain,
        legacy_unavailable: legacy
            .findings
            .iter()
            .any(|finding| finding.code == "LEGACY_HISTORY_UNAVAILABLE"),
        legacy_totals,
        posted_excluded,
        legacy_outside_range,
        legacy_uncertain_excluded,
    })
}

#[derive(Clone, Copy)]
struct Labels {
    title_experience: &'static str,
    title_points: &'static str,
    current: &'static str,
    current_unavailable: &'static str,
    earned: &'static str,
    unspent: &'static str,
    balance: &'static str,
    opening: &'static str,
    opening_note: &'static str,
    date: &'static str,
    from: &'static str,
    through: &'static str,
    revision: &'static str,
    visible_rows: &'static str,
    posted_experience: &'static str,
    posted_points: &'static str,
    legacy_experience: &'static str,
    legacy_points: &'static str,
    legacy_uncertain: &'static str,
    legacy_fields: &'static str,
    raw_earned: &'static str,
    raw_unspent: &'static str,
    effective_date: &'static str,
    recorded_date: &'static str,
    event_type: &'static str,
    earned_change: &'static str,
    unspent_change: &'static str,
    point_change: &'static str,
    recorded_change: &'static str,
    imported_earned: &'static str,
    imported_unspent: &'static str,
    reason: &'static str,
    no_rows: &'static str,
    posted_excluded: &'static str,
    legacy_outside: &'static str,
    legacy_unknown: &'static str,
    exclusions: &'static str,
    legacy_unavailable_warning: &'static str,
    reversal_note: &'static str,
    award: &'static str,
    spend: &'static str,
    adjustment: &'static str,
    reversal: &'static str,
}

fn labels(language: Language) -> Labels {
    match language {
        Language::English => Labels {
            title_experience: "Experience History",
            title_points: "Player Point History",
            current: "Current local snapshot",
            current_unavailable: "No exact local snapshot is available.",
            earned: "Earned",
            unspent: "Unspent",
            balance: "Balance",
            opening: "Opening declaration",
            opening_note: "Shown separately; it is not a date-filtered entry.",
            date: "Date",
            from: "From (inclusive)",
            through: "Through (inclusive)",
            revision: "Snapshot revision",
            visible_rows: "Visible history rows",
            posted_experience: "Posted Character XP entries",
            posted_points: "Posted Player Points entries",
            legacy_experience: "Imported legacy history",
            legacy_points: "Imported Player Point history",
            legacy_uncertain: "Imported entries excluded because their dates are unavailable",
            legacy_fields: "Imported fields as recorded (raw; not replayed)",
            raw_earned: "Imported Earned field (raw)",
            raw_unspent: "Imported Unspent field (raw)",
            effective_date: "Effective date",
            recorded_date: "Recorded date",
            event_type: "Type",
            earned_change: "Earned change",
            unspent_change: "Unspent change",
            point_change: "Balance change",
            recorded_change: "Recorded change",
            imported_earned: "Imported Earned field",
            imported_unspent: "Imported Unspent field",
            reason: "Reason",
            no_rows: "No entries in this section for the selected date range.",
            posted_excluded: "Posted entries outside the selected date range",
            legacy_outside: "Imported entries outside the selected date range",
            legacy_unknown: "Imported entries excluded for uncertain dates",
            exclusions: "Date range exclusions",
            legacy_unavailable_warning: "Some imported history could not be parsed and is unavailable in this report.",
            reversal_note: "Reversals appear as signed changes; original entries remain in the history.",
            award: "Award",
            spend: "Spend",
            adjustment: "Adjustment",
            reversal: "Reversal",
        },
        Language::Portuguese => Labels {
            title_experience: "Histórico de Experiência",
            title_points: "Histórico de Pontos do Jogador",
            current: "Resumo local atual",
            current_unavailable: "Não há um resumo local exato disponível.",
            earned: "Obtido",
            unspent: "Não gasto",
            balance: "Saldo",
            opening: "Saldo inicial declarado",
            opening_note: "Exibido separadamente; não é um lançamento filtrado por data.",
            date: "Data",
            from: "De (inclusive)",
            through: "Até (inclusive)",
            revision: "Revisão do resumo",
            visible_rows: "Linhas de histórico visíveis",
            posted_experience: "Lançamentos de XP de personagem",
            posted_points: "Lançamentos de Pontos do Jogador",
            legacy_experience: "Histórico legado importado",
            legacy_points: "Histórico importado de Pontos do Jogador",
            legacy_uncertain: "Lançamentos importados excluídos por data indisponível",
            legacy_fields: "Campos importados como registrados (brutos; sem recálculo)",
            raw_earned: "Campo Obtido importado (bruto)",
            raw_unspent: "Campo Não gasto importado (bruto)",
            effective_date: "Data efetiva",
            recorded_date: "Data registrada",
            event_type: "Tipo",
            earned_change: "Variação de obtido",
            unspent_change: "Variação de não gasto",
            point_change: "Variação do saldo",
            recorded_change: "Variação registrada",
            imported_earned: "Campo Obtido importado",
            imported_unspent: "Campo Não gasto importado",
            reason: "Motivo",
            no_rows: "Não há lançamentos nesta seção para o período selecionado.",
            posted_excluded: "Lançamentos publicados fora do período selecionado",
            legacy_outside: "Lançamentos importados fora do período selecionado",
            legacy_unknown: "Lançamentos importados excluídos por data incerta",
            exclusions: "Exclusões do período",
            legacy_unavailable_warning: "Parte do histórico importado não pôde ser interpretada e não está disponível neste relatório.",
            reversal_note: "Estornos aparecem como variações assinadas; os lançamentos originais permanecem no histórico.",
            award: "Crédito",
            spend: "Gasto",
            adjustment: "Ajuste",
            reversal: "Estorno",
        },
    }
}

fn append(out: &mut String, value: &str) -> Result<()> {
    let size = out
        .len()
        .checked_add(value.len())
        .filter(|size| *size <= MAX_HTML_BYTES)
        .ok_or_else(|| {
            error(
                "HISTORY_TOO_LARGE",
                "The history report exceeds its size limit.",
            )
        })?;
    out.reserve(size.saturating_sub(out.len()));
    out.push_str(value);
    Ok(())
}

fn append_escaped(out: &mut String, value: &str) -> Result<()> {
    for character in value.chars() {
        match character {
            '&' => append(out, "&amp;")?,
            '<' => append(out, "&lt;")?,
            '>' => append(out, "&gt;")?,
            '"' => append(out, "&quot;")?,
            '\'' => append(out, "&#39;")?,
            _ => {
                let mut encoded = [0; 4];
                append(out, character.encode_utf8(&mut encoded))?;
            }
        }
    }
    Ok(())
}

fn append_cell(out: &mut String, value: &str, heading: bool) -> Result<()> {
    if heading {
        append(out, "<th scope=\"col\">")?;
        append_escaped(out, value)?;
        append(out, "</th>")?;
    } else {
        append(out, "<td>")?;
        append_escaped(out, value)?;
        append(out, "</td>")?;
    }
    Ok(())
}

fn append_count(out: &mut String, label: &str, count: usize) -> Result<()> {
    append(out, "<li>")?;
    append_escaped(out, label)?;
    append(out, ": ")?;
    append_escaped(out, &count.to_string())?;
    append(out, "</li>")
}

fn append_kind(out: &mut String, kind: EntryKind, text: Labels) -> Result<()> {
    let label = match kind {
        EntryKind::Award => text.award,
        EntryKind::Spend => text.spend,
        EntryKind::Adjustment => text.adjustment,
        EntryKind::Reversal => text.reversal,
    };
    append_escaped(out, label)
}

fn append_posted_table(
    out: &mut String,
    history: &History,
    text: Labels,
    reasons: bool,
) -> Result<()> {
    if history.posted.is_empty() {
        append(out, "<p class=\"empty\">")?;
        append_escaped(out, text.no_rows)?;
        return append(out, "</p>");
    }
    append(out, "<p class=\"note\">")?;
    append_escaped(out, text.reversal_note)?;
    append(out, "</p><div class=\"table-wrap\"><table><thead><tr>")?;
    append_cell(out, text.effective_date, true)?;
    append_cell(out, text.event_type, true)?;
    match history.pool {
        Pool::CharacterExperience => {
            append_cell(out, text.earned_change, true)?;
            append_cell(out, text.unspent_change, true)?;
        }
        Pool::PlayerPoints => append_cell(out, text.point_change, true)?,
    }
    if reasons {
        append_cell(out, text.reason, true)?;
    }
    append(out, "</tr></thead><tbody>")?;
    for row in &history.posted {
        append(out, "<tr>")?;
        append_cell(out, &row.day, false)?;
        append(out, "<td>")?;
        append_kind(out, row.kind, text)?;
        append(out, "</td>")?;
        match &row.delta {
            PostedDelta::CharacterExperience { earned, unspent } => {
                append_cell(out, earned, false)?;
                append_cell(out, unspent, false)?;
            }
            PostedDelta::PlayerPoints { balance } => append_cell(out, balance, false)?,
        }
        if reasons {
            append_cell(out, row.reason.as_deref().unwrap_or(""), false)?;
        }
        append(out, "</tr>")?;
    }
    append(out, "</tbody></table></div>")
}

fn append_legacy_table(
    out: &mut String,
    rows: &[LegacyRow],
    text: Labels,
    reasons: bool,
) -> Result<()> {
    if rows.is_empty() {
        append(out, "<p class=\"empty\">")?;
        append_escaped(out, text.no_rows)?;
        return append(out, "</p>");
    }
    append(out, "<div class=\"table-wrap\"><table><thead><tr>")?;
    append_cell(out, text.recorded_date, true)?;
    append_cell(out, text.event_type, true)?;
    append_cell(out, text.recorded_change, true)?;
    append_cell(out, text.imported_earned, true)?;
    append_cell(out, text.imported_unspent, true)?;
    if reasons {
        append_cell(out, text.reason, true)?;
    }
    append(out, "</tr></thead><tbody>")?;
    for row in rows {
        append(out, "<tr>")?;
        append_cell(out, &row.date, false)?;
        append_cell(out, &row.kind, false)?;
        append_cell(out, &row.amount, false)?;
        append_cell(out, &row.earned, false)?;
        append_cell(out, &row.unspent, false)?;
        if reasons {
            append_cell(out, row.reason.as_deref().unwrap_or(""), false)?;
        }
        append(out, "</tr>")?;
    }
    append(out, "</tbody></table></div>")
}

fn section_heading(out: &mut String, title: &str) -> Result<()> {
    append(out, "<section><h2>")?;
    append_escaped(out, title)?;
    append(out, "</h2>")
}

pub fn html(history: &History, language: Language) -> Result<String> {
    let text = labels(language);
    let title = match history.pool {
        Pool::CharacterExperience => text.title_experience,
        Pool::PlayerPoints => text.title_points,
    };
    let lang = match language {
        Language::English => "en",
        Language::Portuguese => "pt",
    };
    let show_reasons = history
        .opening
        .as_ref()
        .is_some_and(|opening| opening.reason.is_some())
        || history.posted.iter().any(|row| row.reason.is_some())
        || history.legacy.iter().any(|row| row.reason.is_some())
        || history
            .legacy_uncertain
            .iter()
            .any(|row| row.reason.is_some());

    let mut out = String::new();
    append(&mut out, "<!doctype html><html lang=\"")?;
    append(&mut out, lang)?;
    append(
        &mut out,
        "\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\"><title>",
    )?;
    append_escaped(&mut out, title)?;
    append(
        &mut out,
        "</title><style>body{font:16px/1.45 system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;color:#17212b}h1,h2{line-height:1.2}section{margin:1.8rem 0}dl{display:grid;grid-template-columns:max-content 1fr;gap:.35rem 1rem}dt{font-weight:650}dd{margin:0}.subject{font-size:1.15rem;font-weight:650}.private-banner{font-weight:700;color:#7a2020;border:1px solid #7a2020;padding:.4rem .6rem;display:inline-block}.warning{background:#fff5d6;border-left:4px solid #9b6c00;padding:.6rem}.note,.empty{color:#44515e}.counts{margin:.5rem 0}.table-wrap{overflow-x:auto}table{border-collapse:collapse;width:100%;margin:.5rem 0 1rem}th,td{border:1px solid #9ba8b3;padding:.4rem .55rem;text-align:left;vertical-align:top}th{background:#eef2f5}td{overflow-wrap:anywhere}@media print{body{max-width:none;margin:0;padding:0;color:#000}section{break-inside:avoid}.table-wrap{overflow:visible}table{font-size:10pt}th,td{border-color:#666}.note,.empty{color:#222}}</style></head><body>",
    )?;
    append(&mut out, "<header><h1>")?;
    append_escaped(&mut out, title)?;
    append(
        &mut out,
        "</h1><p class=\"private-banner\">Private Storyteller report / Relatório privado</p><p class=\"subject\">",
    )?;
    append_escaped(&mut out, &history.subject)?;
    append(&mut out, "</p></header>")?;

    section_heading(&mut out, text.current)?;
    append(&mut out, "<dl><dt>")?;
    append_escaped(&mut out, text.from)?;
    append(&mut out, "</dt><dd>")?;
    append_escaped(&mut out, &history.from)?;
    append(&mut out, "</dd><dt>")?;
    append_escaped(&mut out, text.through)?;
    append(&mut out, "</dt><dd>")?;
    append_escaped(&mut out, &history.through)?;
    append(&mut out, "</dd><dt>")?;
    append_escaped(&mut out, text.revision)?;
    append(&mut out, "</dt><dd>")?;
    append_escaped(&mut out, &history.revision.to_string())?;
    append(&mut out, "</dd><dt>")?;
    append_escaped(&mut out, text.visible_rows)?;
    append(&mut out, "</dt><dd>")?;
    append_escaped(&mut out, &row_count(history).to_string())?;
    append(&mut out, "</dd>")?;
    match &history.snapshot {
        CurrentSnapshot::Unavailable => {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.current)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, text.current_unavailable)?;
            append(&mut out, "</dd>")?;
        }
        CurrentSnapshot::CharacterExperience { earned, unspent } => {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.earned)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, earned)?;
            append(&mut out, "</dd><dt>")?;
            append_escaped(&mut out, text.unspent)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, unspent)?;
            append(&mut out, "</dd>")?;
        }
        CurrentSnapshot::PlayerPoints { balance } => {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.balance)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, balance)?;
            append(&mut out, "</dd>")?;
        }
    }
    append(&mut out, "</dl></section>")?;

    if let Some(opening) = &history.opening {
        section_heading(&mut out, text.opening)?;
        append(&mut out, "<p class=\"note\">")?;
        append_escaped(&mut out, text.opening_note)?;
        append(&mut out, "</p><dl><dt>")?;
        append_escaped(&mut out, text.date)?;
        append(&mut out, "</dt><dd>")?;
        append_escaped(&mut out, &opening.day)?;
        append(&mut out, "</dd>")?;
        if let Some(balance) = &opening.balance {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.balance)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, balance)?;
            append(&mut out, "</dd>")?;
        }
        if let Some(earned) = &opening.earned {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.earned)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, earned)?;
            append(&mut out, "</dd>")?;
        }
        if let Some(unspent) = &opening.unspent {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.unspent)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, unspent)?;
            append(&mut out, "</dd>")?;
        }
        if show_reasons && let Some(reason) = &opening.reason {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.reason)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, reason)?;
            append(&mut out, "</dd>")?;
        }
        append(&mut out, "</dl></section>")?;
    }

    section_heading(
        &mut out,
        match history.pool {
            Pool::CharacterExperience => text.posted_experience,
            Pool::PlayerPoints => text.posted_points,
        },
    )?;
    append_posted_table(&mut out, history, text, show_reasons)?;
    append(&mut out, "</section>")?;

    section_heading(
        &mut out,
        match history.pool {
            Pool::CharacterExperience => text.legacy_experience,
            Pool::PlayerPoints => text.legacy_points,
        },
    )?;
    if history.legacy_unavailable {
        append(&mut out, "<p class=\"warning\" role=\"status\">")?;
        append_escaped(&mut out, text.legacy_unavailable_warning)?;
        append(&mut out, "</p>")?;
    }
    append_legacy_table(&mut out, &history.legacy, text, show_reasons)?;
    if let Some(totals) = &history.legacy_totals {
        append(&mut out, "<h3>")?;
        append_escaped(&mut out, text.legacy_fields)?;
        append(&mut out, "</h3><dl>")?;
        if let Some(earned) = &totals.earned {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.raw_earned)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, earned)?;
            append(&mut out, "</dd>")?;
        }
        if let Some(unspent) = &totals.unspent {
            append(&mut out, "<dt>")?;
            append_escaped(&mut out, text.raw_unspent)?;
            append(&mut out, "</dt><dd>")?;
            append_escaped(&mut out, unspent)?;
            append(&mut out, "</dd>")?;
        }
        append(&mut out, "</dl>")?;
    }
    append(&mut out, "</section>")?;

    append(&mut out, "<section><h2>")?;
    append_escaped(&mut out, text.legacy_uncertain)?;
    append(&mut out, "</h2>")?;
    append_legacy_table(&mut out, &history.legacy_uncertain, text, show_reasons)?;
    append(&mut out, "</section><section><h2>")?;
    append_escaped(&mut out, text.exclusions)?;
    append(&mut out, "</h2><ul class=\"counts\">")?;
    append_count(&mut out, text.posted_excluded, history.posted_excluded)?;
    append_count(&mut out, text.legacy_outside, history.legacy_outside_range)?;
    append_count(
        &mut out,
        text.legacy_unknown,
        history.legacy_uncertain_excluded,
    )?;
    append(&mut out, "</ul></section></body></html>")?;
    Ok(out)
}

pub fn row_count(history: &History) -> usize {
    history
        .posted
        .len()
        .saturating_add(history.legacy.len())
        .saturating_add(history.legacy_uncertain.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FROM: &str = "2024-01-01";
    const THROUGH: &str = "2024-12-31";

    fn xp_baseline() -> xp::Baseline {
        xp::Baseline {
            day: "2020-01-01".into(),
            earned: "10.75".into(),
            unspent: "8.25".into(),
            actor: "baseline actor secret".into(),
            reason: "baseline reason secret".into(),
            evidence: "baseline evidence secret".into(),
        }
    }

    fn xp_command(
        id: &str,
        kind: xp::XpKind,
        amount: &str,
        day: &str,
        reversal_of: Option<&str>,
    ) -> xp::XpCommand {
        xp::XpCommand {
            id: id.into(),
            kind,
            amount: amount.into(),
            day: day.into(),
            actor: "actor secret".into(),
            reason: "reason secret".into(),
            evidence: "evidence secret".into(),
            reversal_of: reversal_of.map(str::to_owned),
            cost: None,
        }
    }

    fn xp_entry(command: xp::XpCommand) -> xp::Entry {
        xp::Entry {
            command,
            recorded_unix_seconds: 1,
            origin_library_id: "library secret".into(),
        }
    }

    fn xp_view(ledger: Option<xp::Ledger>) -> xp::XpView {
        xp::XpView {
            record_id: "record secret".into(),
            record_name: "A <Character> & friend".into(),
            is_build: false,
            ledger,
            earned: None,
            unspent: None,
            legacy_history_count: 0,
            legacy_history: vec![],
            legacy_earned: None,
            legacy_unspent: None,
            findings: vec![],
            coverage: "private coverage".into(),
        }
    }

    fn xp_reversal_ledger() -> xp::Ledger {
        xp::Ledger {
            baseline: xp_baseline(),
            entries: vec![
                xp_entry(xp_command(
                    "award-id-secret",
                    xp::XpKind::Award,
                    "1.25",
                    "2024-01-01",
                    None,
                )),
                xp_entry(xp_command(
                    "spend-id-secret",
                    xp::XpKind::Spend,
                    "0.5",
                    "2024-01-05",
                    None,
                )),
                xp_entry(xp_command(
                    "reversal-id-secret",
                    xp::XpKind::Reversal,
                    "1.25",
                    "2024-02-01",
                    Some("award-id-secret"),
                )),
            ],
        }
    }

    fn legacy_event(date: &str, amount: &str, reason: &str) -> xp::LegacyEvent {
        xp::LegacyEvent {
            id: "legacy id secret".into(),
            date: date.into(),
            kind: "Earned".into(),
            amount: amount.into(),
            earned: "raw earned snapshot".into(),
            unspent: "raw unspent snapshot".into(),
            reason: reason.into(),
        }
    }

    #[test]
    fn character_reversal_is_signed_exact_and_can_target_outside_range() {
        let mut view = xp_view(Some(xp_reversal_ledger()));
        view.legacy_history = vec![legacy_event("Unknown date", "raw", "legacy reason")];
        let history = character(&view, 17, "2024-02-01", "2024-02-01", false).unwrap();
        assert_eq!(
            history.snapshot,
            CurrentSnapshot::CharacterExperience {
                earned: "10.75".into(),
                unspent: "7.75".into()
            }
        );
        assert_eq!(history.posted.len(), 1);
        assert_eq!(history.posted_excluded, 2);
        assert_eq!(history.legacy_uncertain_excluded, 1);
        assert_eq!(
            history.posted[0].delta,
            PostedDelta::CharacterExperience {
                earned: "-1.25".into(),
                unspent: "-1.25".into()
            }
        );
        assert_eq!(history.posted[0].reason, None);
        assert_eq!(row_count(&history), 2);
    }

    #[test]
    fn range_is_inclusive_and_rows_sort_by_effective_day_then_append_order() {
        let history = character(
            &xp_view(Some(xp_reversal_ledger())),
            4,
            "2024-01-01",
            "2024-01-05",
            true,
        )
        .unwrap();
        assert_eq!(
            history
                .posted
                .iter()
                .map(|row| row.day.as_str())
                .collect::<Vec<_>>(),
            vec!["2024-01-01", "2024-01-05"]
        );
        assert_eq!(history.posted_excluded, 1);
        assert_eq!(history.posted[0].reason.as_deref(), Some("reason secret"));
    }

    #[test]
    fn imported_unknown_dates_remain_visible_and_raw_totals_are_not_current_snapshot() {
        let mut view = xp_view(None);
        view.legacy_history = vec![
            legacy_event("Unknown date", "raw-change", "do not expose"),
            legacy_event("2024-02-28 23:59:00", "raw-in-range", "legacy note"),
            legacy_event("2025-01-01 00:00:00", "raw-outside", "outside note"),
        ];
        view.legacy_history_count = view.legacy_history.len();
        view.legacy_earned = Some("not-an-exact-number".into());
        view.legacy_unspent = Some("raw source value".into());
        view.findings.push(xp::Finding {
            code: "LEGACY_HISTORY_UNAVAILABLE".into(),
            severity: "unverified".into(),
            entry_id: Some("must-not-project".into()),
            message: "arbitrary diagnostic text must not project".into(),
        });
        let history = character(&view, 5, "2024-02-28", "2024-02-28", false).unwrap();
        assert_eq!(history.snapshot, CurrentSnapshot::Unavailable);
        assert!(history.legacy_unavailable);
        assert_eq!(history.legacy.len(), 1);
        assert_eq!(history.legacy[0].date, "2024-02-28 23:59:00");
        assert_eq!(history.legacy_outside_range, 1);
        assert_eq!(history.legacy_uncertain_excluded, 1);
        assert_eq!(history.legacy_uncertain[0].amount, "raw-change");
        assert_eq!(
            history.legacy_totals,
            Some(LegacyTotals {
                earned: Some("not-an-exact-number".into()),
                unspent: Some("raw source value".into())
            })
        );
        assert!(history.legacy[0].reason.is_none());
        let output = html(&history, Language::English).unwrap();
        assert!(output.contains("Some imported history could not be parsed"));
        assert!(!output.contains("must-not-project"));
        assert!(!output.contains("arbitrary diagnostic text"));
        assert!(output.contains("Private Storyteller report / Relatório privado"));
    }

    fn points_ledger(reversal_amount: &str) -> player_points::Ledger {
        let target = "00000000-0000-4000-8000-000000000001";
        player_points::Ledger {
            baseline: player_points::Baseline {
                day: "2020-01-01".into(),
                balance: "3.125".into(),
                actor: "player baseline actor".into(),
                reason: "player baseline reason".into(),
                evidence: "player baseline evidence".into(),
            },
            entries: vec![
                player_points::Command {
                    id: target.into(),
                    kind: player_points::Kind::Award,
                    amount: "0.875".into(),
                    day: "2024-01-10".into(),
                    actor: "points actor secret".into(),
                    reason: "points reason secret".into(),
                    evidence: "points evidence secret".into(),
                    reversal_of: None,
                },
                player_points::Command {
                    id: "00000000-0000-4000-8000-000000000002".into(),
                    kind: player_points::Kind::Reversal,
                    amount: reversal_amount.into(),
                    day: "2024-02-10".into(),
                    actor: "points reversal actor secret".into(),
                    reason: "points reversal reason secret".into(),
                    evidence: "points reversal evidence secret".into(),
                    reversal_of: Some(target.into()),
                },
            ],
        }
    }

    #[test]
    fn player_points_reversal_keeps_its_signed_decimal_and_pool_label() {
        let legacy = xp_view(None);
        let history = player(
            "Player Example",
            Some(&points_ledger("-0.875")),
            &legacy,
            22,
            "2024-02-10",
            "2024-02-10",
            false,
        )
        .unwrap();
        assert_eq!(
            history.snapshot,
            CurrentSnapshot::PlayerPoints {
                balance: "3.125".into()
            }
        );
        assert_eq!(history.posted_excluded, 1);
        assert_eq!(
            history.posted[0].delta,
            PostedDelta::PlayerPoints {
                balance: "-0.875".into()
            }
        );
        let output = html(&history, Language::English).unwrap();
        assert!(output.contains("Player Point History"));
        assert!(output.contains("Posted Player Points entries"));
        assert!(output.contains("Imported Player Point history"));
        assert!(!output.contains("Character XP"));
        assert!(!output.contains("Experience History"));
    }

    #[test]
    fn privacy_escaping_and_reason_opt_in_are_enforced() {
        let mut ledger = xp_reversal_ledger();
        ledger.entries.truncate(1);
        let view = xp_view(Some(ledger));
        let private = character(&view, 1, FROM, THROUGH, false).unwrap();
        let output = html(&private, Language::English).unwrap();
        for secret in [
            "record secret",
            "award-id-secret",
            "actor secret",
            "evidence secret",
            "library secret",
            "baseline actor secret",
            "baseline reason secret",
            "reason secret",
        ] {
            assert!(!output.contains(secret), "leaked {secret}");
        }
        assert!(output.contains("A &lt;Character&gt; &amp; friend"));
        assert!(output.contains("Content-Security-Policy"));
        assert!(output.contains("@media print"));

        let opted_in = character(&view, 1, FROM, THROUGH, true).unwrap();
        let opted_output = html(&opted_in, Language::Portuguese).unwrap();
        assert!(opted_output.contains("reason secret"));
        assert!(opted_output.contains("lang=\"pt\""));
    }

    #[test]
    fn invalid_ranges_and_corrupt_ledgers_are_rejected_before_filtering() {
        let invalid_range = character(
            &xp_view(Some(xp_reversal_ledger())),
            1,
            "2024-02-30",
            "2024-03-01",
            false,
        );
        assert!(invalid_range.is_err());

        let mut corrupt_xp = xp_reversal_ledger();
        corrupt_xp.entries[2].command.amount = "2".into();
        assert!(
            character(
                &xp_view(Some(corrupt_xp)),
                1,
                "2024-01-01",
                "2024-01-01",
                false
            )
            .is_err()
        );

        let corrupt_points = points_ledger("-0.5");
        assert!(
            player(
                "Player Example",
                Some(&corrupt_points),
                &xp_view(None),
                1,
                "2024-01-10",
                "2024-01-10",
                false
            )
            .is_err()
        );
    }
}
