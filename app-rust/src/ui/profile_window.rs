use super::*;
use grapevine_workbench::player_profiles::{Profile, Snapshot};
#[derive(Default)]
pub(super) struct ProfileWindow {
    pub open: bool,
    pub target: Option<String>,
    pub edit: Profile,
    saved: Profile,
    revision: Option<i64>,
    snapshot: Option<Snapshot>,
    pub message: Option<String>,
}

impl ProfileWindow {
    pub(super) fn dirty(&self) -> bool {
        self.edit != self.saved
    }
    pub(super) fn synthetic(revision: Option<i64>) -> Self {
        Self {
            open: true,
            revision,
            edit: Profile {
                name: "Synthetic Profile".into(),
                position: "Local volunteer".into(),
                status: "Active".into(),
                email: "PRIVATE_PROFILE_EMAIL".into(),
                phone: "PRIVATE_PROFILE_PHONE".into(),
                address: "PRIVATE_PROFILE_ADDRESS".into(),
                notes: "PRIVATE_PROFILE_NOTES".into(),
            },
            ..Default::default()
        }
    }
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
impl InconnuApp {
    pub(super) fn open_profiles(&mut self) {
        if self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        if self.profiles.revision.is_none() {
            let id = self
                .summaries
                .iter()
                .find(|s| s.kind == "Player" && s.build.is_none())
                .map(|s| s.id.clone());
            self.load_profile(id);
        } else {
            self.profiles.open = true;
        }
    }
    pub(super) fn load_profile(&mut self, id: Option<String>) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let revision = lib.revision().ok();
        let result = id
            .as_ref()
            .map(|id| lib.player_profile(id, Role::Storyteller))
            .transpose();
        match result {
            Ok(snapshot) => {
                let edit = snapshot
                    .as_ref()
                    .map_or_else(Profile::default, |s| s.profile.clone());
                self.profiles = ProfileWindow {
                    open: true,
                    target: id,
                    saved: edit.clone(),
                    edit,
                    revision: snapshot.as_ref().map(|s| s.revision).or(revision),
                    snapshot,
                    ..Default::default()
                };
            }
            Err(e) => self.log_error("player_profile.read", &e),
        }
    }
    fn save_profile(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.profiles.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(revision)) = (&mut self.library, self.profiles.revision) else {
            return;
        };
        let result = match &self.profiles.target {
            Some(id) => lib
                .save_player_profile(id, &self.profiles.edit, revision, Role::Storyteller)
                .map(|_| id.clone()),
            None => lib.create_player_profile(&self.profiles.edit, revision, Role::Storyteller),
        };
        match result {
            Ok(id) => {
                self.profiles.target = Some(id.clone());
                self.profiles.edit.name = self.profiles.edit.name.trim().into();
                self.profiles.saved = self.profiles.edit.clone();
                self.profiles.revision = Some(revision + 1);
                self.revision = Some(revision + 1);
                self.profiles.snapshot = lib.player_profile(&id, Role::Storyteller).ok();
                self.profiles.message = Some(
                    t(
                        self.language,
                        "Player profile saved.",
                        "Perfil do Jogador salvo.",
                    )
                    .into(),
                );
                self.refresh_summaries(true);
            }
            Err(e) => self.profiles.message = Some(format!("{}: {}", e.code, e.message)),
        }
    }
    pub(super) fn render_profiles(&mut self, ctx: &egui::Context) {
        if !self.profiles.open || self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        let l = self.language;
        let mut open = true;
        let mut select = None;
        let mut save = false;
        let mut points = None;
        let mut character = None;
        egui::Window::new(t(l, "Player profiles", "Perfis de Jogadores"))
            .id(egui::Id::unique("profile_window"))
            .default_size([700., 530.])
            .vscroll(true)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button(t(l, "New Player", "Novo Jogador")).clicked() {
                        select = Some(None);
                    }
                    let title = self
                        .summaries
                        .iter()
                        .find(|r| Some(&r.id) == self.profiles.target.as_ref())
                        .map_or(t(l, "Choose a Player", "Escolher Jogador"), |r| {
                            r.name.as_str()
                        });
                    egui::ComboBox::from_id_salt("profile_select")
                        .selected_text(title)
                        .show_ui(ui, |ui| {
                            for r in self
                                .summaries
                                .iter()
                                .filter(|r| r.kind == "Player" && r.build.is_none())
                            {
                                if ui
                                    .selectable_label(
                                        self.profiles.target.as_ref() == Some(&r.id),
                                        if self
                                            .summaries
                                            .iter()
                                            .filter(|other| {
                                                other.kind == "Player" && other.name == r.name
                                            })
                                            .count()
                                            > 1
                                        {
                                            format!(
                                                "{} · {}",
                                                r.name,
                                                r.id.chars().take(8).collect::<String>()
                                            )
                                        } else {
                                            r.name.clone()
                                        },
                                    )
                                    .clicked()
                                {
                                    select = Some(Some(r.id.clone()));
                                }
                            }
                        });
                    let b = ui.add_enabled(
                        self.profiles.dirty(),
                        egui::Button::new(t(l, "Save profile", "Salvar perfil")),
                    );
                    let rect = ui.clip_rect().contains_rect(b.rect).then_some(b.rect);
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::unique("profile_save"), rect));
                    save = b.clicked();
                    if let Some(id) = &self.profiles.target
                        && ui
                            .button(t(l, "Player points…", "Pontos do Jogador…"))
                            .clicked()
                    {
                        points = Some(id.clone());
                    }
                });
                if self.profiles.dirty() {
                    ui.label(t(l, "Unsaved changes", "Alterações não salvas"));
                }
                if let Some(message) = &self.profiles.message {
                    ui.label(message);
                }
                ui.label(t(
                    l,
                    "Position and status describe the saved profile.",
                    "Função e situação descrevem o perfil salvo.",
                ));
                let p = &mut self.profiles.edit;
                for (en, pt, value) in [
                    ("Name", "Nome", &mut p.name),
                    ("Position", "Função", &mut p.position),
                    ("Status", "Situação", &mut p.status),
                    ("Email", "E-mail", &mut p.email),
                    ("Phone", "Telefone", &mut p.phone),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(t(l, en, pt));
                        ui.add(egui::TextEdit::singleline(value).desired_width(350.));
                    });
                }
                ui.label(t(l, "Private address", "Endereço privado"));
                ui.add(
                    egui::TextEdit::multiline(&mut p.address)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.label(t(l, "Private notes", "Notas privadas"));
                ui.add(
                    egui::TextEdit::multiline(&mut p.notes)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                if let Some(snapshot) = &self.profiles.snapshot {
                    if let Some(id) = &snapshot.legacy_id {
                        ui.label(format!(
                            "{}: {id}",
                            t(
                                l,
                                "Retained legacy identifier",
                                "Identificador legado preservado"
                            )
                        ));
                    }
                    ui.separator();
                    ui.heading(t(
                        l,
                        "Explicitly linked characters",
                        "Personagens com vínculo explícito",
                    ));
                    for (id, name) in &snapshot.linked_characters {
                        if ui.button(name).clicked() {
                            character = Some(id.clone());
                        }
                    }
                }
            });
        self.profiles.open = open;
        if let Some(id) = select {
            self.request_action(PendingAction::PlayerProfile(id));
        }
        if save {
            self.save_profile();
        }
        if let Some(id) = points {
            self.request_action(PendingAction::SelectPoints(id));
        }
        if let Some(id) = character {
            self.request_action(PendingAction::SelectRecord(id));
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn profile_draft_survives_close_language_and_cancel_and_stale_save_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let id = lib.create_player("Original", 0, Role::Storyteller).unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.load_profile(Some(id.clone()));
        app.profiles.edit.email = "PRIVATE_DRAFT".into();
        assert!(app.has_unsaved_edit());
        app.profiles.open = false;
        app.language = Language::Portuguese;
        app.open_profiles();
        assert_eq!(app.profiles.edit.email, "PRIVATE_DRAFT");
        app.request_action(PendingAction::PlayerProfile(None));
        assert!(matches!(
            app.pending_action,
            Some(PendingAction::PlayerProfile(None))
        ));
        app.pending_action = None;
        assert_eq!(app.profiles.target.as_deref(), Some(id.as_str()));
        assert_eq!(app.profiles.edit.email, "PRIVATE_DRAFT");
        let mut writer = Library::open(&path).unwrap();
        writer
            .create_player("Other writer", 1, Role::Storyteller)
            .unwrap();
        app.save_profile();
        assert!(app.profiles.dirty());
        assert!(
            app.profiles
                .message
                .as_ref()
                .unwrap()
                .contains("STALE_REVISION")
        );
        assert_eq!(
            writer
                .player_profile(&id, Role::Storyteller)
                .unwrap()
                .profile
                .email,
            ""
        );
        app.clear_purchases();
        app.load_profile(Some(id));
        app.profiles.edit.phone = "PRIVATE_SAVED".into();
        app.save_profile();
        assert!(!app.profiles.dirty());
        assert_eq!(app.revision, Some(3));
        app.change_route(WorkArea::Player);
        assert!(!app.profiles.open);
        assert_eq!(app.profiles.edit, Profile::default());
    }
}
