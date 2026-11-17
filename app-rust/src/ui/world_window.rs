use super::*;
use grapevine_workbench::world;
use serde_json::Value;

pub(super) struct WorldState {
    pub open: bool,
    pub kind: String,
    pub target: Option<String>,
    pub edit: world::Edit,
    saved: world::Edit,
    revision: Option<i64>,
    pub(super) message: Option<String>,
    search: String,
    document_language: Option<Language>,
    include_notes: bool,
    retained_fields: Vec<String>,
}
impl Default for WorldState {
    fn default() -> Self {
        Self {
            open: false,
            kind: "Item".into(),
            target: None,
            edit: Default::default(),
            saved: Default::default(),
            revision: None,
            message: None,
            search: String::new(),
            document_language: None,
            include_notes: false,
            retained_fields: Vec::new(),
        }
    }
}
impl WorldState {
    pub(super) fn synthetic_draft(revision: Option<i64>) -> Self {
        Self {
            open: true,
            revision,
            ..Default::default()
        }
    }
    pub(super) fn dirty(&self) -> bool {
        self.edit != self.saved
    }
}
use grapevine_workbench::world_card::label;
impl InconnuApp {
    pub(super) fn load_world(&mut self, id: Option<String>, kind: String) {
        if self.route != WorkArea::Storyteller || !world::is_world_kind(&kind) {
            return;
        }
        let Some(lib) = &self.library else { return };
        let revision = lib.revision().ok();
        let edit = match &id {
            Some(id) => lib.world_edit(id, Role::Storyteller),
            None => Ok(Default::default()),
        };
        match edit {
            Ok(edit) => {
                let retained_fields = id
                    .as_ref()
                    .and_then(|id| lib.record(id, Role::Storyteller).ok())
                    .map(|view| {
                        world::text_fields(&kind)
                            .iter()
                            .chain(world::integer_fields(&kind))
                            .chain(world::list_fields(&kind))
                            .filter(|key| {
                                view.fields.contains_key(**key) && !edit.fields.contains_key(**key)
                            })
                            .map(|key| (*key).to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                self.world = WorldState {
                    open: true,
                    kind,
                    target: id,
                    saved: edit.clone(),
                    edit,
                    revision,
                    retained_fields,
                    ..Default::default()
                }
            }
            Err(e) => self.log_error("world.read", &e),
        }
    }
    fn save_world_draft(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        // Other forms keep their own revision: do not silently rebase or discard them.
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.world.message = Some(
                self.t(
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(revision)) = (&mut self.library, self.world.revision) else {
            return;
        };
        let result = if let Some(id) = &self.world.target {
            lib.save_world(id, &self.world.edit, revision, Role::Storyteller)
                .map(|_| id.clone())
        } else {
            lib.create_world(
                &self.world.kind,
                &self.world.edit,
                revision,
                Role::Storyteller,
            )
        };
        match result {
            Ok(id) => {
                self.world.target = Some(id);
                self.world.saved = self.world.edit.clone();
                self.world.revision = lib.revision().ok();
                self.revision = self.world.revision;
                self.world.message = Some(
                    self.t(
                        "Saved. Other open drafts retain their revision.",
                        "Salvo. Outros rascunhos abertos mantêm sua revisão.",
                    )
                    .into(),
                );
                self.refresh_summaries(true);
            }
            Err(e) => self.world.message = Some(format!("{}: {}", e.code, e.message)),
        }
    }
    pub(super) fn render_world(&mut self, ctx: &egui::Context) {
        if !self.world.open || self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        let language = self.language;
        let rows: Vec<_> = self
            .summaries
            .iter()
            .filter(|r| world::is_world_kind(&r.kind) && r.build.is_none())
            .cloned()
            .collect();
        let mut open = true;
        let mut select = None;
        let mut save = false;
        let mut export = false;
        egui::Window::new(self.t("Items, locations and rotes", "Itens, locais e rotinas"))
            .id(egui::Id::unique("world_window"))
            .default_size([800.0, 620.0])
            .vscroll(true)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(self.t(
                    "Descriptive records; values do not verify game rules or ownership.",
                    "Registros descritivos; os valores não verificam regras ou propriedade.",
                ));
                ui.horizontal(|ui| {
                    for kind in ["Item", "Location", "Rote"] {
                        if ui
                            .button(format!(
                                "{} {}",
                                self.t("New", "Novo"),
                                label(kind, language)
                            ))
                            .clicked()
                        {
                            select = Some((None, kind.into()));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(self.t("Search", "Buscar"));
                    ui.text_edit_singleline(&mut self.world.search);
                });
                egui::ScrollArea::vertical()
                    .id_salt("world_browser")
                    .max_height(110.0)
                    .show(ui, |ui| {
                        let query = self.world.search.to_lowercase();
                        for row in &rows {
                            if row.name.to_lowercase().contains(&query)
                                && ui
                                    .selectable_label(
                                        self.world.target.as_ref() == Some(&row.id),
                                        format!("{} · {}", label(&row.kind, language), row.name),
                                    )
                                    .clicked()
                            {
                                select = Some((Some(row.id.clone()), row.kind.clone()));
                            }
                        }
                    });
                ui.separator();
                if self.world.revision.is_none() {
                    ui.label(self.t(
                        "Choose a record or create one.",
                        "Escolha um registro ou crie um.",
                    ));
                    return;
                }
                ui.heading(label(&self.world.kind, language));
                ui.horizontal(|ui| {
                    ui.label(self.t("Name", "Nome"));
                    ui.add(egui::TextEdit::singleline(&mut self.world.edit.name).char_limit(4096));
                });
                egui::ScrollArea::vertical()
                    .id_salt("world_fields")
                    .max_height((ctx.content_rect().height() - 370.0).clamp(100.0, 340.0))
                    .show(ui, |ui| {
                        let kind = self.world.kind.clone();
                        for key in world::text_fields(&kind) {
                            if self.world.retained_fields.iter().any(|field| field == key) {
                                ui.label(format!(
                                    "{}: {}",
                                    label(key, language),
                                    self.t(
                                        "Unsupported imported value retained unchanged.",
                                        "Valor importado não compatível preservado sem alterações."
                                    )
                                ));
                                continue;
                            }
                            ui.push_id(key, |ui| {
                                ui.label(label(key, language));
                                if let Some(Value::String(value)) =
                                    self.world.edit.fields.get_mut(*key)
                                {
                                    ui.add(
                                        egui::TextEdit::multiline(value)
                                            .desired_rows(2)
                                            .char_limit(16000),
                                    );
                                } else if !self.world.edit.fields.contains_key(*key) {
                                    if ui.button(self.t("Add field", "Adicionar campo")).clicked() {
                                        self.world
                                            .edit
                                            .fields
                                            .insert((*key).into(), Value::String(String::new()));
                                    }
                                } else {
                                    ui.label(self.t(
                                        "Unsupported imported value retained.",
                                        "Valor importado não compatível preservado.",
                                    ));
                                }
                            });
                        }
                        for key in world::integer_fields(&kind) {
                            if self.world.retained_fields.iter().any(|field| field == key) {
                                ui.label(format!(
                                    "{}: {}",
                                    label(key, language),
                                    self.t(
                                        "Unsupported imported value retained unchanged.",
                                        "Valor importado não compatível preservado sem alterações."
                                    )
                                ));
                                continue;
                            }
                            ui.push_id(key, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(label(key, language));
                                    if let Some(mut value) =
                                        self.world.edit.fields.get(*key).and_then(Value::as_i64)
                                    {
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut value)
                                                    .range(i16::MIN as i64..=i16::MAX as i64),
                                            )
                                            .changed()
                                        {
                                            self.world
                                                .edit
                                                .fields
                                                .insert((*key).into(), value.into());
                                        }
                                    } else if !self.world.edit.fields.contains_key(*key) {
                                        if ui
                                            .button(self.t("Add value", "Adicionar valor"))
                                            .clicked()
                                        {
                                            self.world.edit.fields.insert((*key).into(), 0.into());
                                        }
                                    } else {
                                        ui.label(self.t(
                                            "Unsupported imported value retained.",
                                            "Valor importado não compatível preservado.",
                                        ));
                                    }
                                });
                            });
                        }
                        for key in world::list_fields(&kind) {
                            if self.world.retained_fields.iter().any(|field| field == key) {
                                ui.label(format!(
                                    "{}: {}",
                                    label(key, language),
                                    self.t(
                                        "Unsupported imported value retained unchanged.",
                                        "Valor importado não compatível preservado sem alterações."
                                    )
                                ));
                                continue;
                            }
                            ui.push_id(key, |ui| {
                                egui::CollapsingHeader::new(label(key, language)).show(ui, |ui| {
                                    if let Some(value) = self.world.edit.fields.get_mut(*key) {
                                        crate::sheet_ui::render_list(ui, key, value, language);
                                    } else if ui
                                        .button(self.t("Add list", "Adicionar lista"))
                                        .clicked()
                                    {
                                        self.world.edit.fields.insert(
                                            (*key).into(),
                                            grapevine_workbench::sheet::empty_list(key),
                                        );
                                    }
                                });
                            });
                        }
                    });
                let response = ui.button(self.t("Save record", "Salvar registro"));
                let visible = ui.is_rect_visible(response.rect).then_some(response.rect);
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::unique("world_save"), visible));
                save = response.clicked();
                ui.horizontal(|ui| {
                    let lang = self.world.document_language.get_or_insert(language);
                    ui.label(if language == Language::English {
                        "Document language"
                    } else {
                        "Idioma do documento"
                    });
                    ui.selectable_value(lang, Language::English, "English");
                    ui.selectable_value(lang, Language::Portuguese, "Português");
                });
                ui.checkbox(
                    &mut self.world.include_notes,
                    if language == Language::English {
                        "Include private notes"
                    } else {
                        "Incluir notas privadas"
                    },
                );
                let export_button = ui.add_enabled(
                    self.world.target.is_some() && !self.world.dirty(),
                    egui::Button::new(self.t(
                        "Save private printable card…",
                        "Salvar cartão privado para impressão…",
                    )),
                );
                let export_visible = ui
                    .clip_rect()
                    .contains_rect(export_button.rect)
                    .then_some(export_button.rect);
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::unique("world_export"), export_visible));
                export = export_button.clicked();
                if self.world.dirty() {
                    ui.label(self.t("Unsaved changes", "Alterações não salvas"));
                }
                if let Some(message) = &self.world.message {
                    ui.label(message);
                }
            });
        self.world.open = open;
        if let Some((id, kind)) = select {
            self.request_action(PendingAction::WorldRecord(id, kind));
        }
        if save {
            self.save_world_draft();
        }
        if export
            && let Some(path) = FileDialog::new()
                .set_title(self.t("Private Storyteller card", "Cartão privado do Narrador"))
                .set_file_name("Inconnu-private-card.html")
                .add_filter("HTML", &["html"])
                .save_file()
        {
            let result = grapevine_workbench::world_card::html(
                &self.world.kind,
                &self.world.edit,
                self.world.document_language.unwrap_or(language),
                self.world.include_notes,
            )
            .and_then(|html| grapevine_workbench::safe_files::write_new(&path, html.as_bytes()));
            self.world.message = Some(match result {
                Ok(()) => self
                    .t(
                        "Private card saved; open it in a browser to print or save as PDF.",
                        "Cartão privado salvo; abra no navegador para imprimir ou salvar como PDF.",
                    )
                    .into(),
                Err(e) => e.message,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_drafts_preserve_close_language_and_guard_navigation_then_clear_on_player_route() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("world-ui.gvlib");
        drop(Library::create(&path, "Synthetic world UI").unwrap());
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.load_world(None, "Location".into());
        app.world.edit.name = "Private location".into();
        app.world
            .edit
            .fields
            .insert("Owner".into(), "Free text, not an identity".into());
        assert!(app.has_unsaved_edit());
        app.world.open = false;
        app.language = Language::Portuguese;
        app.world.open = true;
        assert_eq!(app.world.edit.name, "Private location");
        app.request_action(PendingAction::WorldRecord(None, "Rote".into()));
        assert!(app.pending_action.is_some());
        assert_eq!(app.world.kind, "Location");
        app.pending_action = None;
        app.save_world_draft();
        assert!(app.world.target.is_some());
        assert!(!app.world.dirty());
        let id = app.world.target.clone().unwrap();
        app.load_world(Some(id), "Location".into());
        assert_eq!(app.world.edit.fields["Owner"], "Free text, not an identity");
        app.change_route(WorkArea::Player);
        assert!(!app.world.open && app.world.target.is_none() && app.world.edit.name.is_empty());
    }
    #[test]
    fn stale_world_draft_is_retained_without_rebasing_after_another_write() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("world-stale.gvlib");
        drop(Library::create(&path, "Synthetic world UI").unwrap());
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.load_world(None, "Item".into());
        app.world.edit.name = "Keep draft".into();
        let lib = app.library.as_mut().unwrap();
        lib.create_player("Other writer", lib.revision().unwrap(), Role::Storyteller)
            .unwrap();
        app.save_world_draft();
        assert!(app.world.target.is_none() && app.world.dirty());
        assert!(app.world.message.as_ref().unwrap().contains("STALE"));
        assert_eq!(app.world.edit.name, "Keep draft");
    }
}
