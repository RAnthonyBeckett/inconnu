use super::*;
use grapevine_workbench::character_lifecycle as life;
#[derive(Default)]
pub(super) struct LifecycleWindow {
    pub open: bool,
    pub target: Option<String>,
    pub context: Option<life::Context>,
    pub request: life::Request,
    pub quote: Option<life::Quote>,
    pub dirty: bool,
    pub message: Option<String>,
}
fn t(pt: bool, en: &str, br: &str) -> String {
    if pt { br.into() } else { en.into() }
}
impl InconnuApp {
    pub(super) fn open_character_lifecycle(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if self.lifecycle.target.as_ref() != Some(&id) {
            self.lifecycle = LifecycleWindow {
                open: true,
                target: Some(id),
                ..Default::default()
            };
            self.reload_character_lifecycle();
        } else {
            self.lifecycle.open = true;
        }
    }
    fn reload_character_lifecycle(&mut self) {
        let state = &mut self.lifecycle;
        let Some(id) = &state.target else {
            return;
        };
        let result = self
            .library
            .as_ref()
            .map(|lib| lib.character_lifecycle_context(id, Role::Storyteller));
        state.request = Default::default();
        state.quote = None;
        state.dirty = false;
        match result {
            Some(Ok(c)) => {
                state.context = Some(c);
                state.message = None;
            }
            Some(Err(e)) => {
                state.context = None;
                state.message = Some(e.message);
            }
            None => state.context = None,
        }
    }
    fn character_lifecycle_action(&mut self, action: u8) {
        if self.route != WorkArea::Storyteller || self.lifecycle.target != self.selected_id {
            return;
        }
        if action == 0 {
            self.reload_character_lifecycle();
            return;
        }
        if self.edit != self.saved_edit
            || self.sheet_edit != self.saved_sheet
            || (self.xp_form.is_dirty() && self.xp_form != self.xp_saved_form)
            || self.creation.dirty
            || self.met_reference.dirty
            || self.ordinary.dirty
            || self.purchase_form.dirty
        {
            self.lifecycle.message = Some(
                self.t(
                    "Save or discard other character/XP drafts first.",
                    "Salve ou descarte outros rascunhos de personagem/XP primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(id), Some(context), Some(lib)) = (
            &self.lifecycle.target,
            &self.lifecycle.context,
            &mut self.library,
        ) else {
            return;
        };
        if action == 1 {
            self.lifecycle.quote = None;
            match lib.quote_character_lifecycle(
                id,
                &self.lifecycle.request,
                context.revision,
                Role::Storyteller,
            ) {
                Ok(q) => {
                    self.lifecycle.quote = Some(q);
                    self.lifecycle.message = None;
                }
                Err(e) => self.lifecycle.message = Some(e.message),
            }
        } else if action == 2 {
            let Some(q) = self.lifecycle.quote.clone() else {
                return;
            };
            if q.request != self.lifecycle.request || q.revision != context.revision {
                self.lifecycle.message = Some(
                    self.t(
                        "Preview the current decision again.",
                        "Visualize a decisão atual novamente.",
                    )
                    .into(),
                );
                return;
            }
            match lib.commit_character_lifecycle(&q, Role::Storyteller) {
                Ok(_) => {
                    self.lifecycle.dirty = false;
                    self.revision = self.library.as_ref().and_then(|l| l.revision().ok());
                    self.refresh_summaries(true);
                    self.reload_character_lifecycle();
                    self.lifecycle.message = Some(
                        self.t(
                            "Decision saved. Traits and XP are unchanged.",
                            "Decisão salva. Características e XP não mudaram.",
                        )
                        .into(),
                    );
                }
                Err(e) => self.lifecycle.message = Some(e.message),
            }
        }
    }
    pub(super) fn render_character_lifecycle(&mut self, ctx: &egui::Context) {
        if self.route != WorkArea::Storyteller || self.lifecycle.target != self.selected_id {
            self.lifecycle = Default::default();
            return;
        }
        if !self.lifecycle.open {
            return;
        }
        let pt = self.language == Language::Portuguese;
        let live_revision = self.library.as_ref().and_then(|l| l.revision().ok());
        let mut open = true;
        let mut action = None;
        let state = &mut self.lifecycle;
        egui::Window::new(t(pt,"Entering play / retirement","Entrada em jogo / aposentadoria"))
            .id(egui::Id::unique("character_lifecycle_window"))
            .open(&mut open).default_width(720.0).default_height(580.0).show(ctx,|ui|{
                ui.horizontal_wrapped(|ui|{
                    let preview=ui.button(t(pt,"Preview decision","Visualizar decisão"));
                    remember(ui,&preview,"lifecycle_preview");if preview.clicked(){action=Some(1);}
                    let fresh=state.quote.as_ref().is_some_and(|q|q.request==state.request && Some(q.revision)==live_revision);
                    let save=ui.add_enabled(fresh,egui::Button::new(t(pt,"Save reviewed decision","Salvar decisão revisada")));
                    remember(ui,&save,"lifecycle_commit");if save.clicked(){action=Some(2);}
                    if ui.button(t(pt,"Reload / discard draft","Recarregar / descartar rascunho")).clicked(){action=Some(0);}
                });
                if let Some(message)=&state.message{ui.label(message);}
                egui::ScrollArea::vertical().show(ui,|ui|{
                    let Some(context)=&state.context else{ui.label(t(pt,"No original Vampire selected.","Nenhum Vampiro original selecionado."));return;};
                    ui.heading(&context.name);
                    ui.label(format!("{}: {}",t(pt,"Current status","Situação atual"),context.status));
                    if context.status_drift{ui.colored_label(egui::Color32::YELLOW,t(pt,"Status changed outside the last recorded decision. That review is historical.","A situação mudou fora da última decisão registrada. Essa revisão é histórica."));}
                    ui.label(t(pt,"This is a local staff decision. It does not grant traits, change XP, decide resurrection or establish organizational approval.","Esta é uma decisão da equipe local. Não concede características, não muda XP, não decide ressurreição nem estabelece aprovação organizacional."));
                    let before=state.request.clone();
                    ui.horizontal_wrapped(|ui|{
                        ui.selectable_value(&mut state.request.action,life::Action::Activate,t(pt,"Enter / return to active play","Entrar / voltar ao jogo ativo"));
                        ui.selectable_value(&mut state.request.action,life::Action::Retire,t(pt,"Retire; preserve character","Aposentar; preservar personagem"));
                    });
                    ui.label(if context.created_in_app {t(pt,"Created here: creation review and explicit XP opening are required for active play.","Criado aqui: revisão de criação e abertura explícita de XP são necessárias para jogar.")} else {t(pt,"Imported: original creation eligibility must be reviewed by staff.","Importado: a elegibilidade da criação original deve ser revisada pela equipe.")});
                    if !context.has_xp_opening{ui.weak(t(pt,"XP opening has not been recorded. Retirement remains available.","A abertura de XP não foi registrada. A aposentadoria continua disponível."));}
                    for (en,br,value) in [
                        ("Review date (YYYY-MM-DD)","Data da revisão (AAAA-MM-DD)",&mut state.request.day),
                        ("Staff name","Nome da pessoa da equipe",&mut state.request.actor),
                        ("Reason","Motivo",&mut state.request.reason),
                        ("Review / eligibility / XP evidence reference","Referência da revisão / elegibilidade / evidência de XP",&mut state.request.review_reference),
                    ] {ui.label(t(pt,en,br));ui.text_edit_singleline(value);}
                    if state.request.action==life::Action::Activate {
                        ui.checkbox(&mut state.request.confirm_legacy,t(pt,"I reviewed the imported character or existing custom status; no original creation approval is inferred.","Revisei o personagem importado ou a situação personalizada existente; nenhuma aprovação original é presumida."));
                        ui.checkbox(&mut state.request.confirm_return,t(pt,"I explicitly reviewed returning this retired character to play.","Revisei explicitamente o retorno deste personagem aposentado ao jogo."));
                        ui.checkbox(&mut state.request.confirm_post_creation,t(pt,"The initial sheet changed: I reviewed all later advancement and its XP evidence.","A ficha inicial mudou: revisei todos os avanços posteriores e suas evidências de XP."));
                    }
                    ui.checkbox(&mut state.request.confirmed,t(pt,"I reviewed this exact status decision, eligibility and XP evidence. Traits and XP must remain unchanged.","Revisei esta decisão exata de situação, elegibilidade e evidências de XP. Características e XP devem permanecer iguais."));
                    if state.request.action!=before.action {
                        state.request.confirmed=false;state.request.confirm_legacy=false;state.request.confirm_return=false;state.request.confirm_post_creation=false;
                    }
                    if state.request!=before {state.quote=None;state.dirty=true;}
                    if let Some(q)=&state.quote {
                        ui.separator();ui.strong(format!("{}: {} | {}: {}",t(pt,"Before","Antes"),q.before,t(pt,"After","Depois"),q.after));
                        ui.label(format!("{} · {}",q.request.day,q.request.actor));ui.label(&q.request.reason);ui.label(&q.request.review_reference);
                        ui.label(t(pt,"Preview only until saved. Character identity, traits, source data and XP are preserved.","Somente visualização até salvar. Identidade, características, dados de origem e XP são preservados."));
                    }
                    ui.separator();ui.heading(t(pt,"Private decision history","Histórico privado de decisões"));
                    for receipt in context.history.iter().rev() {
                        ui.collapsing(format!("{} · {} / {}",receipt.quote.request.day,receipt.quote.before,receipt.quote.after),|ui|{
                            ui.label(&receipt.quote.request.actor);ui.label(&receipt.quote.request.reason);ui.label(&receipt.quote.request.review_reference);
                        });
                    }
                });
            });
        self.lifecycle.open = open;
        if let Some(action) = action {
            self.character_lifecycle_action(action);
        }
    }
}
fn remember(ui: &egui::Ui, response: &egui::Response, key: &str) {
    let rect = ui
        .clip_rect()
        .contains_rect(response.rect)
        .then_some(response.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), rect));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_language_unsaved_guard_and_status_refresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lifecycle-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        lib.create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_character_lifecycle();
        app.lifecycle.request = life::Request {
            action: life::Action::Retire,
            day: "2026-11-14".into(),
            actor: "PRIVATE_LIFE_UI".into(),
            reason: "Synthetic".into(),
            review_reference: "Synthetic staff decision".into(),
            confirmed: true,
            ..Default::default()
        };
        app.lifecycle.dirty = true;
        app.character_lifecycle_action(1);
        let quote = app.lifecycle.quote.clone().unwrap();
        let request = app.lifecycle.request.clone();
        let ctx = egui::Context::default();
        for language in [Language::English, Language::Portuguese] {
            app.language = language;
            for _ in 0..2 {
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    app.render_character_lifecycle(ui.ctx())
                });
                output.textures_delta.clear();
            }
            assert_eq!(app.lifecycle.request, request);
            assert_eq!(app.lifecycle.quote.as_ref(), Some(&quote));
        }
        app.edit.name = "Unsaved".into();
        app.character_lifecycle_action(2);
        assert!(app.lifecycle.context.as_ref().unwrap().history.is_empty());
        app.edit = app.saved_edit.clone();
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        app.character_lifecycle_action(2);
        assert_eq!(app.selected_record.as_ref().unwrap().status, "Retired");
        assert!(!app.lifecycle.dirty);
        assert_eq!(app.lifecycle.context.as_ref().unwrap().history.len(), 1);
        assert_eq!(app.edit, app.saved_edit);
        app.change_route(WorkArea::Player);
        assert!(app.lifecycle.context.is_none());
        assert!(app.lifecycle.quote.is_none());
    }
}
