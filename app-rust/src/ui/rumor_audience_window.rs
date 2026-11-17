use super::*;
use grapevine_workbench::rumor_audience::{Preview as AudiencePreview, Queue, Snapshot};
use std::collections::BTreeSet;

pub(super) struct RumorAudienceWindow {
    pub open: bool,
    pub snapshot: Option<Snapshot>,
    pub selected: BTreeSet<String>,
    saved: BTreeSet<String>,
    pub preview: Option<AudiencePreview>,
    pub from: String,
    pub through: String,
    pub queue: Option<Queue>,
    search: String,
    language: Language,
    pub message: Option<String>,
}
impl Default for RumorAudienceWindow {
    fn default() -> Self {
        Self {
            open: false,
            snapshot: None,
            selected: BTreeSet::new(),
            saved: BTreeSet::new(),
            preview: None,
            from: String::new(),
            through: String::new(),
            queue: None,
            search: String::new(),
            language: Language::English,
            message: None,
        }
    }
}
impl RumorAudienceWindow {
    pub(super) fn synthetic(snapshot: Snapshot, character_id: String) -> Self {
        let saved: BTreeSet<_> = snapshot
            .recipients
            .iter()
            .map(|r| r.binding.character_id.clone())
            .collect();
        let mut selected = saved.clone();
        selected.insert(character_id);
        Self {
            open: true,
            snapshot: Some(snapshot),
            saved,
            selected,
            from: "2026-11-01".into(),
            through: "2026-11-30".into(),
            language: Language::Portuguese,
            ..Default::default()
        }
    }
    pub(super) fn dirty(&self) -> bool {
        self.selected != self.saved
    }
    fn preview_current(&self, revision: Option<i64>) -> bool {
        self.preview.as_ref().is_some_and(|p| {
            Some(p.revision) == revision
                && self
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.rumor_id == p.rumor_id)
                && p.character_ids == self.selected.iter().cloned().collect::<Vec<_>>()
        })
    }
    fn snapshot_current(&self, revision: Option<i64>) -> bool {
        !self.dirty()
            && self
                .snapshot
                .as_ref()
                .is_some_and(|s| Some(s.revision) == revision)
    }
    fn queue_current(&self, revision: Option<i64>) -> bool {
        self.queue.as_ref().is_some_and(|q| {
            Some(q.revision) == revision
                && q.from == self.from.trim()
                && q.through == self.through.trim()
        })
    }
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
fn set_probe(ui: &egui::Ui, response: &egui::Response, key: &'static str) {
    let visible = ui
        .clip_rect()
        .contains_rect(response.rect)
        .then_some(response.rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::unique(key), visible));
}
impl InconnuApp {
    pub(super) fn open_rumor_audience(&mut self) {
        if self.route == WorkArea::Storyteller && self.library.is_some() {
            self.rumor_audience.open = true;
        }
    }
    pub(super) fn load_rumor_audience(&mut self, id: String) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        match lib.rumor_audience_snapshot(&id, Role::Storyteller) {
            Ok(snapshot) => {
                let selected: BTreeSet<_> = snapshot
                    .recipients
                    .iter()
                    .map(|r| r.binding.character_id.clone())
                    .collect();
                self.rumor_audience = RumorAudienceWindow {
                    open: true,
                    selected: selected.clone(),
                    saved: selected,
                    snapshot: Some(snapshot),
                    from: self.rumor_audience.from.clone(),
                    through: self.rumor_audience.through.clone(),
                    queue: self.rumor_audience.queue.take(),
                    language: self.rumor_audience.language,
                    ..Default::default()
                };
            }
            Err(e) => self.log_error("rumor.audience.read", &e),
        }
    }
    pub(super) fn prepare_rumor_queue(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        match lib.rumor_audience_queue(
            self.rumor_audience.from.trim(),
            self.rumor_audience.through.trim(),
            Role::Storyteller,
        ) {
            Ok(queue) => {
                self.rumor_audience.queue = Some(queue);
                self.rumor_audience.message = None;
            }
            Err(e) => {
                self.rumor_audience.queue = None;
                self.rumor_audience.message = Some(format!("{}: {}", e.code, e.message));
            }
        }
    }
    pub(super) fn preview_rumor_audience(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let (Some(lib), Some(snapshot)) = (&self.library, &self.rumor_audience.snapshot) else {
            return;
        };
        let ids: Vec<_> = self.rumor_audience.selected.iter().cloned().collect();
        match lib.preview_rumor_audience(
            &snapshot.rumor_id,
            &ids,
            snapshot.revision,
            Role::Storyteller,
        ) {
            Ok(preview) => {
                self.rumor_audience.preview = Some(preview);
                self.rumor_audience.message = None;
            }
            Err(e) => {
                self.rumor_audience.preview = None;
                self.rumor_audience.message = Some(format!("{}: {}", e.code, e.message));
            }
        }
    }
    pub(super) fn save_rumor_audience(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet || self.story.dirty()
        {
            self.rumor_audience.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet and story drafts first.",
                    "Salve ou descarte os rascunhos da ficha e da história primeiro.",
                )
                .into(),
            );
            return;
        }
        let revision = self.library.as_ref().and_then(|lib| lib.revision().ok());
        if !self.rumor_audience.preview_current(revision) {
            self.rumor_audience.preview = None;
            return;
        }
        let (Some(lib), Some(preview)) = (&mut self.library, &self.rumor_audience.preview) else {
            return;
        };
        match lib.commit_rumor_audience(preview, Role::Storyteller) {
            Ok(next) => {
                self.rumor_audience.saved = self.rumor_audience.selected.clone();
                let id = preview.rumor_id.clone();
                self.rumor_audience.snapshot =
                    lib.rumor_audience_snapshot(&id, Role::Storyteller).ok();
                if let Some(snapshot) = &self.rumor_audience.snapshot
                    && self.story.target.as_deref() == Some(id.as_str())
                    && self.story.draft
                        == grapevine_workbench::story_work::Draft::Rumor(snapshot.rumor.clone())
                {
                    self.story.revision = Some(next);
                }
                self.rumor_audience.preview = None;
                self.rumor_audience.queue = None;
                self.revision = Some(next);
                self.rumor_audience.message = Some(
                    t(
                        self.language,
                        "Audience list saved locally. Nothing was sent.",
                        "Lista de público salva localmente. Nada foi enviado.",
                    )
                    .into(),
                );
                self.refresh_summaries(true);
            }
            Err(e) => self.rumor_audience.message = Some(format!("{}: {}", e.code, e.message)),
        }
    }
    pub(super) fn render_rumor_audience(&mut self, ctx: &egui::Context) {
        if !self.rumor_audience.open
            || self.route != WorkArea::Storyteller
            || self.library.is_none()
        {
            return;
        }
        let l = self.language;
        let revision = self.library.as_ref().and_then(|lib| lib.revision().ok());
        let mut open = true;
        let (mut prepare_queue, mut preview, mut save, mut export) = (false, false, false, false);
        let mut select = None;
        let preview_current = self.rumor_audience.preview_current(revision);
        let snapshot_current = self.rumor_audience.snapshot_current(revision);
        let queue_current = self.rumor_audience.queue_current(revision);
        egui::Window::new(t(
            l,
            "Rumor audience review",
            "Revisão do público do rumor",
        ))
        .id(egui::Id::unique("rumor_audience_window"))
        .default_size([830., 640.])
        .vscroll(true)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(t(
                l,
                "Private staff list only. No sending, receipt or character access is recorded.",
                "Somente lista privada da equipe. Nenhum envio, recebimento ou acesso do personagem é registrado.",
            ));
            egui::CollapsingHeader::new(t(
                l,
                "Rumors by date",
                "Rumores por data",
            ))
            .default_open(self.rumor_audience.snapshot.is_none())
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(t(l, "From", "De"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.rumor_audience.from)
                            .desired_width(100.),
                    );
                    ui.label(t(l, "Through", "Até"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.rumor_audience.through)
                            .desired_width(100.),
                    );
                    let response = ui.button(t(l, "Prepare queue", "Preparar fila"));
                    set_probe(ui, &response, "rumor_audience_queue");
                    prepare_queue = response.clicked();
                });
                ui.label(t(
                    l,
                    "Inclusive dates, YYYY-MM-DD. Undated records are counted separately.",
                    "Datas inclusivas, AAAA-MM-DD. Registros sem data são contados separadamente.",
                ));
                if queue_current && let Some(queue) = &self.rumor_audience.queue {
                    ui.label(format!(
                        "{} {} · {} {}",
                        t(l, "Rumors", "Rumores"),
                        queue.rows.len(),
                        t(l, "Undated omitted", "Sem data omitidos"),
                        queue.undated_omitted
                    ));
                    egui::ScrollArea::vertical()
                        .id_salt("rumor_queue_rows")
                        .max_height(140.)
                        .show(ui, |ui| {
                            for row in &queue.rows {
                                if ui
                                    .selectable_label(
                                        self.rumor_audience
                                            .snapshot
                                            .as_ref()
                                            .is_some_and(|s| s.rumor_id == row.rumor_id),
                                        format!(
                                            "{} · {} · {} {}",
                                            row.day,
                                            row.title,
                                            row.recipient_count,
                                            t(l, "selected", "selecionados")
                                        ),
                                    )
                                    .clicked()
                                {
                                    select = Some(row.rumor_id.clone());
                                }
                            }
                        });
                }
            });
            if let Some(snapshot) = &self.rumor_audience.snapshot {
                ui.heading(&snapshot.rumor.title);
                ui.label(format!(
                    "{} · {}",
                    snapshot.rumor.day, snapshot.rumor.audience_label
                ));
                ui.horizontal_wrapped(|ui| {
                    let response =
                        ui.button(t(l, "Preview selected audience", "Pré-visualizar público selecionado"));
                    set_probe(ui, &response, "rumor_audience_preview");
                    preview = response.clicked();
                    let response = ui.add_enabled(
                        preview_current,
                        egui::Button::new(t(l, "Save audience list", "Salvar lista de público")),
                    );
                    set_probe(ui, &response, "rumor_audience_save");
                    save = response.clicked();
                    ui.selectable_value(
                        &mut self.rumor_audience.language,
                        Language::English,
                        "English",
                    );
                    ui.selectable_value(
                        &mut self.rumor_audience.language,
                        Language::Portuguese,
                        "Português",
                    );
                    export = ui
                        .add_enabled(
                            snapshot_current,
                            egui::Button::new(t(
                                l,
                                "Save private packet…",
                                "Salvar pacote privado…",
                            )),
                        )
                        .clicked();
                });
                ui.label(format!(
                    "{} {} / 500",
                    t(l, "Selected records", "Registros selecionados"),
                    self.rumor_audience.selected.len()
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut self.rumor_audience.search)
                        .hint_text(t(l, "Find character", "Buscar personagem")),
                );
                let query = self.rumor_audience.search.to_lowercase();
                egui::ScrollArea::vertical()
                    .id_salt("rumor_audience_characters")
                    .max_height(150.)
                    .show(ui, |ui| {
                        for row in self.summaries.iter().filter(|r| {
                            r.build.is_none()
                                && grapevine_workbench::sheet::is_character_kind(&r.kind)
                                && r.name.to_lowercase().contains(&query)
                        }) {
                            let mut chosen = self.rumor_audience.selected.contains(&row.id);
                            let label = format!(
                                "{} · {} · {}",
                                row.name,
                                row.kind,
                                row.id.chars().take(8).collect::<String>()
                            );
                            if ui.checkbox(&mut chosen, label).changed() {
                                if chosen {
                                    self.rumor_audience.selected.insert(row.id.clone());
                                } else {
                                    self.rumor_audience.selected.remove(&row.id);
                                }
                            }
                        }
                        for recipient in snapshot
                            .recipients
                            .iter()
                            .filter(|r| r.current_name.is_none())
                        {
                            let id = &recipient.binding.character_id;
                            let mut chosen = self.rumor_audience.selected.contains(id);
                            if ui
                                .checkbox(
                                    &mut chosen,
                                    format!(
                                        "{} · {}",
                                        recipient.binding.label_at_binding,
                                        t(l, "unresolved saved link", "vínculo salvo não resolvido")
                                    ),
                                )
                                .changed()
                            {
                                if chosen {
                                    self.rumor_audience.selected.insert(id.clone());
                                } else {
                                    self.rumor_audience.selected.remove(id);
                                }
                            }
                        }
                    });
                if preview_current && let Some(p) = &self.rumor_audience.preview {
                    ui.strong(t(
                        l,
                        "Prepared audience — review before saving",
                        "Público preparado — revise antes de salvar",
                    ));
                    egui::ScrollArea::vertical()
                        .id_salt("rumor_audience_preview_rows")
                        .max_height(100.)
                        .show(ui, |ui| {
                            for recipient in &p.recipients {
                                ui.label(
                                    recipient
                                        .current_name
                                        .as_deref()
                                        .unwrap_or(&recipient.binding.label_at_binding),
                                );
                            }
                        });
                }
                egui::CollapsingHeader::new(t(
                    l,
                    "Saved rumor text",
                    "Texto salvo do rumor",
                ))
                .show(ui, |ui| {
                    for entry in &snapshot.rumor.entries {
                        ui.strong(format!("{} {}", t(l, "Level", "Nível"), entry.level));
                        ui.label(&entry.body);
                    }
                    if snapshot.legacy || snapshot.retained {
                        ui.label(t(
                            l,
                            "Extra source query, recipient and completion evidence remains preserved for manual review.",
                            "Evidências adicionais da consulta, destinatários e conclusão da origem permanecem preservadas para revisão manual.",
                        ));
                    }
                });
            }
            if let Some(message) = &self.rumor_audience.message {
                ui.label(message);
            }
        });
        self.rumor_audience.open = open;
        if prepare_queue {
            self.prepare_rumor_queue();
        }
        if let Some(id) = select {
            self.request_action(PendingAction::RumorAudience(id));
        }
        if preview {
            self.preview_rumor_audience();
        }
        if save {
            self.save_rumor_audience();
        }
        if export
            && self.rumor_audience.snapshot_current(revision)
            && let Some(snapshot) = &self.rumor_audience.snapshot
        {
            match grapevine_workbench::rumor_packet::html(snapshot, self.rumor_audience.language) {
                Ok(html) => {
                    if let Some(path) = FileDialog::new()
                        .set_file_name("private-rumor-audience.html")
                        .save_file()
                        && let Err(e) =
                            grapevine_workbench::safe_files::write_new(&path, html.as_bytes())
                    {
                        self.rumor_audience.message = Some(format!("{}: {}", e.code, e.message));
                    }
                }
                Err(e) => self.rumor_audience.message = Some(format!("{}: {}", e.code, e.message)),
            }
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use grapevine_workbench::{
        rumors::{Entry, Rumor},
        story_work::Draft,
    };

    #[test]
    fn audience_preview_is_readonly_scoped_and_cleared_in_player_view() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rumor-audience-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let character = lib
            .create_character("Vampire", "Same name", 0, Role::Storyteller)
            .unwrap();
        let rumor = lib
            .create_story(
                &Draft::Rumor(Rumor {
                    title: "Synthetic rumor".into(),
                    day: "2026-11-11".into(),
                    entries: vec![Entry {
                        level: "0".into(),
                        body: "Private staff text".into(),
                    }],
                    ..Default::default()
                }),
                1,
                Role::Storyteller,
            )
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.load_rumor_audience(rumor.clone());
        app.rumor_audience.selected.insert(character.clone());
        app.preview_rumor_audience();
        assert!(app.rumor_audience.preview_current(Some(2)));
        assert_eq!(app.library.as_ref().unwrap().revision().unwrap(), 2);
        app.rumor_audience.open = false;
        app.language = Language::Portuguese;
        app.open_rumor_audience();
        assert!(app.rumor_audience.selected.contains(&character));
        assert!(app.has_unsaved_edit());
        let mut other = Library::open(&path).unwrap();
        other
            .create_player("Other writer", 2, Role::Storyteller)
            .unwrap();
        app.save_rumor_audience();
        assert!(app.rumor_audience.dirty());
        assert!(app.rumor_audience.preview.is_none());
        assert!(
            other
                .rumor_audience_snapshot(&rumor, Role::Storyteller)
                .unwrap()
                .recipients
                .is_empty()
        );
        app.load_rumor_audience(rumor.clone());
        app.rumor_audience.selected.insert(character);
        app.preview_rumor_audience();
        app.save_rumor_audience();
        assert!(!app.rumor_audience.dirty());
        assert!(app.rumor_audience.snapshot_current(Some(4)));
        app.rumor_audience.from = "2026-11-01".into();
        app.rumor_audience.through = "2026-11-30".into();
        app.prepare_rumor_queue();
        assert!(app.rumor_audience.queue_current(Some(4)));
        app.rumor_audience.through = "2026-12-01".into();
        assert!(!app.rumor_audience.queue_current(Some(4)));
        app.change_route(WorkArea::Player);
        assert!(app.rumor_audience.snapshot.is_none());
        assert!(app.rumor_audience.preview.is_none());
        assert!(app.rumor_audience.selected.is_empty());
        app.open_rumor_audience();
        assert!(!app.rumor_audience.open);
    }
}
