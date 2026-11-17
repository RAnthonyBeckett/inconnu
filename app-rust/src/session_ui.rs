use std::collections::BTreeMap;

use eframe::egui;
use grapevine_workbench::{
    game_sessions::{Attendance, SessionQuote, SessionRequest},
    model::{Language, Summary},
    sheet,
    store::SessionReceipt,
    xp,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub included: bool,
    pub present: bool,
    pub award_enabled: bool,
    pub amount: String,
}

pub struct Form {
    pub legacy_origin: Option<grapevine_workbench::legacy_calendar::Origin>,
    pub request: SessionRequest,
    pub quote: Option<SessionQuote>,
    pub revision: Option<i64>,
    pub receipts: Vec<SessionReceipt>,
    pub schedules: Vec<grapevine_workbench::store::ScheduledSession>,
    pub message: Option<String>,
    pub dirty: bool,
    pub rows: BTreeMap<String, Row>,
    pub document_language: Language,
    pub bulk_amount: String,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            legacy_origin: None,
            request: SessionRequest {
                id: uuid::Uuid::new_v4().to_string(),
                title: String::new(),
                day: String::new(),
                actor: String::new(),
                reason: String::new(),
                evidence: String::new(),
                attendance: Vec::new(),
            },
            quote: None,
            revision: None,
            receipts: Vec::new(),
            schedules: Vec::new(),
            message: None,
            dirty: false,
            rows: BTreeMap::new(),
            document_language: Language::English,
            bulk_amount: String::new(),
        }
    }
}

impl Form {
    pub fn from_roster(summaries: &[Summary]) -> Self {
        let mut form = Self::default();
        form.sync_request(summaries);
        form
    }

    /// Keep UI rows aligned to live characters and derive the request from included rows.
    pub fn sync_request(&mut self, summaries: &[Summary]) {
        let live_ids: std::collections::BTreeSet<&str> = summaries
            .iter()
            .filter(|summary| is_live_character(summary))
            .map(|summary| summary.id.as_str())
            .collect();
        self.rows.retain(|id, _| live_ids.contains(id.as_str()));
        for id in &live_ids {
            self.rows.entry((*id).to_owned()).or_default();
        }

        self.request.attendance = self
            .rows
            .iter()
            .filter(|(_, row)| row.included)
            .map(|(record_id, row)| Attendance {
                record_id: record_id.clone(),
                present: row.present,
                award: (row.present && row.award_enabled).then(|| row.amount.clone()),
            })
            .collect();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    SaveSchedule,
    LoadSchedule(usize),
    CancelSchedule(usize),
    Preview,
    Commit,
    New,
    RefreshHistory,
    ExportReceipt(usize),
}

fn t<'a>(pt: bool, en: &'a str, br: &'a str) -> &'a str {
    if pt { br } else { en }
}

fn is_live_character(summary: &Summary) -> bool {
    summary.build.is_none() && sheet::is_character_kind(&summary.kind)
}

fn display_label(record_id: &str, summaries: &[Summary]) -> String {
    summaries
        .iter()
        .find(|summary| summary.id == record_id && is_live_character(summary))
        .map(|summary| {
            if summary.player_name.trim().is_empty() {
                format!("{} · {}", summary.name, summary.kind)
            } else {
                format!(
                    "{} · {} · {}",
                    summary.name, summary.player_name, summary.kind
                )
            }
        })
        .unwrap_or_else(|| record_id.to_owned())
}

fn positive_amount(value: &str) -> bool {
    xp::amount(value).is_ok_and(|units| units > 0)
}

pub fn render(
    ui: &mut egui::Ui,
    form: &mut Form,
    summaries: &[Summary],
    revision: i64,
    language: Language,
) -> Option<Action> {
    let pt = language == Language::Portuguese;
    form.sync_request(summaries);
    let before_request = form.request.clone();
    let before_rows = form.rows.clone();
    let mut action = None;

    egui::ScrollArea::vertical()
        .id_salt("game_session_form")
        .max_height(690.0)
        .show(ui, |ui| {
            ui.heading(t(pt, "Game session", "Sessão de jogo"));
            ui.label(t(
                pt,
                "Awards are manual declarations. Inconnu does not verify OWbN session-award rules.",
                "As concessões são declarações manuais. O Inconnu não verifica regras de concessão de XP da OWbN.",
            ));
            if let Some(origin)=&form.legacy_origin {
                ui.collapsing(t(pt,"Private retained calendar details","Detalhes privados do calendário retido"),|ui|{
                    ui.label(t(pt,"Historical source details; the current plan can be edited separately. No attendance or XP is implied.","Detalhes históricos da fonte; o plano atual pode ser editado separadamente. Nenhuma presença ou XP é presumida."));
                    ui.label(&origin.source_name);
                    for (en,br,value) in [("Original date","Data original",&origin.entry.raw_date),("Original time","Horário original",&origin.entry.time),("Original place","Local original",&origin.entry.place),("Original notes","Notas originais",&origin.entry.notes),("Imported ISO day","Data ISO importada",&origin.imported_day),("Imported title","Título importado",&origin.imported_title)]{
                        ui.label(format!("{}: {}",t(pt,en,br),value));
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(t(pt, "Receipt language", "Idioma do recibo"));
                ui.selectable_value(&mut form.document_language, Language::English, "English");
                ui.selectable_value(
                    &mut form.document_language,
                    Language::Portuguese,
                    "Português",
                );
            });

            ui.horizontal_wrapped(|ui| {
                let saved=form.receipts.iter().any(|r|r.quote.request.id==form.request.id);
                let preview = ui.add_enabled(!saved,egui::Button::new(t(pt, "Preview session", "Pré-visualizar sessão")));
                if saved {ui.small(t(pt,"This session is saved. Choose New session to record another.","Esta sessão está salva. Escolha Nova sessão para registrar outra."));}
                let rect = ui.is_rect_visible(preview.rect).then_some(preview.rect);
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::unique("session_preview"), rect)
                });
                if preview.clicked() {
                    action = Some(Action::Preview);
                }

                let current = form.quote.as_ref().is_some_and(|quote| {
                    quote.revision == revision && quote.request == form.request
                });
                let commit = ui.add_enabled(
                    current,
                    egui::Button::new(t(pt, "Commit session", "Registrar sessão")),
                );
                let rect = ui.is_rect_visible(commit.rect).then_some(commit.rect);
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::unique("session_commit"), rect)
                });
                if commit.clicked() {
                    action = Some(Action::Commit);
                }
            });

            let schedule=ui.button(t(pt,"Save scheduled event (no XP awarded)","Salvar evento agendado (sem conceder XP)"));
            let visible=ui.is_rect_visible(schedule.rect).then_some(schedule.rect);
            ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique("session_schedule"),visible));
            if schedule.clicked(){action=Some(Action::SaveSchedule);}
            egui::CollapsingHeader::new(t(pt,"Calendar / scheduled events","Calendário / eventos agendados")).default_open(true).show(ui,|ui|{
                for (index,event) in form.schedules.iter().enumerate(){
                    ui.horizontal_wrapped(|ui|{
                        ui.label(format!("{} · {}",event.request.day,event.request.title));
                        if event.cancelled {ui.small(t(pt,"Cancelled","Cancelado"));}
                        else {
                            let load=ui.button(t(pt,"Load event","Carregar evento"));
                            let visible=ui.is_rect_visible(load.rect).then_some(load.rect);
                            ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique(format!("session_load{index}")),visible));
                            if load.clicked(){action=Some(Action::LoadSchedule(index));}
                            if ui.small_button(t(pt,"Cancel event","Cancelar evento")).clicked(){action=Some(Action::CancelSchedule(index));}
                        }
                    });
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                let new = ui.button(t(pt, "New session", "Nova sessão"));
                let rect = ui.is_rect_visible(new.rect).then_some(new.rect);
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::unique("session_new"), rect)
                });
                if new.clicked() {
                    action = Some(Action::New);
                }
                if ui
                    .button(t(pt, "Refresh history", "Atualizar histórico"))
                    .clicked()
                {
                    action = Some(Action::RefreshHistory);
                }
            });

            ui.horizontal_wrapped(|ui| {
                ui.label(t(pt, "Title", "Título"));
                ui.add(
                    egui::TextEdit::singleline(&mut form.request.title)
                        .desired_width(250.0)
                        .hint_text(t(pt, "Session title", "Título da sessão")),
                );
                ui.label(t(pt, "Date (YYYY-MM-DD)", "Data (AAAA-MM-DD)"));
                ui.add(
                    egui::TextEdit::singleline(&mut form.request.day)
                        .desired_width(120.0)
                        .hint_text("YYYY-MM-DD"),
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(t(pt, "Recorded by", "Registrado por"));
                ui.add(
                    egui::TextEdit::singleline(&mut form.request.actor)
                        .desired_width(250.0),
                );
            });
            ui.label(t(pt, "Reason", "Motivo"));
            ui.add(
                egui::TextEdit::multiline(&mut form.request.reason)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            ui.label(t(pt, "Evidence reference", "Referência de evidência"));
            ui.add(
                egui::TextEdit::multiline(&mut form.request.evidence)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );

            ui.separator();
            ui.heading(t(pt, "Attendance and manual awards", "Presença e concessões manuais"));
            ui.label(t(
                pt,
                "Include and present are separate choices. Only included rows enter this session.",
                "Incluir e presente são opções separadas. Apenas linhas incluídas entram nesta sessão.",
            ));
            ui.horizontal_wrapped(|ui| {
                if ui.button(t(pt,"Include all","Incluir todos")).clicked() { for row in form.rows.values_mut(){ row.included=true; } }
                if ui.button(t(pt,"Mark included present","Marcar incluídos como presentes")).clicked() { for row in form.rows.values_mut().filter(|r|r.included){row.present=true;} }
                if ui.button(t(pt,"Clear selection","Limpar seleção")).clicked() { for row in form.rows.values_mut(){*row=Row::default();} }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(t(pt,"Group manual XP","XP manual do grupo"));
                ui.add(egui::TextEdit::singleline(&mut form.bulk_amount).desired_width(70.0));
                let valid=xp::amount(&form.bulk_amount).is_ok_and(|amount|amount>0);
                if ui.add_enabled(valid,egui::Button::new(t(pt,"Apply to included present","Aplicar aos incluídos presentes"))).clicked() {
                    for row in form.rows.values_mut().filter(|r|r.included && r.present) {row.award_enabled=true;row.amount=form.bulk_amount.clone();}
                }
            });
            let row_ids: Vec<String> = form.rows.keys().cloned().collect();
            egui::ScrollArea::vertical()
                .id_salt("game_session_roster")
                .max_height(255.0)
                .show(ui, |ui| {
                    for record_id in row_ids {
                        let Some(summary) = summaries
                            .iter()
                            .find(|summary| summary.id == record_id && is_live_character(summary))
                        else {
                            continue;
                        };
                        let Some(row) = form.rows.get_mut(&record_id) else {
                            continue;
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.checkbox(&mut row.included, t(pt, "Include", "Incluir"));
                            ui.strong(&summary.name);
                            ui.small(format!("{} · {}", summary.kind, summary.player_name));
                            ui.checkbox(&mut row.present, t(pt, "Present", "Presente"));
                            if !row.included || !row.present {
                                row.award_enabled = false;
                            }
                            ui.add_enabled_ui(row.included && row.present, |ui| {
                                ui.checkbox(
                                    &mut row.award_enabled,
                                    t(pt, "Manual award", "Concessão manual"),
                                );
                                ui.add_enabled(
                                    row.award_enabled,
                                    egui::TextEdit::singleline(&mut row.amount)
                                        .desired_width(82.0)
                                        .hint_text(t(pt, "Amount", "Valor")),
                                );
                            });
                            if row.included
                                && row.present
                                && row.award_enabled
                                && !positive_amount(&row.amount)
                            {
                                ui.colored_label(
                                    egui::Color32::YELLOW,
                                    t(
                                        pt,
                                        "Enter a positive amount; zero is invalid.",
                                        "Informe um valor positivo; zero é inválido.",
                                    ),
                                );
                            }
                        });
                        ui.separator();
                    }
                });

            form.sync_request(summaries);
            if before_request != form.request || before_rows != form.rows {
                form.quote = None;
                form.dirty = true;
                form.message = None;
            }


            if let Some(message) = &form.message {
                ui.label(message);
            }
            if let Some(quote) = &form.quote {
                let current = quote.revision == revision && quote.request == form.request;
                ui.separator();
                ui.heading(t(pt, "Session preview", "Prévia da sessão"));
                if !current {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        t(
                            pt,
                            "Inputs or library revision changed; preview again before committing.",
                            "Os dados ou a revisão da biblioteca mudaram; atualize a prévia antes de registrar.",
                        ),
                    );
                }
                ui.label(format!("{} · {}", quote.request.title, quote.request.day));
                let included = quote.request.attendance.len();
                let present = quote.request.attendance.iter().filter(|row| row.present).count();
                ui.label(format!(
                    "{}: {included} · {}: {present}",
                    t(pt, "Included", "Incluídos"),
                    t(pt, "Present", "Presentes")
                ));

                let absent: Vec<&Attendance> = quote
                    .request
                    .attendance
                    .iter()
                    .filter(|row| !row.present)
                    .collect();
                ui.strong(t(pt, "Included but absent", "Incluídos, mas ausentes"));
                if absent.is_empty() {
                    ui.small(t(pt, "None", "Nenhum"));
                } else {
                    for row in absent {
                        ui.label(display_label(&row.record_id, summaries));
                    }
                }

                let excluded: Vec<String> = form
                    .rows
                    .iter()
                    .filter(|(_, row)| !row.included)
                    .map(|(record_id, _)| display_label(record_id, summaries))
                    .collect();
                ui.strong(t(pt, "Excluded from this session", "Excluídos desta sessão"));
                if excluded.is_empty() {
                    ui.small(t(pt, "None", "Nenhum"));
                } else {
                    for label in excluded {
                        ui.label(label);
                    }
                }

                ui.strong(t(pt, "Manual award changes", "Alterações de concessões manuais"));
                if quote.awards.is_empty() {
                    ui.small(t(pt, "No awards", "Sem concessões"));
                } else {
                    for award in &quote.awards {
                        ui.label(format!(
                            "{} · {} {} · {} {} → {} · {} {} → {}",
                            display_label(&award.record_id, summaries),
                            t(pt, "award", "concessão"),
                            award.command.amount,
                            t(pt, "earned", "ganho"),
                            award.before_earned,
                            award.after_earned,
                            t(pt, "unspent", "não gasto"),
                            award.before_unspent,
                            award.after_unspent,
                        ));
                    }
                }
            }

            ui.separator();
            ui.heading(t(pt, "Session receipt history", "Histórico de recibos de sessão"));
            if form.receipts.is_empty() {
                ui.small(t(pt, "No saved session receipts.", "Nenhum recibo de sessão salvo."));
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("game_session_history")
                    .max_height(170.0)
                    .show(ui, |ui| {
                        for (index, receipt) in form.receipts.iter().enumerate() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!(
                                    "{} · {} · {}: {}",
                                    receipt.quote.request.day,
                                    receipt.quote.request.title,
                                    t(pt, "revision", "revisão"),
                                    receipt.committed_revision
                                ));
                                if ui
                                    .button(t(pt, "Export receipt", "Exportar recibo"))
                                    .clicked()
                                {
                                    action = Some(Action::ExportReceipt(index));
                                }
                            });
                        }
                    });
            }
        });

    action
}
