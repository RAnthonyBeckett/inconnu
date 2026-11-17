use super::*;
use grapevine_workbench::{
    actions::Action,
    story_work::{Draft, Kind},
};

fn t(language: Language, english: &'static str, portuguese: &'static str) -> &'static str {
    if language == Language::Portuguese {
        portuguese
    } else {
        english
    }
}
impl InconnuApp {
    fn timeline_ready(&self) -> bool {
        self.route == WorkArea::Storyteller
            && if self.story.include_rumors {
                self.story.combined.as_ref().is_some_and(|s| {
                    self.story.character_id.as_deref() == Some(s.request.character_id.as_str())
                        && self.story.from.trim() == s.request.from
                        && self.story.through.trim() == s.request.through
                        && self.library.as_ref().and_then(|l| l.revision().ok())
                            == Some(s.actions.revision)
                })
            } else {
                self.story.combined.is_none()
            }
            && self.story.timeline.as_ref().is_some_and(|timeline| {
                self.story.character_id.as_deref() == Some(timeline.character_id.as_str())
                    && self.story.from.trim() == timeline.from
                    && self.story.through.trim() == timeline.through
                    && self.library.as_ref().and_then(|lib| lib.revision().ok())
                        == Some(timeline.revision)
            })
    }
    pub(super) fn prepare_action_timeline(&mut self) {
        if self.route != WorkArea::Storyteller || self.story.kind() != Kind::Action {
            return;
        }
        let (Some(lib), Some(character)) = (&self.library, &self.story.character_id) else {
            return;
        };
        let result = if self.story.include_rumors {
            lib.combined_story_snapshot(
                &grapevine_workbench::combined_story::Request {
                    character_id: character.clone(),
                    from: self.story.from.trim().into(),
                    through: self.story.through.trim().into(),
                },
                Role::Storyteller,
            )
            .map(|snapshot| (snapshot.actions.clone(), Some(snapshot)))
        } else {
            lib.action_timeline(
                character,
                self.story.from.trim(),
                self.story.through.trim(),
                Role::Storyteller,
            )
            .map(|timeline| (timeline, None))
        };
        match result {
            Ok((timeline, combined)) => {
                self.story.timeline = Some(timeline);
                self.story.combined = combined;
                self.story.message = None;
            }
            Err(error) => {
                self.story.timeline = None;
                self.story.combined = None;
                self.story.message = Some(error.message);
            }
        }
    }
    pub(super) fn prepare_action_copy(&mut self) {
        if self.route != WorkArea::Storyteller
            || self.story.target.is_some()
            || !self.timeline_ready()
        {
            return;
        }
        let (Some(lib), Some(character), Draft::Action(action)) =
            (&self.library, &self.story.character_id, &self.story.draft)
        else {
            return;
        };
        match lib.preview_action_copy(
            character,
            &self.story.prior_action_id,
            &action.day,
            &self.story.copy_options,
            Role::Storyteller,
        ) {
            Ok(preview) => {
                self.story.copy_preview = Some(preview);
                self.story.confirm_copy = false;
                self.story.message = None;
            }
            Err(error) => {
                self.story.copy_preview = None;
                self.story.message = Some(error.message);
            }
        }
    }
    fn action_copy_ready(&self) -> bool {
        if self.story.target.is_some() || !self.timeline_ready() {
            return false;
        }
        let Draft::Action(action) = &self.story.draft else {
            return false;
        };
        self.story.copy_preview.as_ref().is_some_and(|preview| {
            self.story.character_id.as_deref() == Some(preview.character_id.as_str())
                && self.story.prior_action_id == preview.source_id
                && action.day == preview.day
                && self.story.copy_options == preview.options
                && self.library.as_ref().and_then(|lib| lib.revision().ok())
                    == Some(preview.revision)
        })
    }
    pub(super) fn apply_action_copy(&mut self) {
        if !self.action_copy_ready() {
            self.story.message = Some(
                t(
                    self.language,
                    "The request or library changed. Prepare the preview again.",
                    "A solicitação ou biblioteca mudou. Prepare a prévia novamente.",
                )
                .into(),
            );
            return;
        }
        let preview = self.story.copy_preview.take().unwrap();
        self.story.draft = Draft::Action(preview.draft);
        self.story.character_id = Some(preview.character_id);
        self.story.revision = Some(preview.revision);
        self.story.binding_missing = false;
        self.story.confirm_copy = false;
        self.story.message = Some(
            t(
                self.language,
                "Preview applied to the new draft. Review and save when ready.",
                "Prévia aplicada ao novo rascunho. Revise e salve quando estiver pronto.",
            )
            .into(),
        );
    }
    pub(super) fn action_timeline_document(&self) -> grapevine_workbench::model::Result<String> {
        if !self.timeline_ready() {
            return Err(grapevine_workbench::model::error(
                "ACTION_TIMELINE_REFRESH",
                "Prepare a current character timeline first.",
            ));
        }
        if self.story.include_rumors {
            return grapevine_workbench::combined_story_report::html(
                self.story.combined.as_ref().unwrap(),
                self.story.document_language.unwrap_or(self.language),
            );
        }
        grapevine_workbench::action_timeline_report::html(
            self.story.timeline.as_ref().unwrap(),
            self.story.document_language.unwrap_or(self.language),
        )
    }
    pub(super) fn render_action_context(&mut self, ui: &mut egui::Ui) {
        let language = self.language;
        let mut changed = false;
        let choices: Vec<_> = self
            .summaries
            .iter()
            .filter(|row| {
                row.build.is_none() && grapevine_workbench::sheet::is_character_kind(&row.kind)
            })
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(t(language, "Linked character", "Personagem vinculado"));
            let title = self
                .story
                .character_id
                .as_ref()
                .and_then(|id| choices.iter().find(|row| &row.id == id))
                .map_or(t(language, "Unlinked label", "Nome sem vínculo"), |row| {
                    row.name.as_str()
                });
            egui::ComboBox::from_id_salt("action_character_link")
                .selected_text(title)
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(
                            &mut self.story.character_id,
                            None,
                            t(language, "Unlinked label", "Nome sem vínculo"),
                        )
                        .changed();
                    for row in &choices {
                        let duplicate = choices
                            .iter()
                            .filter(|other| other.name == row.name && other.kind == row.kind)
                            .count()
                            > 1;
                        let mut label = format!("{} · {}", row.name, row.kind);
                        if duplicate {
                            label.push_str(&format!(
                                " · {}",
                                row.id.chars().take(8).collect::<String>()
                            ));
                        }
                        changed |= ui
                            .selectable_value(
                                &mut self.story.character_id,
                                Some(row.id.clone()),
                                label,
                            )
                            .changed();
                    }
                });
        });
        if changed {
            if let Some(row) = choices
                .iter()
                .find(|row| Some(&row.id) == self.story.character_id.as_ref())
                && let Draft::Action(action) = &mut self.story.draft
            {
                action.character = row.name.clone();
            }
            self.story.binding_missing = false;
            self.story.timeline = None;
            self.story.combined = None;
            self.story.copy_preview = None;
            self.story.prior_action_id.clear();
        }
        if self.story.binding_missing {
            ui.label(t(
                language,
                "The saved character link requires review. Its recorded label is retained.",
                "O vínculo salvo do personagem requer revisão. O nome registrado foi preservado.",
            ));
        }
        let linked = self.story.character_id.is_some();
        if let Draft::Action(action) = &mut self.story.draft {
            ui.horizontal_wrapped(|ui| {
                ui.label(t(language, "Character label", "Nome do personagem"));
                ui.add_enabled(
                    !linked,
                    egui::TextEdit::singleline(&mut action.character).desired_width(220.),
                );
                ui.label(t(language, "Date (YYYY-MM-DD)", "Data (AAAA-MM-DD)"));
                ui.add(egui::TextEdit::singleline(&mut action.day).desired_width(105.));
            });
        }
        if !linked {
            return;
        }
        let mut prepare = false;
        let mut preview = false;
        let mut apply = false;
        let mut export = false;
        let timeline_ready = self.timeline_ready();
        let copy_ready = self.action_copy_ready();
        let panel_title = t(
            language,
            "Character timeline and previous values",
            "Linha do tempo do personagem e valores anteriores",
        );
        egui::CollapsingHeader::new(panel_title)
            .id_salt("action_character_context")
            .default_open(true)
            .show(ui, |ui| {
                let option = ui.checkbox(
                    &mut self.story.include_rumors,
                    t(
                        language,
                        "Include staff-selected Rumors",
                        "Incluir rumores selecionados pela equipe",
                    ),
                );
                set_probe(ui, &option, "combined_story_option");
                if option.changed() {
                    self.story.timeline = None;
                    self.story.combined = None;
                    self.story.copy_preview = None;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(t(language, "From", "De"));
                    ui.add(egui::TextEdit::singleline(&mut self.story.from).desired_width(105.));
                    ui.label(t(language, "Through", "Até"));
                    ui.add(egui::TextEdit::singleline(&mut self.story.through).desired_width(105.));
                    let button =
                        ui.button(t(language, "Prepare timeline", "Preparar linha do tempo"));
                    set_probe(ui, &button, "action_timeline_prepare");
                    prepare = button.clicked();
                    export = ui
                        .add_enabled(
                            timeline_ready,
                            egui::Button::new(t(
                                language,
                                "Save private timeline…",
                                "Salvar linha do tempo privada…",
                            )),
                        )
                        .clicked();
                });
                if let Some(timeline) = &self.story.timeline {
                    ui.label(format!(
                        "{} {} · {} {}",
                        t(language, "Linked Actions", "Ações vinculadas"),
                        timeline.rows.len(),
                        t(
                            language,
                            "Unlinked library Actions",
                            "Ações sem vínculo na biblioteca"
                        ),
                        timeline.unlinked_in_library
                    ));
                }
                if let Some(combined) = &self.story.combined {
                    ui.label(format!(
                        "{} {}",
                        t(
                            language,
                            "Staff-selected Rumors",
                            "Rumores selecionados pela equipe"
                        ),
                        combined.rumors.len()
                    ));
                }
                if self.story.target.is_none() {
                    let day = match &self.story.draft {
                        Draft::Action(action) => action.day.as_str(),
                        _ => "",
                    };
                    egui::ComboBox::from_id_salt("previous_action_source")
                        .selected_text(
                            self.story
                                .timeline
                                .as_ref()
                                .and_then(|timeline| {
                                    timeline
                                        .rows
                                        .iter()
                                        .find(|row| row.action_id == self.story.prior_action_id)
                                })
                                .map_or(
                                    t(
                                        language,
                                        "Choose an earlier Action",
                                        "Escolher uma ação anterior",
                                    ),
                                    |row| row.snapshot.draft.day(),
                                ),
                        )
                        .show_ui(ui, |ui| {
                            if timeline_ready && let Some(timeline) = &self.story.timeline {
                                for row in timeline
                                    .rows
                                    .iter()
                                    .filter(|row| row.snapshot.draft.day() < day)
                                {
                                    let label = format!(
                                        "{} · {}",
                                        row.snapshot.draft.day(),
                                        row.action_id.chars().take(8).collect::<String>()
                                    );
                                    ui.selectable_value(
                                        &mut self.story.prior_action_id,
                                        row.action_id.clone(),
                                        label,
                                    );
                                }
                            }
                        });
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(
                            &mut self.story.copy_options.carry_unused,
                            t(
                                language,
                                "Copy recorded unused values",
                                "Copiar valores não usados registrados",
                            ),
                        );
                        ui.checkbox(
                            &mut self.story.copy_options.carry_growth,
                            t(
                                language,
                                "Copy recorded growth values",
                                "Copiar valores de crescimento registrados",
                            ),
                        );
                        let button = ui.add_enabled(
                            timeline_ready && !self.story.prior_action_id.is_empty(),
                            egui::Button::new(t(
                                language,
                                "Preview previous values",
                                "Prévia dos valores anteriores",
                            )),
                        );
                        set_probe(ui, &button, "action_copy_prepare");
                        preview = button.clicked();
                    });
                    if let Some(copy) = &self.story.copy_preview {
                        ui.label(format!(
                            "{} {} · {} {}",
                            t(language, "Source date", "Data de origem"),
                            copy.source_day,
                            t(language, "Activity slots", "Atividades"),
                            copy.draft.slots.len()
                        ));
                        for slot in &copy.draft.slots {
                            ui.label(format!(
                                "{} · {} {} · {} {} · {} {}",
                                slot.name,
                                t(language, "total", "total"),
                                slot.total,
                                t(language, "unused", "não usado"),
                                slot.unused,
                                t(language, "growth", "crescimento"),
                                slot.growth
                            ));
                        }
                        let button = ui.add_enabled(
                            copy_ready,
                            egui::Button::new(t(
                                language,
                                "Use preview in new draft",
                                "Usar prévia no novo rascunho",
                            )),
                        );
                        set_probe(ui, &button, "action_copy_apply");
                        if button.clicked() {
                            if matches!(
                                &self.story.draft,
                                Draft::Action(Action { slots, .. }) if !slots.is_empty()
                            ) {
                                self.story.confirm_copy = true;
                            } else {
                                apply = true;
                            }
                        }
                    }
                    if self.story.confirm_copy {
                        ui.label(t(
                            language,
                            "Replace the current new Action draft with this preview?",
                            "Substituir o novo rascunho atual por esta prévia?",
                        ));
                        ui.horizontal(|ui| {
                            apply = ui
                                .add_enabled(
                                    copy_ready,
                                    egui::Button::new(t(
                                        language,
                                        "Replace new draft",
                                        "Substituir novo rascunho",
                                    )),
                                )
                                .clicked();
                            if ui.button(t(language, "Cancel", "Cancelar")).clicked() {
                                self.story.confirm_copy = false;
                            }
                        });
                    }
                }
            });
        if prepare {
            self.prepare_action_timeline();
        }
        if preview {
            self.prepare_action_copy();
        }
        if apply {
            self.apply_action_copy();
        }
        if export {
            match self.action_timeline_document() {
                Ok(html) => {
                    if let Some(path) = FileDialog::new()
                        .set_file_name(if self.story.include_rumors {
                            "Inconnu-private-character-story.html"
                        } else {
                            "Inconnu-private-action-timeline.html"
                        })
                        .add_filter("HTML", &["html"])
                        .save_file()
                    {
                        self.story.message = Some(
                            match grapevine_workbench::safe_files::write_new(&path, html.as_bytes())
                            {
                                Ok(()) => {
                                    t(language, "Timeline saved.", "Linha do tempo salva.").into()
                                }
                                Err(error) => error.message,
                            },
                        );
                    }
                }
                Err(error) => self.story.message = Some(error.message),
            }
        }
    }
}
fn set_probe(ui: &egui::Ui, response: &egui::Response, key: &'static str) {
    let rect = ui
        .clip_rect()
        .contains_rect(response.rect)
        .then_some(response.rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::unique(key), rect));
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use grapevine_workbench::{action_links::CopyOptions, actions::ActivitySlot};
    #[test]
    fn timeline_and_copy_preview_remain_read_only_until_explicit_apply_and_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("action-context.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let character = lib
            .create_character("Vampire", "Holder", 0, Role::Storyteller)
            .unwrap();
        let source = Action {
            character: "Holder".into(),
            day: "2026-01-01".into(),
            slots: vec![ActivitySlot {
                name: "Investigation".into(),
                level: "2".into(),
                total: "6".into(),
                unused: "3".into(),
                growth: "4".into(),
                action: "SOURCE_ACTION".into(),
                result: "SOURCE_RESULT".into(),
            }],
        };
        let source_id = lib
            .create_linked_action(&source, Some(&character), 1, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.load_story(Kind::Action, None);
        app.story.character_id = Some(character.clone());
        app.story.draft = Draft::Action(Action {
            character: "Holder".into(),
            day: "2026-02-01".into(),
            slots: Vec::new(),
        });
        app.story.from = "2026-01-01".into();
        app.story.through = "2026-02-01".into();
        app.story.prior_action_id = source_id.clone();
        app.story.copy_options = CopyOptions {
            carry_unused: true,
            carry_growth: false,
        };
        app.prepare_action_timeline();
        assert!(app.action_timeline_document().is_ok());
        app.prepare_action_copy();
        assert_eq!(app.library.as_ref().unwrap().revision().unwrap(), 2);
        assert!(matches!(&app.story.draft,Draft::Action(action) if action.slots.is_empty()));
        app.story.copy_options.carry_growth = true;
        assert!(!app.action_copy_ready());
        app.apply_action_copy();
        assert!(matches!(&app.story.draft,Draft::Action(action) if action.slots.is_empty()));
        app.story.copy_options.carry_growth = false;
        app.prepare_action_copy();
        let mut writer = Library::open(&path).unwrap();
        writer
            .create_player("Concurrent", 2, Role::Storyteller)
            .unwrap();
        assert!(!app.action_copy_ready());
        app.apply_action_copy();
        assert!(matches!(&app.story.draft,Draft::Action(action) if action.slots.is_empty()));
        app.prepare_action_timeline();
        app.prepare_action_copy();
        app.apply_action_copy();
        let Draft::Action(draft) = &app.story.draft else {
            panic!("Action draft")
        };
        assert_eq!(draft.slots[0].unused, "3");
        assert_eq!(draft.slots[0].growth, "0");
        assert!(draft.slots[0].action.is_empty() && draft.slots[0].result.is_empty());
        assert_eq!(app.library.as_ref().unwrap().revision().unwrap(), 3);
        app.save_story_draft();
        let saved = app.story.target.as_ref().unwrap();
        let binding = writer
            .linked_action_snapshot(saved, Role::Storyteller)
            .unwrap();
        assert_eq!(binding.binding.unwrap().binding.character_id, character);
        let old = writer
            .linked_action_snapshot(&source_id, Role::Storyteller)
            .unwrap();
        assert_eq!(old.story.draft, Draft::Action(source));
        app.change_route(WorkArea::Player);
        assert!(
            app.story.character_id.is_none()
                && app.story.timeline.is_none()
                && app.story.copy_preview.is_none()
        );
        assert!(app.action_timeline_document().is_err());
    }

    #[test]
    fn combined_view_is_optional_readonly_and_bound_to_character_dates_and_revision() {
        use grapevine_workbench::rumors::{Entry, Rumor};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("combined-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let character = lib
            .create_character("Vampire", "Holder", 0, Role::Storyteller)
            .unwrap();
        let action = lib
            .create_linked_action(
                &Action {
                    character: "Holder".into(),
                    day: "2026-11-01".into(),
                    slots: Vec::new(),
                },
                Some(&character),
                1,
                Role::Storyteller,
            )
            .unwrap();
        let rumor = lib
            .create_story(
                &Draft::Rumor(Rumor {
                    title: "Selected rumor".into(),
                    day: "2026-11-02".into(),
                    entries: vec![Entry {
                        level: "0".into(),
                        body: "RUMOR_REVIEW_TEXT".into(),
                    }],
                    ..Default::default()
                }),
                2,
                Role::Storyteller,
            )
            .unwrap();
        let preview = lib
            .preview_rumor_audience(
                &rumor,
                std::slice::from_ref(&character),
                3,
                Role::Storyteller,
            )
            .unwrap();
        lib.commit_rumor_audience(&preview, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.load_story(Kind::Action, Some(action));
        app.story.from = "2026-11-01".into();
        app.story.through = "2026-11-30".into();
        app.prepare_action_timeline();
        assert!(
            !app.action_timeline_document()
                .unwrap()
                .contains("RUMOR_REVIEW_TEXT")
        );
        app.story.include_rumors = true;
        assert!(app.action_timeline_document().is_err());
        app.prepare_action_timeline();
        assert_eq!(app.story.timeline.as_ref().unwrap().rows.len(), 1);
        assert_eq!(app.story.combined.as_ref().unwrap().rumors.len(), 1);
        app.language = Language::Portuguese;
        app.story.document_language = Some(Language::Portuguese);
        assert!(
            app.action_timeline_document()
                .unwrap()
                .contains("RUMOR_REVIEW_TEXT")
        );
        assert_eq!(app.library.as_ref().unwrap().revision().unwrap(), 4);
        app.story.through = "2026-11-01".into();
        assert!(app.action_timeline_document().is_err());
        app.story.through = "2026-11-30".into();
        let mut writer = Library::open(&path).unwrap();
        writer.create_player("Other", 4, Role::Storyteller).unwrap();
        assert!(app.action_timeline_document().is_err());
        app.prepare_action_timeline();
        assert!(app.action_timeline_document().is_ok());
        app.change_route(WorkArea::Player);
        assert!(app.story.combined.is_none() && !app.story.include_rumors);
        assert!(app.action_timeline_document().is_err());
    }
}
