use crate::{
    action_links::Timeline,
    model::{Language, Result, error},
    story_work::{self, Draft},
    world_card::escape,
};
use std::fmt::Write;
pub fn html(timeline: &Timeline, language: Language) -> Result<String> {
    story_work::validate_range(&timeline.from, &timeline.through)?;
    if timeline.rows.len() > 500
        || timeline.rows.iter().any(|row| {
            row.snapshot.revision != timeline.revision
                || !matches!(row.snapshot.draft, Draft::Action(_))
                || row.snapshot.draft.day() < timeline.from.as_str()
                || row.snapshot.draft.day() > timeline.through.as_str()
        })
    {
        return Err(error(
            "ACTION_TIMELINE_SCOPE",
            "An Action is outside this timeline snapshot.",
        ));
    }
    let t = |en, pt| {
        if language == Language::Portuguese {
            pt
        } else {
            en
        }
    };
    let mut out = story_work::header(
        t(
            "Character Action timeline",
            "Linha do tempo de ações do personagem",
        ),
        language,
        timeline.revision,
    );
    let _ = write!(
        out,
        "<h2>{}</h2><p>{} {} – {}</p><p>{} {} · {} {} · {} {}</p>",
        escape(&timeline.character_name),
        t("Inclusive dates", "Datas inclusivas"),
        escape(&timeline.from),
        escape(&timeline.through),
        t("Linked Actions shown", "Ações vinculadas exibidas"),
        timeline.rows.len(),
        t(
            "Linked Actions with unresolved dates excluded",
            "Ações vinculadas com datas não resolvidas excluídas"
        ),
        timeline.undated_omitted,
        t(
            "Unlinked Actions in the library requiring review",
            "Ações sem vínculo na biblioteca que requerem revisão"
        ),
        timeline.unlinked_in_library
    );
    for row in &timeline.rows {
        story_work::section(&mut out, &row.snapshot, language);
        if out.len() > 8 * 1024 * 1024 {
            return Err(error(
                "ACTION_TIMELINE_LIMIT",
                "Narrow the timeline date range for printing.",
            ));
        }
    }
    if timeline.rows.is_empty() {
        let _ = write!(
            out,
            "<p>{}</p>",
            t(
                "No linked Actions in this date range.",
                "Nenhuma ação vinculada neste intervalo de datas."
            )
        );
    }
    out.push_str("</html>");
    Ok(out)
}
