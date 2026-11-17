use super::*;
use grapevine_workbench::operations::{self, Content, Request, Snapshot};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Kind {
    #[default]
    Roster,
    SignIn,
    Calendar,
    CharacterHistory,
    PlayerHistory,
    PlayerRoster,
    VampireStatus,
}
struct Prepared {
    request: Request,
    snapshot: Snapshot,
}
#[derive(Default)]
pub(super) struct OperationsWindow {
    pub open: bool,
    kind: Kind,
    family: String,
    status: String,
    include_players: bool,
    include_reasons: bool,
    include_contacts: bool,
    include_boons: bool,
    include_descriptions: bool,
    include_legacy_ids: bool,
    character_history_id: String,
    player_history_id: String,
    from: String,
    through: String,
    include_cancelled: bool,
    schedule_id: String,
    plans: Vec<(String, String)>,
    document_language: Option<Language>,
    prepared: Option<Prepared>,
    pub message: Option<String>,
}
impl OperationsWindow {
    fn request(&self) -> Request {
        match self.kind {
            Kind::VampireStatus => Request::VampireStatus {
                status: self.status.trim().into(),
                include_boons: self.include_boons,
                include_descriptions: self.include_descriptions,
            },
            Kind::PlayerRoster => Request::PlayerRoster {
                status: self.status.clone(),
                include_contacts: self.include_contacts,
                include_legacy_ids: self.include_legacy_ids,
            },
            Kind::CharacterHistory => Request::CharacterHistory {
                record_id: self.character_history_id.clone(),
                from: self.from.clone(),
                through: self.through.clone(),
                include_reasons: self.include_reasons,
            },
            Kind::PlayerHistory => Request::PlayerHistory {
                record_id: self.player_history_id.clone(),
                from: self.from.clone(),
                through: self.through.clone(),
                include_reasons: self.include_reasons,
            },
            Kind::Roster => Request::Roster {
                family: self.family.clone(),
                status: self.status.trim().to_owned(),
                include_players: self.include_players,
            },
            Kind::SignIn => Request::SignIn {
                schedule_id: self.schedule_id.clone(),
                include_players: self.include_players,
            },
            Kind::Calendar => Request::Calendar {
                from: self.from.trim().to_owned(),
                through: self.through.trim().to_owned(),
                include_cancelled: self.include_cancelled,
            },
        }
    }
    pub(super) fn synthetic_status() -> Self {
        Self {
            kind: Kind::VampireStatus,
            include_boons: true,
            ..Self::synthetic()
        }
    }
    pub(super) fn synthetic_history(id: String, player: bool) -> Self {
        let mut window = Self::synthetic();
        window.kind = if player {
            Kind::PlayerHistory
        } else {
            Kind::CharacterHistory
        };
        if player {
            window.player_history_id = id;
        } else {
            window.character_history_id = id;
        }
        window.from = "2026-01-01".into();
        window.through = "2026-12-31".into();
        window
    }
    pub(super) fn is_prepared(&self) -> bool {
        self.prepared.is_some()
    }
    pub(super) fn synthetic() -> Self {
        Self {
            open: true,
            document_language: Some(Language::Portuguese),
            ..Default::default()
        }
    }
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}

impl InconnuApp {
    pub(super) fn open_operations(&mut self) {
        if self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        self.operations.open = true;
        self.refresh_report_plans();
    }

    fn refresh_report_plans(&mut self) {
        let Some(lib) = &self.library else {
            return;
        };
        match lib.scheduled_sessions(Role::Storyteller) {
            Ok(rows) => {
                self.operations.plans = rows
                    .into_iter()
                    .filter(|p| !p.cancelled)
                    .map(|p| {
                        (
                            p.request.id,
                            format!("{} · {}", p.request.day, p.request.title),
                        )
                    })
                    .collect();
                if !self
                    .operations
                    .plans
                    .iter()
                    .any(|(id, _)| *id == self.operations.schedule_id)
                {
                    self.operations.schedule_id.clear();
                }
            }
            Err(e) => self.operations.message = Some(e.message),
        }
    }

    fn prepare_operations(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let request = self.operations.request();
        match lib.operations_snapshot(&request, Role::Storyteller) {
            Ok(snapshot) => {
                self.operations.prepared = Some(Prepared { request, snapshot });
                self.operations.message = None;
            }
            Err(e) => {
                self.operations.prepared = None;
                self.operations.message = Some(e.message);
            }
        }
    }

    fn operations_document(&self) -> grapevine_workbench::model::Result<String> {
        let stale = || {
            grapevine_workbench::model::error(
                "REPORT_NOT_READY",
                "Prepare the current report again before saving.",
            )
        };
        if self.route != WorkArea::Storyteller {
            return Err(stale());
        }
        let lib = self.library.as_ref().ok_or_else(stale)?;
        let prepared = self.operations.prepared.as_ref().ok_or_else(stale)?;
        if prepared.request != self.operations.request()
            || prepared.snapshot.revision != lib.revision()?
        {
            return Err(stale());
        }
        operations::html(
            &prepared.snapshot,
            self.operations.document_language.unwrap_or(self.language),
        )
    }

    pub(super) fn render_operations(&mut self, ctx: &egui::Context) {
        if !self.operations.open || self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        let l = self.language;
        let mut open = true;
        let mut prepare = false;
        let mut export = false;
        let mut refresh = false;
        let mut families: Vec<_> = self
            .summaries
            .iter()
            .filter(|s| s.build.is_none() && grapevine_workbench::sheet::is_character_kind(&s.kind))
            .map(|s| s.kind.as_str())
            .collect();
        families.sort_unstable();
        families.dedup();
        egui::Window::new(t(l, "Operational reports", "Relatórios operacionais"))
            .id(egui::Id::unique("operations_window"))
            .default_size([680., 420.])
            .vscroll(true)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.operations.kind,
                        Kind::Roster,
                        t(l, "Character roster", "Lista de personagens"),
                    );
                    ui.selectable_value(
                        &mut self.operations.kind,
                        Kind::SignIn,
                        t(l, "Blank sign-in", "Presença em branco"),
                    );
                    ui.selectable_value(
                        &mut self.operations.kind,
                        Kind::Calendar,
                        t(l, "Session calendar", "Calendário de sessões"),
                    );
                });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.operations.kind,
                        Kind::CharacterHistory,
                        t(l, "Experience history", "Histórico de XP"),
                    );
                    ui.selectable_value(
                        &mut self.operations.kind,
                        Kind::PlayerHistory,
                        t(l, "Player Point history", "Histórico de pontos do Jogador"),
                    );
                    ui.selectable_value(
                        &mut self.operations.kind,
                        Kind::PlayerRoster,
                        t(l, "Player roster", "Lista de jogadores"),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.operations.kind,
                        Kind::VampireStatus,
                        t(l, "Vampire Status", "Status de Vampiros"),
                    );
                });
                match self.operations.kind {
                    Kind::VampireStatus => {
                        ui.horizontal(|ui| {
                            ui.label(t(
                                l,
                                "Exact saved status (optional)",
                                "Situação salva exata (opcional)",
                            ));
                            ui.text_edit_singleline(&mut self.operations.status);
                        });
                        ui.checkbox(
                            &mut self.operations.include_boons,
                            t(l, "Include Boons", "Incluir favores"),
                        );
                        ui.add_enabled(
                            self.operations.include_boons,
                            egui::Checkbox::new(
                                &mut self.operations.include_descriptions,
                                t(
                                    l,
                                    "Include private Boon descriptions",
                                    "Incluir descrições privadas dos favores",
                                ),
                            ),
                        );
                    }
                    Kind::PlayerRoster => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(t(l, "Exact status (optional)", "Situação exata (opcional)"));
                            ui.text_edit_singleline(&mut self.operations.status);
                        });
                        ui.checkbox(
                            &mut self.operations.include_contacts,
                            t(l, "Include email and phone", "Incluir e-mail e telefone"),
                        );
                        ui.checkbox(
                            &mut self.operations.include_legacy_ids,
                            t(
                                l,
                                "Include legacy player identifiers",
                                "Incluir identificadores legados",
                            ),
                        );
                    }
                    Kind::CharacterHistory | Kind::PlayerHistory => {
                        let player = self.operations.kind == Kind::PlayerHistory;
                        let selected = if player {
                            &mut self.operations.player_history_id
                        } else {
                            &mut self.operations.character_history_id
                        };
                        let eligible = |r: &&Summary| {
                            r.build.is_none()
                                && if player {
                                    r.kind == "Player"
                                } else {
                                    grapevine_workbench::sheet::is_character_kind(&r.kind)
                                }
                        };
                        let choices: Vec<_> = self.summaries.iter().filter(eligible).collect();
                        let title = choices
                            .iter()
                            .find(|r| r.id == *selected)
                            .map_or(t(l, "Choose a record", "Escolher registro"), |r| {
                                r.name.as_str()
                            });
                        egui::ComboBox::from_id_salt("history_subject")
                            .selected_text(title)
                            .show_ui(ui, |ui| {
                                for r in &choices {
                                    let duplicate =
                                        choices.iter().filter(|other| other.name == r.name).count()
                                            > 1;
                                    let label = if duplicate {
                                        format!(
                                            "{} · {}",
                                            r.name,
                                            r.id.chars().take(8).collect::<String>()
                                        )
                                    } else {
                                        r.name.clone()
                                    };
                                    ui.selectable_value(selected, r.id.clone(), label);
                                }
                            });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(t(l, "From (YYYY-MM-DD)", "De (AAAA-MM-DD)"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.operations.from)
                                    .desired_width(110.),
                            );
                            ui.label(t(l, "Through", "Até"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.operations.through)
                                    .desired_width(110.),
                            );
                        });
                        ui.checkbox(
                            &mut self.operations.include_reasons,
                            t(l, "Include private reasons", "Incluir motivos privados"),
                        );
                        ui.label(t(
                            l,
                            "Current balances and dated entries are shown separately.",
                            "Saldos atuais e entradas datadas aparecem separadamente.",
                        ));
                    }
                    Kind::Roster => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(t(l, "Family", "Família"));
                            let family_label = if self.operations.family.is_empty() {
                                t(l, "All", "Todas")
                            } else {
                                &self.operations.family
                            };
                            egui::ComboBox::from_id_salt("operations_family")
                                .selected_text(family_label)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.operations.family,
                                        String::new(),
                                        t(l, "All", "Todas"),
                                    );
                                    for family in families {
                                        ui.selectable_value(
                                            &mut self.operations.family,
                                            family.into(),
                                            family,
                                        );
                                    }
                                });
                            ui.label(t(l, "Exact status (optional)", "Situação exata (opcional)"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.operations.status)
                                    .desired_width(135.),
                            );
                        });
                    }
                    Kind::SignIn => {
                        ui.horizontal_wrapped(|ui| {
                            let plan_label = self
                                .operations
                                .plans
                                .iter()
                                .find(|(id, _)| *id == self.operations.schedule_id)
                                .map_or(
                                    t(l, "Choose a saved plan", "Escolher plano salvo"),
                                    |(_, name)| name,
                                );
                            egui::ComboBox::from_id_salt("operations_plan")
                                .selected_text(plan_label)
                                .show_ui(ui, |ui| {
                                    for (id, title) in &self.operations.plans {
                                        ui.selectable_value(
                                            &mut self.operations.schedule_id,
                                            id.clone(),
                                            title,
                                        );
                                    }
                                });
                            refresh = ui
                                .button(t(l, "Refresh plans", "Atualizar planos"))
                                .clicked();
                        });
                        ui.label(t(
                            l,
                            "Uses the saved attendee selection. The attendance column is blank.",
                            "Usa os participantes salvos. A coluna de presença fica em branco.",
                        ));
                    }
                    Kind::Calendar => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(t(l, "From (YYYY-MM-DD)", "De (AAAA-MM-DD)"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.operations.from)
                                    .desired_width(105.),
                            );
                            ui.label(t(l, "Through", "Até"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.operations.through)
                                    .desired_width(105.),
                            );
                        });
                        ui.checkbox(
                            &mut self.operations.include_cancelled,
                            t(l, "Include cancelled plans", "Incluir planos cancelados"),
                        );
                    }
                }
                if matches!(self.operations.kind, Kind::Roster | Kind::SignIn) {
                    ui.checkbox(
                        &mut self.operations.include_players,
                        t(l, "Include player names", "Incluir nomes de jogadores"),
                    );
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    let lang = self.operations.document_language.get_or_insert(l);
                    ui.selectable_value(lang, Language::English, "English");
                    ui.selectable_value(lang, Language::Portuguese, "Português");
                    let b = ui.button(t(l, "Prepare report", "Preparar relatório"));
                    set_probe(ui, &b, "operations_prepare");
                    prepare = b.clicked();
                    let ready = self
                        .operations
                        .prepared
                        .as_ref()
                        .is_some_and(|p| p.request == self.operations.request());
                    let b = ui.add_enabled(
                        ready,
                        egui::Button::new(t(l, "Save private HTML…", "Salvar HTML privado…")),
                    );
                    set_probe(ui, &b, "operations_export");
                    export = b.clicked();
                });
                if let Some(p) = &self.operations.prepared {
                    if p.request == self.operations.request() {
                        let count = match &p.snapshot.content {
                            Content::VampireStatus(s) => s.characters.len(),
                            Content::PlayerRoster { rows, .. } => rows.len(),
                            Content::History(h) => {
                                grapevine_workbench::ledger_reports::row_count(h)
                            }
                            Content::Roster { rows, .. } => rows.len(),
                            Content::SignIn { rows, .. } => rows.len(),
                            Content::Calendar { rows, .. } => rows.len(),
                        };
                        ui.label(format!(
                            "{} {count} · {} {}",
                            t(l, "Rows prepared:", "Linhas preparadas:"),
                            t(l, "Library revision", "Revisão da biblioteca"),
                            p.snapshot.revision
                        ));
                    } else {
                        ui.label(t(
                            l,
                            "Filters changed. Prepare the report again.",
                            "Os filtros mudaram. Prepare o relatório novamente.",
                        ));
                    }
                }
                if let Some(message) = &self.operations.message {
                    ui.label(message);
                }
            });
        self.operations.open = open;
        if refresh {
            self.refresh_report_plans();
        }
        if prepare {
            self.prepare_operations();
        }
        if export {
            match self.operations_document() {
                Ok(html) => {
                    if let Some(path) = FileDialog::new()
                        .set_file_name("Inconnu-private-report.html")
                        .add_filter("HTML", &["html"])
                        .save_file()
                    {
                        self.operations.message = Some(
                            match grapevine_workbench::safe_files::write_new(&path, html.as_bytes())
                            {
                                Ok(()) => t(
                                    l,
                                    "Report saved for browser print/PDF.",
                                    "Relatório salvo para impressão/PDF no navegador.",
                                )
                                .into(),
                                Err(e) => e.message,
                            },
                        );
                    }
                }
                Err(e) => self.operations.message = Some(e.message),
            }
        }
    }
}

fn set_probe(ui: &egui::Ui, b: &egui::Response, key: &'static str) {
    let rect = ui.clip_rect().contains_rect(b.rect).then_some(b.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), rect));
}

#[cfg(test)]
mod history_lifecycle_tests {
    use super::*;
    #[test]
    fn vampire_status_opt_ins_revision_and_player_route_guard_the_prepared_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let holder = lib
            .create_character("Vampire", "Holder", 0, Role::Storyteller)
            .unwrap();
        lib.add_boon(
            &holder,
            &grapevine_workbench::boon_book::Edit {
                partner: "Partner".into(),
                kind: "Minor".into(),
                owed_by_holder: Some(true),
                day: "2026-01-01".into(),
                description: "PRIVATE_STATUS_DESCRIPTION".into(),
            },
            1,
            Role::Storyteller,
        )
        .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.operations = OperationsWindow::synthetic_status();
        app.prepare_operations();
        assert!(app.operations_document().is_ok());
        assert!(
            !app.operations_document()
                .unwrap()
                .contains("PRIVATE_STATUS_DESCRIPTION")
        );
        app.operations.include_descriptions = true;
        assert!(app.operations_document().is_err());
        app.prepare_operations();
        assert!(
            app.operations_document()
                .unwrap()
                .contains("PRIVATE_STATUS_DESCRIPTION")
        );
        app.operations.open = false;
        app.language = Language::Portuguese;
        app.open_operations();
        assert!(app.operations.include_descriptions);
        let lib = app.library.as_mut().unwrap();
        lib.create_player("Concurrent", lib.revision().unwrap(), Role::Storyteller)
            .unwrap();
        assert!(app.operations_document().is_err());
        app.change_route(WorkArea::Player);
        assert!(!app.operations.open && !app.operations.is_prepared());
        assert!(!app.operations.include_descriptions);
    }
    #[test]
    fn history_scope_changes_and_library_writes_require_fresh_preparation_and_player_clears_state()
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let id = lib.create_player("Subject", 0, Role::Storyteller).unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.operations = OperationsWindow::synthetic_history(id.clone(), true);
        app.prepare_operations();
        assert!(app.operations_document().is_ok());
        app.operations.include_reasons = true;
        assert!(app.operations_document().is_err());
        app.prepare_operations();
        assert!(app.operations_document().is_ok());
        app.operations.open = false;
        app.language = Language::Portuguese;
        app.open_operations();
        assert_eq!(app.operations.player_history_id, id);
        assert!(app.operations.include_reasons);
        let lib = app.library.as_mut().unwrap();
        lib.create_player("Another writer", lib.revision().unwrap(), Role::Storyteller)
            .unwrap();
        assert!(app.operations_document().is_err());
        app.change_route(WorkArea::Player);
        assert!(!app.operations.open && !app.operations.is_prepared());
        assert!(
            app.operations.player_history_id.is_empty()
                && app.operations.character_history_id.is_empty()
        );
        assert!(app.operations_document().is_err());
    }
}
