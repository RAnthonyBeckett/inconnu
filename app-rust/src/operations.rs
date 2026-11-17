//! Read-only operational report snapshots. Saved roster and calendar data only.
use crate::{
    model::{Language, Result, error},
    world_card::escape,
    xp,
};
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    VampireStatus {
        status: String,
        include_boons: bool,
        include_descriptions: bool,
    },
    PlayerRoster {
        status: String,
        include_contacts: bool,
        include_legacy_ids: bool,
    },
    CharacterHistory {
        record_id: String,
        from: String,
        through: String,
        include_reasons: bool,
    },
    PlayerHistory {
        record_id: String,
        from: String,
        through: String,
        include_reasons: bool,
    },
    Roster {
        family: String,
        status: String,
        include_players: bool,
    },
    SignIn {
        schedule_id: String,
        include_players: bool,
    },
    Calendar {
        from: String,
        through: String,
        include_cancelled: bool,
    },
}
#[derive(Clone, Debug)]
pub struct PlayerLine {
    pub name: String,
    pub position: String,
    pub status: String,
    pub legacy_id: String,
    pub email: String,
    pub phone: String,
}
#[derive(Clone, Debug)]
pub struct CharacterLine {
    pub name: String,
    pub family: String,
    pub group: String,
    pub subgroup: String,
    pub status: String,
    pub npc: bool,
    pub player: String,
}
#[derive(Clone, Debug)]
pub struct SignInLine {
    pub name: String,
    pub player: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarState {
    Planned,
    Cancelled,
    Completed,
}
#[derive(Clone, Debug)]
pub struct CalendarLine {
    pub day: String,
    pub title: String,
    pub state: CalendarState,
    pub selected: usize,
    pub attended: Option<usize>,
}
#[derive(Clone, Debug)]
pub enum Content {
    VampireStatus(Box<crate::vampire_status::Snapshot>),
    PlayerRoster {
        status: String,
        include_contacts: bool,
        include_legacy_ids: bool,
        rows: Vec<PlayerLine>,
    },
    History(Box<crate::ledger_reports::History>),
    Roster {
        family: String,
        status: String,
        include_players: bool,
        rows: Vec<CharacterLine>,
    },
    SignIn {
        day: String,
        title: String,
        include_players: bool,
        missing: usize,
        rows: Vec<SignInLine>,
    },
    Calendar {
        from: String,
        through: String,
        include_cancelled: bool,
        rows: Vec<CalendarLine>,
    },
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub title: String,
    pub revision: i64,
    pub content: Content,
}

pub(crate) fn validate_request(req: &Request) -> Result<()> {
    let short = |s: &str| s.len() <= 256 && !s.contains('\0');
    match req {
        Request::PlayerRoster { status, .. } | Request::VampireStatus { status, .. }
            if !short(status) =>
        {
            Err(error(
                "REPORT_SCOPE_INVALID",
                "Enter a short saved-status filter.",
            ))
        }
        Request::CharacterHistory {
            record_id,
            from,
            through,
            ..
        }
        | Request::PlayerHistory {
            record_id,
            from,
            through,
            ..
        } => {
            if record_id.is_empty()
                || !short(record_id)
                || !xp::day_valid(from)
                || !xp::day_valid(through)
                || from > through
            {
                return Err(error(
                    "REPORT_HISTORY_SCOPE",
                    "Choose a saved record and an inclusive YYYY-MM-DD range.",
                ));
            }
            Ok(())
        }
        Request::Roster { family, status, .. }
            if !short(family)
                || !short(status)
                || (!family.is_empty() && !crate::sheet::is_character_kind(family)) =>
        {
            Err(error(
                "REPORT_SCOPE_INVALID",
                "Choose a supported family and a short status filter.",
            ))
        }
        Request::SignIn { schedule_id, .. } if schedule_id.is_empty() || !short(schedule_id) => {
            Err(error(
                "REPORT_PLAN_REQUIRED",
                "Choose a saved session plan.",
            ))
        }
        Request::Calendar { from, through, .. }
            if !xp::day_valid(from) || !xp::day_valid(through) || from > through =>
        {
            Err(error(
                "REPORT_DATES_INVALID",
                "Enter an inclusive YYYY-MM-DD range in chronological order.",
            ))
        }
        _ => Ok(()),
    }
}

fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
fn table(out: &mut String, headers: &[&str]) {
    out.push_str("<table><thead><tr>");
    for h in headers {
        let _ = write!(out, "<th>{}</th>", escape(h));
    }
    out.push_str("</tr></thead><tbody>");
}
fn row(out: &mut String, values: &[&str]) -> Result<()> {
    out.push_str("<tr>");
    for v in values {
        let _ = write!(out, "<td>{}</td>", escape(v));
    }
    out.push_str("</tr>");
    if out.len() > 8 * 1024 * 1024 {
        return Err(error(
            "REPORT_SIZE_LIMIT",
            "The report is too large. Narrow the selection.",
        ));
    }
    Ok(())
}

pub fn html(snapshot: &Snapshot, l: Language) -> Result<String> {
    let heading = match &snapshot.content {
        Content::PlayerRoster { .. } => t(l, "Player roster", "Lista de jogadores"),
        Content::History(h) => return crate::ledger_reports::html(h, l),
        Content::VampireStatus(s) => return crate::vampire_status::html(s, l),
        Content::Roster { .. } => t(l, "Character roster", "Lista de personagens"),
        Content::SignIn { .. } => t(l, "Blank sign-in sheet", "Folha de presença em branco"),
        Content::Calendar { .. } => t(l, "Session calendar", "Calendário de sessões"),
    };
    let mut out = format!(
        "<!doctype html><html lang=\"{}\"><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{}</title><style>body{{font:16px system-ui;margin:2rem auto;max-width:75rem;padding:0 1rem}}table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #777;padding:.45rem;white-space:pre-wrap;overflow-wrap:anywhere}}th{{text-align:left}}tr{{break-inside:avoid}}thead{{display:table-header-group}}@media print{{body{{margin:0;max-width:none}}}}</style><h1>{}</h1><p><strong>{}</strong> · {} · {} {}</p>",
        t(l, "en", "pt-BR"),
        heading,
        heading,
        t(
            l,
            "Private Storyteller report",
            "Relatório privado do Narrador"
        ),
        escape(&snapshot.title),
        t(l, "Library revision", "Revisão da biblioteca"),
        snapshot.revision
    );
    match &snapshot.content {
        Content::PlayerRoster {
            status,
            include_contacts,
            include_legacy_ids,
            rows,
        } => {
            let _ = write!(
                out,
                "<p>{}: {} · {}: {}</p>",
                t(l, "Players", "Jogadores"),
                rows.len(),
                t(l, "Status filter", "Filtro de situação"),
                escape(status)
            );
            let mut headers = vec![
                t(l, "Player", "Jogador"),
                t(l, "Position", "Função"),
                t(l, "Status", "Situação"),
            ];
            if *include_legacy_ids {
                headers.push(t(l, "Legacy identifier", "Identificador legado"));
            }
            if *include_contacts {
                headers.extend([t(l, "Email", "E-mail"), t(l, "Phone", "Telefone")]);
            }
            table(&mut out, &headers);
            for p in rows {
                let mut cells = vec![p.name.as_str(), p.position.as_str(), p.status.as_str()];
                if *include_legacy_ids {
                    cells.push(p.legacy_id.as_str());
                }
                if *include_contacts {
                    cells.extend([p.email.as_str(), p.phone.as_str()]);
                }
                row(&mut out, &cells)?;
            }
        }
        Content::History(h) => return crate::ledger_reports::html(h, l),
        Content::VampireStatus(s) => return crate::vampire_status::html(s, l),
        Content::Roster {
            family,
            status,
            include_players,
            rows,
        } => {
            let all = t(l, "All", "Todos");
            let _ = write!(
                out,
                "<p>{}: {} · {}: {} · {}: {}</p>",
                t(l, "Family", "Família"),
                if family.is_empty() {
                    all.into()
                } else {
                    escape(family)
                },
                t(l, "Status filter", "Filtro de situação"),
                if status.is_empty() {
                    all.into()
                } else {
                    escape(status)
                },
                t(l, "Characters", "Personagens"),
                rows.len()
            );
            let mut headers = vec![
                t(l, "Character", "Personagem"),
                t(l, "Family", "Família"),
                t(l, "Group", "Grupo"),
                t(l, "Subgroup", "Subgrupo"),
                t(l, "Status / NPC", "Situação / NPC"),
            ];
            if *include_players {
                headers.push(t(l, "Player", "Jogador"));
            }
            table(&mut out, &headers);
            for c in rows {
                let status = if c.npc {
                    format!("{} NPC", c.status).trim().to_owned()
                } else {
                    c.status.clone()
                };
                let mut values = vec![
                    c.name.as_str(),
                    c.family.as_str(),
                    c.group.as_str(),
                    c.subgroup.as_str(),
                    status.as_str(),
                ];
                if *include_players {
                    values.push(c.player.as_str());
                }
                row(&mut out, &values)?;
            }
        }
        Content::SignIn {
            day,
            title,
            include_players,
            missing,
            rows,
        } => {
            let _ = write!(
                out,
                "<h2>{} · {}</h2><p>{}: {} · {}: {}</p><p>{}</p>",
                escape(day),
                escape(title),
                t(l, "Saved attendees", "Participantes salvos"),
                rows.len(),
                t(
                    l,
                    "Unavailable rows omitted",
                    "Linhas indisponíveis omitidas"
                ),
                missing,
                t(
                    l,
                    "Blank for check-in. Printing does not record attendance or award XP.",
                    "Em branco para registrar presença. A impressão não salva presença nem concede XP."
                )
            );
            let mut headers = vec![
                t(l, "Attended?", "Presente?"),
                t(l, "Character", "Personagem"),
            ];
            if *include_players {
                headers.push(t(l, "Player", "Jogador"));
            }
            table(&mut out, &headers);
            for c in rows {
                let mut values = vec!["", &c.name];
                if *include_players {
                    values.push(&c.player);
                }
                row(&mut out, &values)?;
            }
        }
        Content::Calendar {
            from,
            through,
            include_cancelled,
            rows,
        } => {
            let _ = write!(
                out,
                "<p>{}: {} – {} · {} · {}: {}</p>",
                t(l, "Inclusive range", "Intervalo inclusivo"),
                escape(from),
                escape(through),
                if *include_cancelled {
                    t(l, "Includes cancelled plans", "Inclui planos cancelados")
                } else {
                    t(l, "Cancelled plans excluded", "Planos cancelados excluídos")
                },
                t(l, "Sessions", "Sessões"),
                rows.len()
            );
            table(
                &mut out,
                &[
                    t(l, "Date", "Data"),
                    t(l, "Session", "Sessão"),
                    t(l, "Status", "Situação"),
                    t(l, "Selected", "Selecionados"),
                    t(l, "Attended", "Presentes"),
                ],
            );
            for c in rows {
                let state = match c.state {
                    CalendarState::Planned => t(l, "Planned", "Planejada"),
                    CalendarState::Cancelled => t(l, "Cancelled", "Cancelada"),
                    CalendarState::Completed => t(l, "Completed", "Concluída"),
                };
                let selected = c.selected.to_string();
                let attended = c.attended.map_or_else(|| "—".into(), |n| n.to_string());
                row(&mut out, &[&c.day, &c.title, state, &selected, &attended])?;
            }
        }
    }
    out.push_str("</tbody></table></html>");
    Ok(out)
}
