use super::*;
use grapevine_workbench::boon_book::{Edit as BoonEdit, Snapshot};

#[derive(Default)]
pub(super) struct BoonWindow {
    pub open: bool,
    snapshot: Option<Snapshot>,
    selected: Option<String>,
    edit: BoonEdit,
    saved: BoonEdit,
    confirm_remove: bool,
    pub message: Option<String>,
}

impl BoonWindow {
    pub(super) fn dirty(&self) -> bool {
        self.edit != self.saved
    }
    pub(super) fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub(super) fn holder_id(&self) -> Option<&str> {
        self.snapshot.as_ref().map(|s| s.holder_id.as_str())
    }
    pub(super) fn synthetic(snapshot: Snapshot) -> Self {
        Self {
            open: true,
            snapshot: Some(snapshot),
            edit: BoonEdit {
                partner: "Synthetic partner".into(),
                kind: "Synthetic minor".into(),
                owed_by_holder: Some(true),
                day: "2026-01-02".into(),
                description: "PRIVATE_BOON_DESCRIPTION".into(),
            },
            ..Default::default()
        }
    }
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
impl InconnuApp {
    pub(super) fn open_boons(&mut self) {
        if self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        if self.boons.snapshot.is_some() {
            self.boons.open = true;
            return;
        }
        let selected = self.selected_id.as_ref().and_then(|id| {
            self.summaries
                .iter()
                .find(|s| s.id == *id && s.kind == "Vampire" && s.build.is_none())
        });
        if let Some(holder) = selected.or_else(|| {
            self.summaries
                .iter()
                .find(|s| s.kind == "Vampire" && s.build.is_none())
        }) {
            self.load_boon_entry(holder.id.clone(), None);
        }
    }
    pub(super) fn load_boon_entry(&mut self, holder: String, selected: Option<String>) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        match lib.boon_snapshot(&holder, Role::Storyteller) {
            Ok(snapshot) => {
                let entry = selected
                    .as_ref()
                    .and_then(|id| snapshot.entries.iter().find(|entry| &entry.id == id));
                if selected.is_some() && entry.is_none() {
                    self.boons.message = Some(
                        t(
                            self.language,
                            "The selected Boon is unavailable. Reload the holder.",
                            "O favor selecionado está indisponível. Recarregue o titular.",
                        )
                        .into(),
                    );
                    return;
                }
                let edit = entry.map_or_else(BoonEdit::default, |entry| entry.edit.clone());
                self.boons = BoonWindow {
                    open: true,
                    snapshot: Some(snapshot),
                    selected,
                    saved: edit.clone(),
                    edit,
                    ..Default::default()
                };
            }
            Err(e) => self.log_error("boon.read", &e),
        }
    }
    fn save_boon(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.boons.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(snapshot)) = (&mut self.library, &self.boons.snapshot) else {
            return;
        };
        let holder = snapshot.holder_id.clone();
        let revision = snapshot.revision;
        let result = match &self.boons.selected {
            Some(id) => lib
                .edit_boon(&holder, id, &self.boons.edit, revision, Role::Storyteller)
                .map(|_| id.clone()),
            None => lib.add_boon(&holder, &self.boons.edit, revision, Role::Storyteller),
        };
        match result {
            Ok(id) => {
                self.revision = Some(revision + 1);
                self.load_boon_entry(holder, Some(id));
                self.boons.message = Some(t(self.language, "Boon saved.", "Favor salvo.").into());
                self.refresh_summaries(true);
            }
            Err(e) => self.boons.message = Some(format!("{} - {}", e.code, e.message)),
        }
    }
    fn remove_boon(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.boons.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(snapshot), Some(id)) = (
            &mut self.library,
            &self.boons.snapshot,
            &self.boons.selected,
        ) else {
            return;
        };
        let holder = snapshot.holder_id.clone();
        match lib.remove_boon(&holder, id, snapshot.revision, Role::Storyteller) {
            Ok(revision) => {
                self.revision = Some(revision);
                self.load_boon_entry(holder, None);
                self.boons.message = Some(
                    t(
                        self.language,
                        "Boon removed from the active list.",
                        "Favor removido da lista ativa.",
                    )
                    .into(),
                );
                self.refresh_summaries(true);
            }
            Err(e) => self.boons.message = Some(format!("{} - {}", e.code, e.message)),
        }
    }
    pub(super) fn render_boons(&mut self, ctx: &egui::Context) {
        if !self.boons.open || self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        let l = self.language;
        let mut open = true;
        let mut selection = None;
        let mut save = false;
        let mut remove = false;
        egui::Window::new(t(l, "Vampire Boons", "Favores de Vampiros"))
            .id(egui::Id::unique("boon_window"))
            .default_size([760., 540.])
            .vscroll(true)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let title = self.boons.snapshot.as_ref().map_or(
                        t(l, "Choose holder", "Escolher titular"),
                        |snapshot| snapshot.holder_name.as_str(),
                    );
                    egui::ComboBox::from_id_salt("boon_holder")
                        .selected_text(title)
                        .show_ui(ui, |ui| {
                            let holders: Vec<_> = self
                                .summaries
                                .iter()
                                .filter(|summary| {
                                    summary.kind == "Vampire" && summary.build.is_none()
                                })
                                .collect();
                            for holder in &holders {
                                let label = if holders
                                    .iter()
                                    .filter(|other| other.name == holder.name)
                                    .count()
                                    > 1
                                {
                                    format!(
                                        "{} · {}",
                                        holder.name,
                                        holder.id.chars().take(8).collect::<String>()
                                    )
                                } else {
                                    holder.name.clone()
                                };
                                if ui
                                    .selectable_label(
                                        self.boons.holder_id() == Some(holder.id.as_str()),
                                        label,
                                    )
                                    .clicked()
                                {
                                    selection = Some((holder.id.clone(), None));
                                }
                            }
                        });
                    if let Some(holder) = self.boons.holder_id() {
                        if ui.button(t(l, "New Boon", "Novo favor")).clicked() {
                            selection = Some((holder.into(), None));
                        }
                        if ui
                            .button(t(l, "Reload holder", "Recarregar titular"))
                            .clicked()
                        {
                            selection = Some((holder.into(), self.boons.selected.clone()));
                        }
                    }
                });
                if let Some(snapshot) = &self.boons.snapshot {
                    ui.label(format!(
                        "{} {} · {} {}",
                        t(l, "Active Boons", "Favores ativos"),
                        snapshot.entries.len(),
                        t(l, "Removed entries retained", "Entradas removidas preservadas"),
                        snapshot.removed_count
                    ));
                    let label = |entry: &grapevine_workbench::boon_book::Entry| {
                        format!(
                            "{} · {} · {}",
                            entry.edit.kind, entry.edit.partner, entry.edit.day
                        )
                    };
                    let mut counts = std::collections::BTreeMap::new();
                    for entry in &snapshot.entries {
                        *counts.entry(label(entry)).or_insert(0usize) += 1;
                    }
                    egui::ComboBox::from_id_salt("boon_entry")
                        .selected_text(
                            self.boons
                                .selected
                                .as_ref()
                                .and_then(|id| {
                                    snapshot.entries.iter().find(|entry| &entry.id == id)
                                })
                                .map_or_else(|| t(l, "New Boon", "Novo favor").into(), label),
                        )
                        .show_ui(ui, |ui| {
                            for entry in &snapshot.entries {
                                let mut title = label(entry);
                                if counts[&title] > 1 {
                                    title.push_str(&format!(
                                        " · {}",
                                        entry.id.chars().rev().take(8).collect::<String>()
                                    ));
                                }
                                if ui
                                    .selectable_label(
                                        self.boons.selected.as_ref() == Some(&entry.id),
                                        title,
                                    )
                                    .clicked()
                                {
                                    selection = Some((
                                        snapshot.holder_id.clone(),
                                        Some(entry.id.clone()),
                                    ));
                                }
                            }
                        });
                    if let Some(entry) = snapshot
                        .entries
                        .iter()
                        .find(|entry| Some(&entry.id) == self.boons.selected.as_ref())
                        && let Some(legacy) = &entry.legacy
                    {
                        if legacy.day.is_none() {
                            ui.label(t(
                                l,
                                "Imported date requires review. Enter YYYY-MM-DD to resolve it.",
                                "A data importada requer revisão. Digite AAAA-MM-DD para resolvê-la.",
                            ));
                            if let Some(date) = legacy.raw_date.as_str() {
                                ui.label(date);
                            }
                        }
                        if legacy.owed_by_holder.is_none() {
                            ui.label(t(
                                l,
                                "Imported direction requires review.",
                                "A direção importada requer revisão.",
                            ));
                        }
                        if legacy.description.is_none() {
                            ui.label(t(
                                l,
                                "Original description retained for review.",
                                "Descrição original preservada para revisão.",
                            ));
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(t(l,"Boon type","Tipo de favor"));
                    ui.text_edit_singleline(&mut self.boons.edit.kind);
                });
                ui.horizontal(|ui| {
                    ui.label(t(l,"Partner name","Nome da contraparte"));
                    ui.text_edit_singleline(&mut self.boons.edit.partner);
                });
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.boons.edit.owed_by_holder,Some(true),t(l,"Holder owes partner","Titular deve à contraparte"));
                    ui.selectable_value(&mut self.boons.edit.owed_by_holder,Some(false),t(l,"Partner owes holder","Contraparte deve ao titular"));
                });
                ui.horizontal(|ui| {
                    ui.label(t(l,"Date (YYYY-MM-DD)","Data (AAAA-MM-DD)"));
                    ui.text_edit_singleline(&mut self.boons.edit.day);
                });
                ui.label(t(l,"Private description","Descrição privada"));
                ui.add(egui::TextEdit::multiline(&mut self.boons.edit.description)
                    .desired_rows(5).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    let button=ui.add_enabled(self.boons.dirty(),egui::Button::new(t(l,"Save Boon","Salvar favor")));
                    let rect=ui.clip_rect().contains_rect(button.rect).then_some(button.rect);
                    ui.ctx().data_mut(|data|data.insert_temp(egui::Id::unique("boon_save"),rect));
                    save=button.clicked();
                    if self.boons.selected.is_some() && ui.button(t(l,"Remove Boon…","Remover favor…")).clicked() {
                        self.boons.confirm_remove=true;
                    }
                    if self.boons.dirty() {
                        ui.label(t(l,"Unsaved changes","Alterações não salvas"));
                    }
                });
                if self.boons.confirm_remove {
                    ui.label(t(l,"Remove this Boon from the active list?","Remover este favor da lista ativa?"));
                    ui.horizontal(|ui| {
                        remove=ui.button(t(l,"Remove from active list","Remover da lista ativa")).clicked();
                        if ui.button(t(l,"Cancel","Cancelar")).clicked() {
                            self.boons.confirm_remove=false;
                        }
                    });
                }
                if let Some(message)=&self.boons.message {
                    ui.label(message);
                }
            });
        self.boons.open = open;
        if let Some((holder, entry)) = selection {
            self.request_action(PendingAction::BoonEntry(holder, entry));
        }
        if save {
            self.save_boon();
        }
        if remove {
            self.remove_boon();
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn boon_draft_is_retained_and_stale_saves_and_player_navigation_are_guarded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("boon-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let holder = lib
            .create_character("Vampire", "Holder", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        let snapshot = app
            .library
            .as_ref()
            .unwrap()
            .boon_snapshot(&holder, Role::Storyteller)
            .unwrap();
        app.boons = BoonWindow::synthetic(snapshot);
        assert!(app.has_unsaved_edit());
        app.boons.open = false;
        app.language = Language::Portuguese;
        app.open_boons();
        assert_eq!(app.boons.edit.partner, "Synthetic partner");
        app.request_action(PendingAction::BoonEntry(holder.clone(), None));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        let mut writer = Library::open(&path).unwrap();
        writer
            .create_player("Concurrent", 1, Role::Storyteller)
            .unwrap();
        app.save_boon();
        assert!(app.boons.dirty());
        assert!(
            app.boons
                .message
                .as_ref()
                .unwrap()
                .contains("STALE_REVISION")
        );
        assert!(
            writer
                .boon_snapshot(&holder, Role::Storyteller)
                .unwrap()
                .entries
                .is_empty()
        );
        app.clear_purchases();
        app.load_boon_entry(holder.clone(), None);
        app.boons.edit =
            BoonWindow::synthetic(writer.boon_snapshot(&holder, Role::Storyteller).unwrap()).edit;
        app.save_boon();
        assert!(!app.boons.dirty());
        assert!(app.boons.selected_id().is_some());
        let id = app.boons.selected_id().unwrap().to_owned();
        app.boons.edit.description = "Changed draft".into();
        app.boons.open = false;
        app.open_boons();
        assert_eq!(app.boons.selected_id(), Some(id.as_str()));
        assert_eq!(app.boons.edit.description, "Changed draft");
        app.change_route(WorkArea::Player);
        assert!(!app.boons.open);
        assert_eq!(app.boons.edit, BoonEdit::default());
    }
}
