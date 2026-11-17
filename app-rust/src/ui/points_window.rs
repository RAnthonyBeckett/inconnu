use super::*;
use grapevine_workbench::player_points::{Baseline, Command, Kind};
impl InconnuApp {
    pub(super) fn open_points(&mut self) {
        if self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        self.points_open = true;
        if self.points_form.selected_player.is_none()
            && let Some(player) = self.summaries.iter().find(|r| r.kind == "Player")
        {
            self.select_points(player.id.clone());
        }
    }
    pub(super) fn select_points(&mut self, id: String) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        match lib.player_points(&id, Role::Storyteller) {
            Ok(ledger) => {
                self.points_form = Default::default();
                self.points_form.selected_player = Some(id.clone());
                self.points_target = Some(id);
                self.points_ledger = ledger;
                self.points_revision = lib.revision().ok();
                self.points_saved_entry = self.points_form.entry_signature();
                self.points_open = true;
            }
            Err(e) => self.log_error("player_points_read", &e),
        }
    }
    pub(super) fn create_points_player(&mut self) {
        if self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }

        let saved = if self.points_saved_entry.is_null() {
            crate::points_ui::Form::default().entry_signature()
        } else {
            self.points_saved_entry.clone()
        };
        if self.points_form.entry_signature() != saved {
            self.points_form.message = Some(
                self.t(
                    "Finish or refresh the entry draft before creating another Player.",
                    "Conclua ou atualize o rascunho antes de criar outro Jogador.",
                )
                .into(),
            );
            return;
        }
        let name = self.points_form.name.clone();
        let revision = self.library.as_ref().unwrap().revision().unwrap_or(-1);
        match self
            .library
            .as_mut()
            .unwrap()
            .create_player(&name, revision, Role::Storyteller)
        {
            Ok(id) => {
                self.points_form = Default::default();
                self.revision = self.library.as_ref().unwrap().revision().ok();
                self.refresh_summaries(true);
                self.select_points(id);
            }
            Err(e) => self.points_form.message = Some(e.message),
        }
    }
    pub(super) fn render_points(&mut self, ctx: &egui::Context) {
        if !self.points_open || self.route != WorkArea::Storyteller {
            return;
        }
        if self.library.is_none() {
            return;
        }
        let players: Vec<_> = self
            .summaries
            .iter()
            .filter(|s| s.kind == "Player" && s.build.is_none())
            .cloned()
            .collect();
        let mut open = true;
        let mut action = None;
        egui::Window::new(self.t("Players and Player points", "Jogadores e pontos do Jogador"))
            .id(egui::Id::unique("points_window"))
            .default_size([780.0, 620.0])
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("points_scroll")
                    .show(ui, |ui| {
                        action = crate::points_ui::render(
                            ui,
                            &mut self.points_form,
                            &players,
                            self.points_ledger.as_ref(),
                            self.language,
                        );
                    });
            });
        self.points_open = open;
        match action {
            Some(crate::points_ui::Action::Select(id)) => {
                self.points_form.selected_player = self.points_target.clone();
                self.request_action(PendingAction::SelectPoints(id));
            }
            Some(crate::points_ui::Action::Refresh) => {
                if let Some(id) = self.points_form.selected_player.clone() {
                    self.request_action(PendingAction::SelectPoints(id));
                }
            }
            Some(crate::points_ui::Action::Create) => self.create_points_player(),
            Some(crate::points_ui::Action::Open) | Some(crate::points_ui::Action::Post) => {
                if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
                    self.points_form.message = Some(
                        self.t(
                            "Save or discard the sheet draft first.",
                            "Salve ou descarte o rascunho da ficha primeiro.",
                        )
                        .into(),
                    );
                    return;
                }
                let Some(id) = self.points_target.clone() else {
                    return;
                };
                let Some(expected) = self.points_revision else {
                    return;
                };
                let f = &self.points_form;
                let result = if matches!(action, Some(crate::points_ui::Action::Open)) {
                    self.library.as_mut().unwrap().initialize_player_points(
                        &id,
                        &Baseline {
                            day: f.day.clone(),
                            balance: f.opening_balance.clone(),
                            actor: f.actor.clone(),
                            reason: f.reason.clone(),
                            evidence: f.evidence.clone(),
                        },
                        expected,
                        Role::Storyteller,
                    )
                } else {
                    self.library.as_mut().unwrap().post_player_points(
                        &id,
                        &Command {
                            id: f.command_id.clone(),
                            kind: f.kind,
                            amount: if f.kind == Kind::Reversal {
                                String::new()
                            } else {
                                f.amount.clone()
                            },
                            day: f.day.clone(),
                            actor: f.actor.clone(),
                            reason: f.reason.clone(),
                            evidence: f.evidence.clone(),
                            reversal_of: (f.kind == Kind::Reversal).then(|| f.reversal_id.clone()),
                        },
                        expected,
                        Role::Storyteller,
                    )
                };
                match result {
                    Ok(revision) => {
                        self.revision = Some(revision);
                        self.points_revision = Some(revision);
                        self.points_ledger = self
                            .library
                            .as_ref()
                            .unwrap()
                            .player_points(&id, Role::Storyteller)
                            .unwrap_or(None);
                        self.points_form.amount.clear();
                        self.points_form.opening_balance.clear();
                        self.points_form.reversal_id.clear();
                        self.points_form.command_id = uuid::Uuid::new_v4().to_string();
                        self.points_form.reason.clear();
                        self.points_form.evidence.clear();
                        self.points_form.dirty = false;
                        self.points_saved_entry = self.points_form.entry_signature();
                        self.points_form.message = Some(
                            self.t(
                                "Player points saved. Character XP is unchanged.",
                                "Pontos do Jogador salvos. O XP dos personagens não foi alterado.",
                            )
                            .into(),
                        );
                    }
                    Err(e) => self.points_form.message = Some(e.message),
                }
            }
            None => {}
        }
    }
}
