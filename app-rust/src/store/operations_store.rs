use super::*;
use crate::operations::{
    self, CalendarLine, CalendarState, CharacterLine, Content, Request, SignInLine, Snapshot,
};
use std::collections::{BTreeMap, BTreeSet};

fn text<'a>(r: &'a Record, key: &str) -> &'a str {
    field(r, key).and_then(Value::as_str).unwrap_or("")
}
fn field<'a>(r: &'a Record, key: &str) -> Option<&'a Value> {
    r.private_fields
        .get(key)
        .or_else(|| r.public_fields.get(key))
}
fn status(r: &Record) -> &str {
    let s = text(r, "Status");
    if s.is_empty() {
        text(r, "PlayStatus")
    } else {
        s
    }
}
fn npc(r: &Record) -> bool {
    crate::owbn_bylaws::is_npc(r)
}
fn live(r: &Record) -> bool {
    r.build.is_none() && crate::sheet::is_character_kind(&r.kind)
}
fn player_labels(records: &[Record]) -> BTreeMap<&str, String> {
    records
        .iter()
        .filter(|r| r.kind == "Player" && r.build.is_none())
        .map(|r| (r.id.as_str(), record_name(r, Role::Storyteller)))
        .collect()
}
fn player_label(r: &Record, labels: &BTreeMap<&str, String>, include: bool) -> String {
    if !include {
        return String::new();
    }
    r.player_id
        .as_deref()
        .and_then(|id| labels.get(id))
        .cloned()
        .unwrap_or_else(|| text(r, "Player").to_owned())
}

impl Library {
    pub fn operations_snapshot(&self, request: &Request, role: Role) -> Result<Snapshot> {
        require_st(role)?;
        operations::validate_request(request)?;
        let revision = self.revision()?;
        let content = match request {
            Request::VampireStatus {
                status,
                include_boons,
                include_descriptions,
            } => Content::VampireStatus(Box::new(self.vampire_status_snapshot(
                status,
                *include_boons,
                *include_descriptions,
                role,
            )?)),
            Request::PlayerRoster {
                status: scope,
                include_contacts,
                include_legacy_ids,
            } => {
                let records = self.records()?;
                let mut rows: Vec<_> = records
                    .iter()
                    .filter(|r| {
                        r.kind == "Player"
                            && r.build.is_none()
                            && (scope.is_empty() || status(r).eq_ignore_ascii_case(scope.trim()))
                    })
                    .map(|r| crate::operations::PlayerLine {
                        name: record_name(r, role),
                        position: text(r, "Position").into(),
                        status: status(r).into(),
                        legacy_id: if *include_legacy_ids {
                            r.legacy_id.clone().unwrap_or_default()
                        } else {
                            String::new()
                        },
                        email: if *include_contacts {
                            text(r, "EMail").into()
                        } else {
                            String::new()
                        },
                        phone: if *include_contacts {
                            text(r, "Phone").into()
                        } else {
                            String::new()
                        },
                    })
                    .collect();
                rows.sort_by(|a, b| a.name.cmp(&b.name));
                Content::PlayerRoster {
                    status: scope.clone(),
                    include_contacts: *include_contacts,
                    include_legacy_ids: *include_legacy_ids,
                    rows,
                }
            }
            Request::CharacterHistory {
                record_id,
                from,
                through,
                include_reasons,
            } => {
                let record = self.stored_record(record_id)?;
                if !live(&record) {
                    return Err(error(
                        "REPORT_CHARACTER_REQUIRED",
                        "Choose a live character.",
                    ));
                }
                let view =
                    super::ledger_legacy::report_legacy(&record, self.xp_view(record_id, role)?);
                Content::History(Box::new(crate::ledger_reports::character(
                    &view,
                    revision,
                    from,
                    through,
                    *include_reasons,
                )?))
            }
            Request::PlayerHistory {
                record_id,
                from,
                through,
                include_reasons,
            } => {
                let mut record = self.stored_record(record_id)?;
                if record.kind != "Player" || record.build.is_some() {
                    return Err(error(
                        "REPORT_PLAYER_REQUIRED",
                        "Choose a live Player record.",
                    ));
                }
                let ledger = self.player_points(record_id, role)?;
                record.xp_ledger = None;
                let legacy = super::ledger_legacy::report_legacy(&record, crate::xp::view(&record));
                Content::History(Box::new(crate::ledger_reports::player(
                    &record.name,
                    ledger.as_ref(),
                    &legacy,
                    revision,
                    from,
                    through,
                    *include_reasons,
                )?))
            }
            Request::Roster {
                family,
                status: scope,
                include_players,
            } => {
                let records = self.records()?;
                let labels = if *include_players {
                    player_labels(&records)
                } else {
                    BTreeMap::new()
                };
                let mut rows: Vec<_> = records
                    .iter()
                    .filter(|r| {
                        live(r)
                            && (family.is_empty() || r.kind == *family)
                            && (scope.is_empty() || status(r).eq_ignore_ascii_case(scope.trim()))
                    })
                    .map(|r| CharacterLine {
                        name: record_name(r, role),
                        family: r.kind.clone(),
                        group: text(r, "Group").into(),
                        subgroup: text(r, "Subgroup").into(),
                        status: status(r).into(),
                        npc: npc(r),
                        player: player_label(r, &labels, *include_players),
                    })
                    .collect();
                rows.sort_by(|a, b| a.name.cmp(&b.name).then(a.family.cmp(&b.family)));
                Content::Roster {
                    family: family.clone(),
                    status: scope.clone(),
                    include_players: *include_players,
                    rows,
                }
            }
            Request::SignIn {
                schedule_id,
                include_players,
            } => {
                let plan = self
                    .scheduled_sessions(role)?
                    .into_iter()
                    .find(|p| p.request.id == *schedule_id && !p.cancelled)
                    .ok_or_else(|| {
                        error(
                            "REPORT_PLAN_REQUIRED",
                            "Choose a saved plan that has not been cancelled or completed.",
                        )
                    })?;
                let records = self.records()?;
                let labels = if *include_players {
                    player_labels(&records)
                } else {
                    BTreeMap::new()
                };
                let by_id: BTreeMap<_, _> = records
                    .iter()
                    .filter(|r| live(r))
                    .map(|r| (r.id.as_str(), r))
                    .collect();
                let mut missing = 0;
                let mut rows = Vec::new();
                for attendance in &plan.request.attendance {
                    if let Some(r) = by_id.get(attendance.record_id.as_str()) {
                        rows.push(SignInLine {
                            name: record_name(r, role),
                            player: player_label(r, &labels, *include_players),
                        });
                    } else {
                        missing += 1;
                    }
                }
                rows.sort_by(|a, b| a.name.cmp(&b.name));
                Content::SignIn {
                    day: plan.request.day,
                    title: plan.request.title,
                    include_players: *include_players,
                    missing,
                    rows,
                }
            }
            Request::Calendar {
                from,
                through,
                include_cancelled,
            } => {
                let in_range = |day: &str| {
                    day >= from.as_str() && day <= through.as_str() && crate::xp::day_valid(day)
                };
                let receipts = self.session_receipts(role)?;
                let completed: BTreeSet<_> = receipts
                    .iter()
                    .map(|r| r.quote.request.id.as_str())
                    .collect();
                let mut rows = Vec::new();
                for p in self.scheduled_sessions(role)? {
                    if (!*include_cancelled && p.cancelled)
                        || !in_range(&p.request.day)
                        || completed.contains(p.request.id.as_str())
                    {
                        continue;
                    }
                    rows.push(CalendarLine {
                        day: p.request.day,
                        title: p.request.title,
                        selected: p.request.attendance.len(),
                        state: if p.cancelled {
                            CalendarState::Cancelled
                        } else {
                            CalendarState::Planned
                        },
                        attended: None,
                    });
                }
                for r in &receipts {
                    let request = &r.quote.request;
                    if in_range(&request.day) {
                        rows.push(CalendarLine {
                            day: request.day.clone(),
                            title: request.title.clone(),
                            state: CalendarState::Completed,
                            selected: request.attendance.len(),
                            attended: Some(request.attendance.iter().filter(|a| a.present).count()),
                        });
                    }
                }
                rows.sort_by(|a, b| a.day.cmp(&b.day).then(a.title.cmp(&b.title)));
                Content::Calendar {
                    from: from.clone(),
                    through: through.clone(),
                    include_cancelled: *include_cancelled,
                    rows,
                }
            }
        };
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed during the report. Generate it again.",
            ));
        }
        Ok(Snapshot {
            title: self.title.clone(),
            revision,
            content,
        })
    }
}
