use super::*;
use grapevine_workbench::chronicle::{Handover, Profile, PublicSnapshot};

pub(super) struct ChronicleWindow {
    pub open: bool,
    pub handover_tab: bool,
    pub edit: Profile,
    saved: Profile,
    revision: Option<i64>,
    public: Option<PublicSnapshot>,
    pub handover: Option<Handover>,
    contacts: bool,
    notes: bool,
    source_names: bool,
    document_language: Language,
    confirm_reload: bool,
    pub message: Option<String>,
}

impl Default for ChronicleWindow {
    fn default() -> Self {
        Self {
            open: false,
            handover_tab: false,
            edit: Profile::default(),
            saved: Profile::default(),
            revision: None,
            public: None,
            handover: None,
            contacts: false,
            notes: false,
            source_names: false,
            document_language: Language::English,
            confirm_reload: false,
            message: None,
        }
    }
}
impl ChronicleWindow {
    pub(super) fn synthetic(snapshot: grapevine_workbench::chronicle::Snapshot) -> Self {
        Self {
            open: true,
            saved: snapshot.profile.clone(),
            edit: snapshot.profile,
            revision: Some(snapshot.revision),
            document_language: Language::Portuguese,
            ..Default::default()
        }
    }
    pub(super) fn dirty(&self) -> bool {
        self.edit != self.saved
    }
    fn handover_current(&self, revision: Option<i64>) -> bool {
        !self.dirty()
            && self.handover.as_ref().is_some_and(|h| {
                Some(h.revision) == revision
                    && h.include_contacts == self.contacts
                    && h.include_notes == self.notes
                    && h.include_source_names == self.source_names
            })
    }
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
impl InconnuApp {
    pub(super) fn open_chronicle(&mut self) {
        if self.chronicle.revision.is_some() || self.chronicle.public.is_some() {
            self.chronicle.open = true;
        } else {
            self.reload_chronicle();
        }
    }
    fn reload_chronicle(&mut self) {
        let Some(lib) = &self.library else {
            return;
        };
        if self.route == WorkArea::Storyteller {
            match lib.chronicle_profile(Role::Storyteller) {
                Ok(s) => {
                    self.chronicle = ChronicleWindow {
                        open: true,
                        saved: s.profile.clone(),
                        edit: s.profile,
                        revision: Some(s.revision),
                        document_language: self.language,
                        ..Default::default()
                    }
                }
                Err(e) => self.log_error("chronicle.read", &e),
            }
        } else if self.route == WorkArea::Player {
            match lib.public_chronicle_profile() {
                Ok(s) => {
                    self.chronicle = ChronicleWindow {
                        open: true,
                        public: Some(s),
                        ..Default::default()
                    }
                }
                Err(e) => self.log_error("chronicle.public", &e),
            }
        }
    }
    pub(super) fn save_chronicle(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.chronicle.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(revision)) = (&mut self.library, self.chronicle.revision) else {
            return;
        };
        match lib.save_chronicle_profile(&self.chronicle.edit, revision, Role::Storyteller) {
            Ok(next) => {
                self.chronicle.edit.title = self.chronicle.edit.title.trim().into();
                self.chronicle.saved = self.chronicle.edit.clone();
                self.chronicle.revision = Some(next);
                self.chronicle.handover = None;
                self.revision = Some(next);
                self.chronicle.message = Some(
                    t(
                        self.language,
                        "Game information saved.",
                        "Informações do jogo salvas.",
                    )
                    .into(),
                );
                self.refresh_summaries(true);
            }
            Err(e) => self.chronicle.message = Some(format!("{}: {}", e.code, e.message)),
        }
    }
    pub(super) fn prepare_handover(&mut self) {
        if self.route != WorkArea::Storyteller || self.chronicle.dirty() {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        match lib.chronicle_handover(
            self.chronicle.contacts,
            self.chronicle.notes,
            self.chronicle.source_names,
            Role::Storyteller,
        ) {
            Ok(h) => {
                self.chronicle.handover = Some(h);
                self.chronicle.message = None;
            }
            Err(e) => {
                self.chronicle.handover = None;
                self.chronicle.message = Some(format!("{}: {}", e.code, e.message));
            }
        }
    }
    pub(super) fn render_chronicle(&mut self, ctx: &egui::Context) {
        if !self.chronicle.open || !matches!(self.route, WorkArea::Player | WorkArea::Storyteller) {
            return;
        }
        let l = self.language;
        let mut open = true;
        let mut legacy_review = false;
        let mut calendar_review = false;
        let (mut save, mut reload, mut prepare, mut export) = (false, false, false, false);
        let revision = self.library.as_ref().and_then(|lib| lib.revision().ok());
        egui::Window::new(t(l, "Game information", "Informações do jogo"))
            .id(egui::Id::unique("chronicle_window"))
            .default_size([720., 600.])
            .vscroll(true)
            .open(&mut open)
            .show(ctx, |ui| {
                if self.route == WorkArea::Storyteller
                    && ui
                        .button(t(
                            l,
                            "Review legacy game details…",
                            "Revisar detalhes legados do jogo…",
                        ))
                        .clicked()
                {
                    legacy_review = true;
                }
                if self.route == WorkArea::Storyteller
                    && ui
                        .button(t(
                            l,
                            "Review legacy calendar…",
                            "Revisar calendário legado…",
                        ))
                        .clicked()
                {
                    calendar_review = true;
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(t(
                            l,
                            "Reload saved information",
                            "Recarregar informações salvas",
                        ))
                        .clicked()
                    {
                        if self.chronicle.dirty() {
                            self.chronicle.confirm_reload = true;
                        } else {
                            reload = true;
                        }
                    }
                    if self.route == WorkArea::Storyteller {
                        let response = ui.add_enabled(
                            self.chronicle.dirty(),
                            egui::Button::new(t(
                                l,
                                "Save game information",
                                "Salvar informações do jogo",
                            )),
                        );
                        let visible = ui.is_rect_visible(response.rect).then_some(response.rect);
                        ctx.data_mut(|d| {
                            d.insert_temp(egui::Id::unique("chronicle_save"), visible)
                        });
                        save = response.clicked();
                    }
                });
                if self.chronicle.confirm_reload {
                    ui.label(t(
                        l,
                        "Discard this unsaved game profile?",
                        "Descartar este perfil de jogo não salvo?",
                    ));
                    ui.horizontal(|ui| {
                        if ui
                            .button(t(l, "Discard and reload", "Descartar e recarregar"))
                            .clicked()
                        {
                            reload = true;
                        }
                        if ui
                            .button(t(l, "Keep editing", "Continuar editando"))
                            .clicked()
                        {
                            self.chronicle.confirm_reload = false;
                        }
                    });
                }
                if self.route == WorkArea::Player {
                    if let Some(s) = &self.chronicle.public {
                        ui.heading(&s.profile.title);
                        for (label, value) in [
                            (t(l, "Website", "Site"), &s.profile.website),
                            (t(l, "Email", "E-mail"), &s.profile.email),
                            (t(l, "Phone", "Telefone"), &s.profile.phone),
                            (
                                t(l, "Meeting place", "Local do encontro"),
                                &s.profile.meeting_place,
                            ),
                            (
                                t(l, "Meeting time", "Horário do encontro"),
                                &s.profile.meeting_time,
                            ),
                            (t(l, "Description", "Descrição"), &s.profile.description),
                            (
                                t(l, "House rules", "Regras da casa"),
                                &s.profile.house_rules,
                            ),
                        ] {
                            if !value.is_empty() {
                                ui.strong(label);
                                ui.label(value);
                            }
                        }
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.chronicle.handover_tab,
                            false,
                            t(l, "Game profile", "Perfil do jogo"),
                        );
                        ui.selectable_value(
                            &mut self.chronicle.handover_tab,
                            true,
                            t(
                                l,
                                "Private staff handover",
                                "Passagem privada para a equipe",
                            ),
                        );
                    });
                    if !self.chronicle.handover_tab {
                        let p = &mut self.chronicle.edit;
                        for (label, value) in [
                            (t(l, "Chronicle title", "Título da crônica"), &mut p.title),
                            (t(l, "Website", "Site"), &mut p.website),
                            (t(l, "Email", "E-mail"), &mut p.email),
                            (t(l, "Phone", "Telefone"), &mut p.phone),
                            (
                                t(l, "Meeting place", "Local do encontro"),
                                &mut p.meeting_place,
                            ),
                            (
                                t(l, "Meeting time", "Horário do encontro"),
                                &mut p.meeting_time,
                            ),
                        ] {
                            ui.label(label);
                            ui.text_edit_singleline(value);
                        }
                        for (label, value) in [
                            (t(l, "Description", "Descrição"), &mut p.description),
                            (
                                t(
                                    l,
                                    "House rules entered by staff",
                                    "Regras da casa inseridas pela equipe",
                                ),
                                &mut p.house_rules,
                            ),
                            (
                                t(l, "Private staff notes", "Anotações privadas da equipe"),
                                &mut p.private_notes,
                            ),
                        ] {
                            ui.label(label);
                            ui.add(
                                egui::TextEdit::multiline(value)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(3),
                            );
                        }
                        ui.checkbox(
                            &mut p.owbn_bylaws_enabled,
                            t(l, "Enforce OWbN bylaws", "Aplicar os estatutos da OWbN"),
                        );
                        ui.add_enabled_ui(p.owbn_bylaws_enabled, |ui| {
                            ui.checkbox(
                                &mut p.allow_non_binding_sources,
                                t(
                                    l,
                                    "Allow non-binding sources",
                                    "Permitir fontes não vinculantes",
                                ),
                            );
                        });
                        if let Some(message) = house_rules_controls(ui, l, &mut p.house_rules_pack)
                        {
                            self.chronicle.message = Some(message);
                        }
                        ui.checkbox(
                            &mut p.publish_details,
                            t(
                                l,
                                "Show game details in Player view",
                                "Mostrar detalhes do jogo na visão do Jogador",
                            ),
                        );
                        ui.checkbox(
                            &mut p.publish_contacts,
                            t(
                                l,
                                "Show email and phone in Player view",
                                "Mostrar e-mail e telefone na visão do Jogador",
                            ),
                        );
                    } else {
                        egui::CollapsingHeader::new(t(
                            l,
                            "Private staff handover",
                            "Passagem privada para a equipe",
                        ))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.checkbox(
                                &mut self.chronicle.contacts,
                                t(l, "Include email and phone", "Incluir e-mail e telefone"),
                            );
                            ui.checkbox(
                                &mut self.chronicle.notes,
                                t(
                                    l,
                                    "Include private staff notes",
                                    "Incluir anotações privadas da equipe",
                                ),
                            );
                            ui.checkbox(
                                &mut self.chronicle.source_names,
                                t(
                                    l,
                                    "Include source filenames",
                                    "Incluir nomes dos arquivos de origem",
                                ),
                            );
                            ui.horizontal_wrapped(|ui| {
                                ui.selectable_value(
                                    &mut self.chronicle.document_language,
                                    Language::English,
                                    "English",
                                );
                                ui.selectable_value(
                                    &mut self.chronicle.document_language,
                                    Language::Portuguese,
                                    "Português",
                                );
                                let response = ui.add_enabled(
                                    !self.chronicle.dirty(),
                                    egui::Button::new(t(
                                        l,
                                        "Prepare handover",
                                        "Preparar passagem",
                                    )),
                                );
                                let visible =
                                    ui.is_rect_visible(response.rect).then_some(response.rect);
                                ctx.data_mut(|d| {
                                    d.insert_temp(egui::Id::unique("chronicle_prepare"), visible)
                                });
                                prepare = response.clicked();
                                export = ui
                                    .add_enabled(
                                        self.chronicle.handover_current(revision),
                                        egui::Button::new(t(
                                            l,
                                            "Save private report…",
                                            "Salvar relatório privado…",
                                        )),
                                    )
                                    .clicked();
                            });
                            if self.chronicle.handover_current(revision)
                                && let Some(h) = &self.chronicle.handover
                            {
                                ui.label(format!(
                                    "{} {}",
                                    t(l, "Prepared at revision", "Preparado na revisão"),
                                    h.revision
                                ));
                            }
                        });
                    }
                }
                if let Some(message) = &self.chronicle.message {
                    ui.label(message);
                }
            });
        self.chronicle.open = open;
        if reload {
            self.reload_chronicle();
        }
        if save {
            self.save_chronicle();
        }
        if prepare {
            self.prepare_handover();
        }
        if export
            && self.chronicle.handover_current(revision)
            && let Some(h) = &self.chronicle.handover
        {
            match grapevine_workbench::chronicle_handover::html(h, self.chronicle.document_language)
            {
                Ok(html) => {
                    if let Some(path) = FileDialog::new()
                        .set_file_name("private-chronicle-handover.html")
                        .save_file()
                    {
                        match grapevine_workbench::safe_files::write_new(&path, html.as_bytes()) {
                            Ok(()) => {
                                self.chronicle.message = Some(
                                    t(l, "Private report saved.", "Relatório privado salvo.")
                                        .into(),
                                )
                            }
                            Err(e) => {
                                self.chronicle.message = Some(format!("{}: {}", e.code, e.message))
                            }
                        }
                    }
                }
                Err(e) => self.chronicle.message = Some(format!("{}: {}", e.code, e.message)),
            }
        }
        if legacy_review {
            self.open_legacy_profile();
        }
        if calendar_review {
            self.open_legacy_calendar();
        }
    }
}

fn house_rules_controls(
    ui: &mut egui::Ui,
    l: Language,
    pack: &mut Option<grapevine_workbench::house_rules::HouseRulesPack>,
) -> Option<String> {
    use grapevine_workbench::house_rules::HouseRulesPack;
    let mut outcome = None;
    ui.collapsing(t(l, "House rules pack", "Pacote de regras da casa"), |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button(t(l, "Load JSON…", "Carregar JSON…")).clicked()
                && let Some(path) = FileDialog::new().add_filter("JSON", &["json"]).pick_file()
            {
                match HouseRulesPack::load(&path) {
                    Ok(loaded) => {
                        *pack = Some(loaded);
                        outcome = Some(
                            t(
                                l,
                                "Pack loaded into the Chronicle draft.",
                                "Pacote carregado no rascunho da crônica.",
                            )
                            .into(),
                        );
                    }
                    Err(error) => outcome = Some(error.message),
                }
            }
            if pack.is_none() && ui.button(t(l, "Create pack", "Criar pacote")).clicked() {
                *pack = Some(HouseRulesPack::default());
            }
            if let Some(active) = pack.as_ref() {
                if ui.button(t(l, "Save JSON…", "Salvar JSON…")).clicked()
                    && let Some(path) = FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .set_file_name("house-rules.json")
                        .save_file()
                {
                    outcome = Some(match active.save_new(&path) {
                        Ok(()) => t(l, "Pack exported.", "Pacote exportado.").into(),
                        Err(error) => error.message,
                    });
                }
                if ui.button(t(l, "Clear pack", "Limpar pacote")).clicked() {
                    *pack = None;
                }
            }
        });
        if let Some(active) = pack {
            ui.label(t(l, "Pack name", "Nome do pacote"));
            ui.text_edit_singleline(&mut active.name);
            ui.label(t(l, "Pack version", "Versão do pacote"));
            ui.text_edit_singleline(&mut active.version);
            let formula = &mut active.challenge_formula;
            ui.checkbox(
                &mut formula.include_ability_in_trait_call,
                t(
                    l,
                    "Count Ability Traits in calls",
                    "Contar características de Habilidade nas comparações",
                ),
            );
            ui.checkbox(
                &mut formula.include_linked_item_bonuses,
                t(
                    l,
                    "Apply linked item bonuses",
                    "Aplicar bônus dos itens vinculados",
                ),
            );
            ui.checkbox(
                &mut formula.include_merit_flaw_modifiers,
                t(
                    l,
                    "Apply Merit and Flaw modifiers",
                    "Aplicar modificadores de Qualidades e Defeitos",
                ),
            );
            let mut capped = formula.max_trait_bid_cap.is_some();
            if ui
                .checkbox(
                    &mut capped,
                    t(
                        l,
                        "Set a local Trait bid cap",
                        "Definir um limite local de características",
                    ),
                )
                .changed()
            {
                formula.max_trait_bid_cap = capped.then_some(30);
            }
            if let Some(cap) = &mut formula.max_trait_bid_cap {
                ui.add(egui::DragValue::new(cap).range(1..=4096));
            }
        }
    });
    outcome
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn profile_draft_stale_save_and_handover_scope_survive_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chronicle-ui.gvlib");
        drop(Library::create(&path, "Original").unwrap());
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.open_chronicle();
        app.chronicle.edit.email = "PRIVATE_CONTACT".into();
        app.chronicle.edit.private_notes = "PRIVATE_NOTES".into();
        app.chronicle.open = false;
        app.language = Language::Portuguese;
        app.open_chronicle();
        assert!(app.has_unsaved_edit());
        assert_eq!(app.chronicle.edit.email, "PRIVATE_CONTACT");
        let mut other = Library::open(&path).unwrap();
        other
            .create_player("Other writer", 0, Role::Storyteller)
            .unwrap();
        app.save_chronicle();
        assert!(app.chronicle.dirty());
        assert!(
            app.chronicle
                .message
                .as_ref()
                .unwrap()
                .contains("STALE_REVISION")
        );
        app.reload_chronicle();
        app.chronicle.edit.email = "PRIVATE_CONTACT".into();
        app.chronicle.edit.private_notes = "PRIVATE_NOTES".into();
        app.save_chronicle();
        assert!(!app.chronicle.dirty());
        let revision = app.library.as_ref().unwrap().revision().unwrap();
        app.prepare_handover();
        assert_eq!(app.library.as_ref().unwrap().revision().unwrap(), revision);
        assert!(app.chronicle.handover_current(Some(revision)));
        app.chronicle.notes = true;
        assert!(!app.chronicle.handover_current(Some(revision)));
        app.prepare_handover();
        assert_eq!(
            app.chronicle
                .handover
                .as_ref()
                .unwrap()
                .profile
                .private_notes,
            "PRIVATE_NOTES"
        );
        app.change_route(WorkArea::Player);
        assert!(app.chronicle.edit.private_notes.is_empty());
        assert!(app.chronicle.handover.is_none());
        app.open_chronicle();
        let public = serde_json::to_string(app.chronicle.public.as_ref().unwrap()).unwrap();
        assert!(!public.contains("PRIVATE_CONTACT") && !public.contains("PRIVATE_NOTES"));
    }
}
