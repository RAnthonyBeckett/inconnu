use super::*;
use grapevine_workbench::legacy_profile as legacy;
#[derive(Default)]
pub(super) struct LegacyWindow {
    pub open: bool,
    pub sources: Vec<legacy::Source>,
    pub context: Option<legacy::Context>,
    pub request: legacy::Request,
    pub quote: Option<legacy::Quote>,
    pub dirty: bool,
    pub message: Option<String>,
}
fn t(pt: bool, en: &str, br: &str) -> String {
    if pt { br.into() } else { en.into() }
}
fn label(field: legacy::Field, pt: bool) -> String {
    use legacy::Field::*;
    match field {
        Title => t(pt, "Chronicle title", "Título da crônica"),
        Website => t(pt, "Website", "Site"),
        Email => t(pt, "Email", "Email"),
        Phone => t(pt, "Phone", "Telefone"),
        MeetingPlace => t(pt, "Meeting place", "Local de encontro"),
        MeetingTime => t(pt, "Meeting time", "Horário de encontro"),
        Description => t(pt, "Description", "Descrição"),
    }
}
impl InconnuApp {
    pub(super) fn open_legacy_profile(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.chronicle.dirty() {
            self.chronicle.message = Some(
                self.t(
                    "Save or discard game-information edits first.",
                    "Salve ou descarte as edições das informações do jogo primeiro.",
                )
                .into(),
            );
            return;
        }
        self.chronicle.open = false;
        self.legacy_profile.open = true;
        if self.legacy_profile.context.is_some() {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        match lib.legacy_profile_sources(Role::Storyteller) {
            Ok(sources) => self.legacy_profile.sources = sources,
            Err(e) => {
                self.legacy_profile.message = Some(e.message);
                return;
            }
        }
        if let Some(source) = self.legacy_profile.sources.first() {
            self.load_legacy_profile(source.hash.clone());
        }
    }
    fn load_legacy_profile(&mut self, source: String) {
        self.legacy_profile.request.source_hash = source.clone();
        self.legacy_profile.request.selected.clear();
        self.legacy_profile.request.id = uuid::Uuid::new_v4().to_string();
        self.legacy_profile.request.confirmed = false;
        self.legacy_profile.request.confirm_overwrite = false;
        self.legacy_profile.request.confirm_publication = false;
        self.legacy_profile.quote = None;
        self.legacy_profile.dirty = false;
        let Some(lib) = &self.library else {
            return;
        };
        match lib.legacy_profile_context(&source, Role::Storyteller) {
            Ok(c) => {
                self.legacy_profile.context = Some(c);
                self.legacy_profile.message = None;
            }
            Err(e) => {
                self.legacy_profile.context = None;
                self.legacy_profile.message = Some(e.message);
            }
        }
    }
    pub(super) fn legacy_profile_action(&mut self, action: u8) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if action == 0 {
            let source = self.legacy_profile.request.source_hash.clone();
            self.legacy_profile.request = Default::default();
            self.load_legacy_profile(source);
            return;
        }
        // Exclude only this form from the established global unsaved-draft guard.
        let own = std::mem::take(&mut self.legacy_profile.dirty);
        let blocked = self.has_unsaved_edit();
        self.legacy_profile.dirty = own;
        if blocked {
            self.legacy_profile.message = Some(
                self.t(
                    "Save or discard other pending edits first.",
                    "Salve ou descarte outras edições pendentes primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(context)) = (&mut self.library, &self.legacy_profile.context) else {
            return;
        };
        if action == 1 {
            self.legacy_profile.quote = None;
            match lib.quote_legacy_profile(
                &self.legacy_profile.request,
                context.revision,
                Role::Storyteller,
            ) {
                Ok(q) => {
                    self.legacy_profile.quote = Some(q);
                    self.legacy_profile.message = None;
                }
                Err(e) => self.legacy_profile.message = Some(e.message),
            }
        } else if action == 2
            && let Some(q) = self.legacy_profile.quote.clone()
        {
            if q.request != self.legacy_profile.request {
                return;
            }
            match lib.commit_legacy_profile(&q, Role::Storyteller) {
                Ok(_) => {
                    self.legacy_profile.dirty = false;
                    self.revision = lib.revision().ok();
                    self.chronicle = Default::default();
                    self.refresh_summaries(true);
                    self.legacy_profile.request = Default::default();
                    self.load_legacy_profile(q.request.source_hash);
                    self.legacy_profile.message=Some(self.t("Selected game details saved. Publication choices, rules and character data are unchanged.","Detalhes selecionados do jogo salvos. Opções de publicação, regras e dados dos personagens não mudaram.").into());
                }
                Err(e) => self.legacy_profile.message = Some(e.message),
            }
        }
    }
    pub(super) fn render_legacy_profile(&mut self, ctx: &egui::Context) {
        if self.route != WorkArea::Storyteller {
            self.legacy_profile = Default::default();
            return;
        }
        if !self.legacy_profile.open {
            return;
        }
        let pt = self.language == Language::Portuguese;
        let revision = self.library.as_ref().and_then(|l| l.revision().ok());
        let mut action = None;
        let mut source = None;
        let mut open = true;
        let state = &mut self.legacy_profile;
        egui::Window::new(t(pt,"Review legacy game details","Revisar detalhes legados do jogo")).id(egui::Id::unique("legacy_profile_window")).open(&mut open).default_width(760.0).default_height(620.0).show(ctx,|ui|{
            ui.horizontal_wrapped(|ui|{
                let b=ui.button(t(pt,"Preview selected changes","Visualizar alterações selecionadas"));remember(ui,&b,"legacy_profile_prepare");if b.clicked(){action=Some(1);}
                let fresh=state.quote.as_ref().is_some_and(|q|Some(q.revision)==revision&&q.request==state.request);
                let b=ui.add_enabled(fresh,egui::Button::new(t(pt,"Apply reviewed details","Aplicar detalhes revisados")));remember(ui,&b,"legacy_profile_apply");if b.clicked(){action=Some(2);}
                if ui.button(t(pt,"Reload / discard draft","Recarregar / descartar rascunho")).clicked(){action=Some(0);}
            });
            if let Some(message)=&state.message{ui.label(message);}
            egui::ScrollArea::vertical().show(ui,|ui|{
                ui.label(t(pt,"Select a retained import. Changing source clears field choices and preview. Original files stay unchanged; behavior, calendar and menu settings are not mapped here.","Selecione uma importação retida. Mudar a fonte limpa as escolhas e a prévia. Arquivos originais não mudam; configurações de comportamento, calendário e menus não são vinculadas aqui."));
                if state.sources.is_empty(){ui.label(t(pt,"Import a legacy game first; no retained sources are available.","Importe um jogo legado primeiro; não há fontes retidas disponíveis."));}
                egui::ComboBox::from_id_salt("legacy_profile_source").selected_text(state.context.as_ref().map(|c|c.source_name.as_str()).unwrap_or("—")).show_ui(ui,|ui|{
                    for s in &state.sources{if ui.selectable_label(state.request.source_hash==s.hash,format!("{} · {}",s.name,s.available_fields)).clicked(){source=Some(s.hash.clone());}
if let Some(issue)=&s.issue{ui.weak(issue);}}
                });
                let Some(context)=&state.context else{return;};
                ui.label(t(pt,"Keep current is the default for every field. Only checked replacements are included. Marked private source text is removed from proposed shared fields; the complete original remains in retained evidence.","Manter o atual é o padrão para cada campo. Somente substituições marcadas são incluídas. Texto privado marcado é removido dos campos compartilháveis propostos; o original completo permanece na evidência retida."));
                ui.label(format!("{}: {} · {}: {}",t(pt,"Public details","Detalhes públicos"),context.profile.publish_details,t(pt,"Public contacts","Contatos públicos"),context.profile.publish_contacts));
                let before=state.request.clone();
                for candidate in &context.fields {
                    ui.separator();let mut selected=state.request.selected.contains(&candidate.field);
                    if ui.add_enabled(candidate.proposed.is_some(),egui::Checkbox::new(&mut selected,format!("{}: {}",t(pt,"Use legacy value","Usar valor legado"),label(candidate.field,pt)))).changed(){if selected{state.request.selected.push(candidate.field);}else{state.request.selected.retain(|f|f!=&candidate.field);}}
                    ui.label(format!("{}: {}",t(pt,"Current","Atual"),candidate.current));
                    ui.label(format!("{}: {}",t(pt,"Proposed","Proposto"),candidate.proposed.as_deref().unwrap_or(if pt{"ausente ou formato não compatível"}else{"absent or unsupported shape"})));
                    if candidate.private_text_removed{ui.weak(t(pt,"Marked source text was removed; review the proposed value.","Texto marcado da fonte foi removido; revise o valor proposto."));}
                    if candidate.would_be_public{ui.weak(t(pt,"A replacement would be public under the current settings.","Uma substituição seria pública com as configurações atuais."));}
                }
                for (en,br,value) in [("Review date (YYYY-MM-DD)","Data da revisão (AAAA-MM-DD)",&mut state.request.day),("Staff name","Nome da pessoa da equipe",&mut state.request.actor),("Reason","Motivo",&mut state.request.reason),("Review reference","Referência da revisão",&mut state.request.review_reference)]{ui.label(t(pt,en,br));ui.text_edit_singleline(value);}
                ui.checkbox(&mut state.request.confirm_overwrite,t(pt,"I reviewed replacing or clearing existing values.","Revisei a substituição ou remoção de valores existentes."));
                ui.checkbox(&mut state.request.confirm_publication,t(pt,"I reviewed the selected values that would be public under current settings. Publication flags will not change.","Revisei os valores selecionados que seriam públicos com as configurações atuais. As opções de publicação não mudarão."));
                ui.checkbox(&mut state.request.confirmed,t(pt,"I reviewed these exact field choices and source values.","Revisei estas escolhas exatas de campos e valores da fonte."));
                if state.request!=before{state.quote=None;state.dirty=true;}
                if let Some(q)=&state.quote{ui.separator();ui.heading(t(pt,"Selected changes","Alterações selecionadas"));for change in &q.changes{ui.label(format!("{}: {} -> {}",label(change.field,pt),change.before,change.after));}}
                ui.separator();ui.heading(t(pt,"Private review history","Histórico privado de revisões"));for receipt in context.history.iter().rev(){ui.collapsing(format!("{} · {}",receipt.request.day,receipt.source_name),|ui|{ui.label(&receipt.request.actor);ui.label(&receipt.request.reason);ui.label(&receipt.request.review_reference);});}
            });
        });
        self.legacy_profile.open = open;
        if let Some(hash) = source {
            self.load_legacy_profile(hash);
        }
        if let Some(action) = action {
            self.legacy_profile_action(action);
        }
    }
}
fn remember(ui: &egui::Ui, b: &egui::Response, key: &str) {
    let rect = ui.clip_rect().contains_rect(b.rect).then_some(b.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), rect));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_profile_language_drafts_apply_and_player_privacy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy-profile-ui.gvlib");
        smoke_legacy_profile_fixture(&path);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_legacy_profile();
        app.legacy_profile.request = smoke_legacy_profile_request(
            app.legacy_profile
                .context
                .as_ref()
                .unwrap()
                .source_hash
                .clone(),
        );
        app.legacy_profile.dirty = true;
        app.legacy_profile_action(1);
        let q = app.legacy_profile.quote.clone().unwrap();
        let request = app.legacy_profile.request.clone();
        let ctx = egui::Context::default();
        for language in [Language::English, Language::Portuguese] {
            app.language = language;
            for _ in 0..2 {
                let mut output =
                    ctx.run_ui(Default::default(), |ui| app.render_legacy_profile(ui.ctx()));
                output.textures_delta.clear();
            }
            assert_eq!(app.legacy_profile.request, request);
            assert_eq!(app.legacy_profile.quote.as_ref(), Some(&q));
        }
        app.edit.name = "Unsaved".into();
        app.legacy_profile_action(2);
        assert!(
            app.legacy_profile
                .context
                .as_ref()
                .unwrap()
                .history
                .is_empty()
        );
        app.edit = app.saved_edit.clone();
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        app.legacy_profile_action(2);
        let context = app.legacy_profile.context.as_ref().unwrap();
        assert_eq!(context.history.len(), 1);
        assert_eq!(context.profile.title, "Synthetic legacy chronicle");
        assert!(!app.legacy_profile.dirty);
        let public = app
            .library
            .as_ref()
            .unwrap()
            .public_chronicle_profile()
            .unwrap();
        assert!(public.profile.email.is_empty());
        assert!(
            !serde_json::to_string(&public)
                .unwrap()
                .contains("PRIVATE_LEGACY")
        );
        app.change_route(WorkArea::Player);
        assert!(app.legacy_profile.context.is_none());
        assert!(app.legacy_profile.request.actor.is_empty());
    }
}
