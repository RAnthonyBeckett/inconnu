use crate::{
    combined_story::Snapshot,
    model::{Language, Result, error},
    story_work::{self, Draft},
    world_card::escape,
};
use std::fmt::Write;

pub fn html(snapshot: &Snapshot, language: Language) -> Result<String> {
    let actions = &snapshot.actions;
    story_work::validate_range(&snapshot.request.from, &snapshot.request.through)?;
    if snapshot.request.character_id != actions.character_id
        || snapshot.request.from != actions.from
        || snapshot.request.through != actions.through
        || actions.rows.len() + snapshot.rumors.len() > 500
    {
        return Err(error(
            "COMBINED_STORY_SCOPE",
            "Prepare a fresh combined character timeline.",
        ));
    }
    let mut rows = Vec::with_capacity(actions.rows.len() + snapshot.rumors.len());
    for row in &actions.rows {
        if !matches!(row.snapshot.draft, Draft::Action(_)) {
            return Err(error(
                "COMBINED_STORY_SCOPE",
                "An Action row has the wrong type.",
            ));
        }
        rows.push((row.action_id.as_str(), &row.snapshot));
    }
    for row in &snapshot.rumors {
        if !matches!(row.snapshot.draft, Draft::Rumor(_)) {
            return Err(error(
                "COMBINED_STORY_SCOPE",
                "A Rumor row has the wrong type.",
            ));
        }
        rows.push((row.record_id.as_str(), &row.snapshot));
    }
    if rows.iter().any(|(_, s)| {
        s.revision != actions.revision
            || s.draft.day() < actions.from.as_str()
            || s.draft.day() > actions.through.as_str()
    }) {
        return Err(error(
            "COMBINED_STORY_SCOPE",
            "A saved entry is outside this timeline snapshot.",
        ));
    }
    rows.sort_by(|(left_id, left), (right_id, right)| {
        left.draft
            .day()
            .cmp(right.draft.day())
            .then(
                left.draft
                    .kind()
                    .record_kind()
                    .cmp(right.draft.kind().record_kind()),
            )
            .then(left_id.cmp(right_id))
    });
    let t = |en, pt| {
        if language == Language::Portuguese {
            pt
        } else {
            en
        }
    };
    let mut out = story_work::header(
        t(
            "Character Actions and Rumors",
            "Ações e rumores do personagem",
        ),
        language,
        actions.revision,
    );
    let _ = write!(
        out,
        "<h2>{}</h2><p>{} {} – {}</p><p>{}</p>",
        escape(&actions.character_name),
        t("Inclusive dates", "Datas inclusivas"),
        escape(&actions.from),
        escape(&actions.through),
        t(
            "Rumors are selected through saved local staff audience lists. This report records no sending, receipt or character access.",
            "Os rumores são selecionados pelas listas locais salvas da equipe. Este relatório não registra envio, recebimento nem acesso do personagem."
        )
    );
    for (label, count) in [
        (
            t("Linked Actions shown", "Ações vinculadas exibidas"),
            actions.rows.len(),
        ),
        (
            t(
                "Staff-selected Rumors shown",
                "Rumores selecionados pela equipe exibidos",
            ),
            snapshot.rumors.len(),
        ),
        (
            t(
                "Linked Actions with unresolved dates omitted",
                "Ações vinculadas com datas não resolvidas omitidas",
            ),
            actions.undated_omitted,
        ),
        (
            t(
                "Undated Rumors omitted from the library-wide queue",
                "Rumores sem data omitidos da fila de toda a biblioteca",
            ),
            snapshot.undated_rumors,
        ),
        (
            t(
                "Unlinked Actions in the entire library",
                "Ações sem vínculo em toda a biblioteca",
            ),
            actions.unlinked_in_library,
        ),
        (
            t(
                "Rumors without a saved audience in this date range",
                "Rumores sem público salvo neste intervalo de datas",
            ),
            snapshot.unassigned_rumors_in_range,
        ),
    ] {
        let _ = write!(out, "<p>{}: {count}</p>", escape(label));
    }
    for (_, row) in rows {
        story_work::section(&mut out, row, language);
        if out.len() > 8 * 1024 * 1024 {
            return Err(error(
                "COMBINED_STORY_LIMIT",
                "Narrow the timeline dates before printing.",
            ));
        }
    }
    out.push_str("</html>");
    Ok(out)
}
