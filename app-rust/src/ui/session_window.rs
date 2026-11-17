use super::*;
impl InconnuApp {
    pub(super) fn new_session(&mut self) {
        self.session_form = crate::session_ui::Form::from_roster(&self.summaries);
        self.open_sessions();
    }
    pub(super) fn open_sessions(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let snapshot_revision = lib.revision().unwrap_or(-1);
        match lib.session_receipts(Role::Storyteller) {
            Ok(receipts) => {
                self.session_form.receipts = receipts;
                match lib.scheduled_sessions(Role::Storyteller) {
                    Ok(schedules) => self.session_form.schedules = schedules,
                    Err(e) => {
                        self.log_error("calendar_read", &e);
                        return;
                    }
                }
                if lib.revision().ok() != Some(snapshot_revision) {
                    self.session_form.message =
                        Some("Library changed while reading the calendar; refresh history.".into());
                    return;
                }
                if self.session_form.revision.is_none() {
                    self.session_form.revision = Some(snapshot_revision);
                }
                self.session_open = true;
            }
            Err(e) => self.log_error("sessions_open", &e),
        }
    }
    pub(super) fn load_scheduled_session(
        &mut self,
        request: grapevine_workbench::game_sessions::SessionRequest,
    ) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let revision = lib.revision().unwrap_or(-1);
        let fresh = lib
            .scheduled_sessions(Role::Storyteller)
            .ok()
            .and_then(|events| events.into_iter().find(|e| e.request.id == request.id));
        if fresh
            .as_ref()
            .is_none_or(|e| e.cancelled || e.request != request)
            || lib.revision().ok() != Some(revision)
        {
            self.session_form.message =
                Some("Scheduled event changed; refresh history before loading.".into());
            return;
        }
        let mut form = crate::session_ui::Form::from_roster(&self.summaries);
        form.revision = Some(revision);
        for row in &request.attendance {
            if let Some(uirow) = form.rows.get_mut(&row.record_id) {
                uirow.included = true;
                uirow.present = row.present;
                uirow.award_enabled = row.award.is_some();
                uirow.amount = row.award.clone().unwrap_or_default();
            }
        }
        match lib.legacy_calendar_origin(&request.id, Role::Storyteller) {
            Ok(origin) => form.legacy_origin = origin,
            Err(e) => {
                form.message = Some(format!(
                    "{}: {}",
                    self.t(
                        "Plan loaded; retained source details are unavailable",
                        "Plano carregado; detalhes retidos da fonte indisponíveis"
                    ),
                    e.message
                ))
            }
        }
        form.request = request;
        self.session_form = form;
        self.open_sessions();
    }
    pub(super) fn render_sessions(&mut self, ctx: &egui::Context) {
        if !self.session_open || self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let revision = lib.revision().unwrap_or(-1);
        let mut open = true;
        let mut action = None;
        egui::Window::new(self.t("Game sessions", "Sessões de jogo"))
            .id(egui::Id::unique("sessions_window"))
            .default_size([820.0, 640.0])
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("session_scroll")
                    .show(ui, |ui| {
                        action = crate::session_ui::render(
                            ui,
                            &mut self.session_form,
                            &self.summaries,
                            revision,
                            self.language,
                        );
                    });
            });
        self.session_open = open;
        match action {
            Some(crate::session_ui::Action::SaveSchedule) => {
                match self.library.as_mut().unwrap().save_scheduled_session(
                    &self.session_form.request,
                    self.session_form.revision.unwrap_or(-1),
                    Role::Storyteller,
                ) {
                    Ok(revision) => {
                        self.revision = Some(revision);
                        self.session_form.dirty = false;
                        self.session_form.quote = None;
                        self.session_form.revision = Some(revision);
                        self.open_sessions();
                        self.session_form.message = Some(
                            self.t(
                                "Scheduled event saved. No XP was awarded.",
                                "Evento agendado salvo. Nenhum XP foi concedido.",
                            )
                            .into(),
                        );
                    }
                    Err(e) => self.session_form.message = Some(e.message),
                }
            }
            Some(crate::session_ui::Action::LoadSchedule(index)) => {
                if let Some(event) = self.session_form.schedules.get(index) {
                    self.request_action(PendingAction::LoadSession(event.request.clone()));
                }
            }
            Some(crate::session_ui::Action::CancelSchedule(index)) => {
                if let Some(event) = self.session_form.schedules.get(index) {
                    match self.library.as_mut().unwrap().cancel_scheduled_session(
                        &event.request.id,
                        revision,
                        Role::Storyteller,
                    ) {
                        Ok(revision) => {
                            self.revision = Some(revision);
                            self.session_form.quote = None;
                            self.open_sessions();
                            self.session_form.message = Some(
                                self.t(
                                    "Scheduled event cancelled; its record is retained.",
                                    "Evento agendado cancelado; o registro foi preservado.",
                                )
                                .into(),
                            );
                        }
                        Err(e) => self.session_form.message = Some(e.message),
                    }
                }
            }

            Some(crate::session_ui::Action::Preview) => {
                if self.edit != self.saved_edit
                    || self.sheet_edit != self.saved_sheet
                    || self.xp_form != self.xp_saved_form
                    || self.purchase_form.dirty
                {
                    self.session_form.message=Some(self.t("Save or discard other sheet/XP drafts before previewing session awards.","Salve ou descarte outros rascunhos de ficha/XP antes de simular os prêmios da sessão.").into());
                    return;
                }
                match lib.quote_session(
                    &self.session_form.request,
                    self.session_form.revision.unwrap_or(-1),
                    Role::Storyteller,
                ) {
                    Ok(quote) => {
                        self.session_form.quote = Some(quote);
                        self.session_form.message = None;
                    }
                    Err(e) => {
                        self.session_form.quote = None;
                        self.session_form.message = Some(e.message);
                    }
                }
            }
            Some(crate::session_ui::Action::Commit) => {
                let Some(quote) = self.session_form.quote.clone() else {
                    return;
                };
                if quote.request != self.session_form.request {
                    return;
                }
                if self.edit != self.saved_edit
                    || self.sheet_edit != self.saved_sheet
                    || self.xp_form != self.xp_saved_form
                    || self.purchase_form.dirty
                {
                    self.session_form.message = Some(
                        self.t(
                            "Save or discard other drafts before committing.",
                            "Salve ou descarte outros rascunhos antes de gravar.",
                        )
                        .into(),
                    );
                    return;
                }
                match self
                    .library
                    .as_mut()
                    .unwrap()
                    .commit_session(&quote, Role::Storyteller)
                {
                    Ok(receipt) => {
                        self.revision = Some(receipt.committed_revision);
                        self.session_form.revision = Some(receipt.committed_revision);
                        self.session_form.quote = None;
                        self.session_form.dirty = false;
                        self.session_form.schedules = self
                            .library
                            .as_ref()
                            .unwrap()
                            .scheduled_sessions(Role::Storyteller)
                            .unwrap_or_default();
                        self.session_form.receipts = self
                            .library
                            .as_ref()
                            .unwrap()
                            .session_receipts(Role::Storyteller)
                            .unwrap_or_default();
                        self.session_form.message=Some(self.t("Session saved. All selected awards were committed together.","Sessão salva. Todos os prêmios selecionados foram gravados juntos.").into());
                        if let Some(id) = self.selected_id.clone() {
                            self.load_record(&id, false);
                        }
                        self.xp_view = None;
                        self.xp_roster = None;
                        self.xp_window_open = false;
                        self.clear_scenarios();
                        self.purchase_form = Default::default();
                        self.purchase_open = false;
                    }
                    Err(e) => {
                        self.session_form.quote = None;
                        self.session_form.message = Some(e.message);
                    }
                }
            }
            Some(crate::session_ui::Action::New) => self.request_action(PendingAction::NewSession),
            Some(crate::session_ui::Action::RefreshHistory) => self.open_sessions(),
            Some(crate::session_ui::Action::ExportReceipt(index)) => {
                let Some(receipt) = self.session_form.receipts.get(index) else {
                    return;
                };
                let id = receipt.quote.request.id.clone();
                let report =
                    lib.session_report(&id, self.session_form.document_language, Role::Storyteller);
                match report {
                    Ok(html) => {
                        if let Some(path) = FileDialog::new()
                            .set_file_name("Session receipt.html")
                            .add_filter("Printable HTML", &["html"])
                            .save_file()
                        {
                            // Read the retained receipt again after the dialog, including after a restore.
                            if lib
                                .session_report(
                                    &id,
                                    self.session_form.document_language,
                                    Role::Storyteller,
                                )
                                .ok()
                                .as_ref()
                                != Some(&html)
                            {
                                self.session_form.message =
                                    Some("Receipt changed; refresh history.".into());
                                return;
                            }
                            self.session_form.message=Some(match grapevine_workbench::safe_files::write_new(&path,html.as_bytes()) {Ok(())=>self.t("Receipt saved; open it in a browser to print or save PDF.","Recibo salvo; abra no navegador para imprimir ou salvar PDF.").into(),Err(e)=>e.message});
                        }
                    }
                    Err(e) => self.session_form.message = Some(e.message),
                }
            }
            None => {}
        }
    }
}
