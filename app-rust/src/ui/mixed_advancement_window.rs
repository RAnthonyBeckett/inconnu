use super::*;
use grapevine_workbench::{mixed_advancement as mixed, mixed_advancement_draft as draft};
#[derive(Default)]
pub(super) struct MixedWindow {
    pub open: bool,
    pub target: Option<String>,
    pub requests: Vec<mixed::Request>,
    pub source_hash: Option<String>,
    pub quote: Option<mixed::Quote>,
    pub review: Option<draft::Review>,
    pub dirty: bool,
    pub message: Option<String>,
}
fn t(pt: bool, en: &str, br: &str) -> String {
    if pt { br.into() } else { en.into() }
}
impl InconnuApp {
    pub(super) fn queue_mixed_request(
        &mut self,
        id: String,
        request: mixed::Request,
        source_hash: String,
    ) -> bool {
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.error = Some(
                self.t(
                    "Save or discard the main sheet edits first.",
                    "Salve ou descarte as edições da ficha principal primeiro.",
                )
                .into(),
            );
            return false;
        }
        if self.route != WorkArea::Storyteller || self.selected_id.as_ref() != Some(&id) {
            return false;
        }
        if self.mixed.dirty && self.mixed.target.as_ref() != Some(&id) {
            self.error = Some(
                self.t(
                    "Save or discard the other character's spending plan first.",
                    "Salve ou descarte primeiro o plano de gastos do outro personagem.",
                )
                .into(),
            );
            return false;
        }
        if self.mixed.target.as_ref() != Some(&id) {
            self.mixed = MixedWindow {
                target: Some(id),
                ..Default::default()
            };
        }
        if self
            .mixed
            .source_hash
            .as_ref()
            .is_some_and(|h| h != &source_hash)
        {
            self.mixed.message=Some(self.t("The character changed after this plan began. Preserve or discard the plan and review from the current character.","O personagem mudou desde o início deste plano. Preserve ou descarte o plano e revise a partir do personagem atual.").into());
            self.mixed.open = true;
            return false;
        }
        if self.mixed.requests.len() >= 32
            || self.mixed.requests.iter().any(|r| r.id() == request.id())
        {
            self.mixed.message = Some(
                self.t(
                    "The plan is full or this request is already queued.",
                    "O plano está cheio ou esta solicitação já está na fila.",
                )
                .into(),
            );
            return false;
        }
        self.mixed.source_hash = Some(source_hash);
        self.mixed.requests.push(request);
        self.mixed.quote = None;
        self.mixed.review = None;
        self.mixed.dirty = true;
        self.mixed.open = true;
        self.mixed.message = None;
        true
    }
    pub(super) fn open_mixed_advancement(&mut self) {
        if !matches!(self.route, WorkArea::Storyteller | WorkArea::Builder) {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if self.mixed.target.as_ref() != Some(&id) {
            self.mixed = MixedWindow {
                target: Some(id),
                ..Default::default()
            };
        }
        self.mixed.open = true;
    }
    fn mixed_other_dirty(&self) -> bool {
        self.edit != self.saved_edit
            || self.sheet_edit != self.saved_sheet
            || (self.xp_form.is_dirty() && self.xp_form != self.xp_saved_form)
            || self.purchase_form.dirty
            || self.ordinary.dirty
            || self.creation.dirty
            || self.met_reference.dirty
            || self.lifecycle.dirty
    }
    pub(super) fn mixed_action(&mut self, action: u8) {
        if !matches!(self.route, WorkArea::Storyteller | WorkArea::Builder)
            || self.mixed.target != self.selected_id
        {
            return;
        }
        if action == 0 {
            self.mixed.requests.clear();
            self.mixed.quote = None;
            self.mixed.review = None;
            self.mixed.source_hash = None;
            self.mixed.dirty = false;
            self.mixed.message = None;
            return;
        }
        if self.mixed_other_dirty() {
            self.mixed.message = Some(
                self.t(
                    "Save or discard other character/XP drafts first.",
                    "Salve ou descarte outros rascunhos de personagem/XP primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(id), Some(lib)) = (self.mixed.target.clone(), &mut self.library) else {
            return;
        };
        let expected = lib.revision().unwrap_or(-1);
        if self.route == WorkArea::Builder {
            if action == 1 {
                self.mixed.review = None;
                match lib.review_mixed_advancement_draft(&id, expected, Role::Storyteller) {
                    Ok(r) => {
                        self.mixed.review = Some(r);
                        self.mixed.message = None;
                    }
                    Err(e) => self.mixed.message = Some(e.message),
                }
            } else if action == 2
                && let Some(review) = self.mixed.review.clone()
            {
                match lib.apply_mixed_advancement_draft(&review, Role::Storyteller) {
                    Ok(_) => {
                        self.revision = lib.revision().ok();
                        self.refresh_summaries(true);
                        self.mixed.review = self.library.as_ref().and_then(|l| {
                            l.review_mixed_advancement_draft(
                                &id,
                                l.revision().ok()?,
                                Role::Storyteller,
                            )
                            .ok()
                        });
                        self.mixed.message=Some(self.t("Plan applied once. Draft retained as a private review record.","Plano aplicado uma vez. Rascunho mantido como registro privado da revisão.").into());
                    }
                    Err(e) => self.mixed.message = Some(e.message),
                }
            }
            return;
        }
        if action == 1 {
            self.mixed.quote = None;
            match lib.quote_mixed_advancement(
                &id,
                &self.mixed.requests,
                expected,
                Role::Storyteller,
            ) {
                Ok(q) if self.mixed.source_hash.as_ref() == Some(&q.record_hash) => {
                    self.mixed.quote = Some(q);
                    self.mixed.message = None;
                }
                Ok(_) => self.mixed.message = Some(
                    self.t(
                        "The live character changed. This plan will not be silently rebased.",
                        "O personagem ativo mudou. Este plano não será atualizado silenciosamente.",
                    )
                    .into(),
                ),
                Err(e) => self.mixed.message = Some(e.message),
            }
        } else if action == 2
            && let Some(q) = self.mixed.quote.clone()
        {
            if q.requests() != self.mixed.requests
                || Some(&q.record_hash) != self.mixed.source_hash.as_ref()
            {
                return;
            }
            match lib.quote_mixed_advancement(
                &id,
                &self.mixed.requests,
                q.revision,
                Role::Storyteller,
            ) {
                Ok(current) if current == q => {}
                Ok(_) => {
                    self.mixed.message = Some(
                        self.t(
                            "The source or reviewed plan changed. Preview again.",
                            "A fonte ou o plano revisado mudou. Visualize novamente.",
                        )
                        .into(),
                    );
                    return;
                }
                Err(e) => {
                    self.mixed.message = Some(e.message);
                    return;
                }
            }
            match lib.save_mixed_advancement_draft(
                &id,
                &self.mixed.requests,
                q.revision,
                Role::Storyteller,
            ) {
                Ok(draft_id) => {
                    self.mixed.dirty = false;
                    self.mixed.requests.clear();
                    self.revision = lib.revision().ok();
                    self.change_route(WorkArea::Builder);
                    self.refresh_summaries(false);
                    self.select_record(draft_id);
                    self.open_mixed_advancement();
                    self.mixed_action(1);
                }
                Err(e) => self.mixed.message = Some(e.message),
            }
        }
    }
    pub(super) fn render_mixed_advancement(&mut self, ctx: &egui::Context) {
        if !matches!(self.route, WorkArea::Storyteller | WorkArea::Builder)
            || self.mixed.target != self.selected_id
        {
            self.mixed = Default::default();
            return;
        }
        if !self.mixed.open {
            return;
        }
        let builder = self.route == WorkArea::Builder;
        let pt = self.language == Language::Portuguese;
        let live_revision = self.library.as_ref().and_then(|l| l.revision().ok());
        let mut action = None;
        let mut reorder = None;
        let mut open = true;
        let state = &mut self.mixed;
        egui::Window::new(t(pt,"Combined spending plan","Plano combinado de gastos")).id(egui::Id::unique("mixed_advancement")).open(&mut open).default_width(760.0).default_height(580.0).show(ctx,|ui|{
            ui.horizontal_wrapped(|ui|{
                let b=ui.button(if builder{t(pt,"Review saved plan","Revisar plano salvo")}else{t(pt,"Preview complete plan","Visualizar plano completo")});remember(ui,&b,"mixed_prepare");if b.clicked(){action=Some(1);}
                let fresh=if builder{state.review.as_ref().is_some_and(|r|!r.applied&&Some(r.quote.revision)==live_revision)}else{state.quote.as_ref().is_some_and(|q|Some(q.revision)==live_revision&&q.requests()==state.requests)};
                let b=ui.add_enabled(fresh,egui::Button::new(if builder{t(pt,"Apply reviewed plan","Aplicar plano revisado")}else{t(pt,"Save private Draft / review","Salvar rascunho privado / revisar")}));remember(ui,&b,"mixed_save_apply");if b.clicked(){action=Some(2);}
                if !builder&&ui.button(t(pt,"Discard queued plan","Descartar plano na fila")).clicked(){action=Some(0);}
            });
            if let Some(message)=&state.message{ui.label(message);}
            egui::ScrollArea::vertical().show(ui,|ui|{
                ui.label(t(pt,"Preview does not spend XP. Saved Drafts retain intent; their copied traits and XP never replace the live character. Changed source or edited Draft requires a new review.","A prévia não gasta XP. Rascunhos salvos mantêm a intenção; características e XP copiados nunca substituem o personagem ativo. Fonte alterada ou rascunho editado exige nova revisão."));
                if !builder {
                    ui.label(t(pt,"Add reviewed intent from Advance ordinary traits or Purchase / receipts. Each step keeps its own source and private staff evidence. Reordering requires another complete preview.","Adicione a intenção revisada em Avançar características ou Compra / recibos. Cada etapa mantém sua fonte e evidências privadas da equipe. Reordenar exige outra prévia completa."));
                    for (i,r) in state.requests.iter().enumerate(){ui.horizontal_wrapped(|ui|{
                        ui.label(format!("{}. {}",i+1,request_label(r)));
                        if i>0&&ui.small_button(t(pt,"Earlier","Antes")).clicked(){reorder=Some((i,Some(i-1)));}
                        if i+1<state.requests.len()&&ui.small_button(t(pt,"Later","Depois")).clicked(){reorder=Some((i,Some(i+1)));}
                        if ui.small_button(t(pt,"Remove","Remover")).clicked(){reorder=Some((i,None));}
                    });}
                }
                let quote=if builder{state.review.as_ref().map(|r|&r.quote)}else{state.quote.as_ref()};
                if let Some(q)=quote {render_quote(ui,q,pt);}
                if state.review.as_ref().is_some_and(|r|r.applied){ui.strong(t(pt,"Already applied; reopening does not spend again.","Já aplicado; reabrir não gasta novamente."));}
                ui.weak(t(pt,"Only supported existing prices and explicit local declarations are used. This does not establish organizational approval. Individual existing correction/refund tools remain available; group undo is not automatic.","Somente custos existentes compatíveis e declarações locais explícitas são usados. Isto não estabelece aprovação organizacional. Ferramentas individuais de correção/reembolso continuam disponíveis; desfazer o grupo não é automático."));
            });
        });
        self.mixed.open = open;
        if let Some((i, target)) = reorder {
            if let Some(j) = target {
                self.mixed.requests.swap(i, j);
            } else {
                self.mixed.requests.remove(i);
            }
            self.mixed.quote = None;
            self.mixed.dirty = true;
        }
        if let Some(action) = action {
            self.mixed_action(action);
        }
    }
}
fn request_label(r: &mixed::Request) -> String {
    match r {
        mixed::Request::Power(r) => grapevine_workbench::power_catalogue::lookup(&r.power_id)
            .map(|e| e.name.to_string())
            .unwrap_or_else(|| "Unsupported power".into()),
        mixed::Request::Trait(r) => match &r.target {
            grapevine_workbench::trait_advancement::Target::Trait { name, .. } => name.clone(),
            grapevine_workbench::trait_advancement::Target::Resource { field } => field.clone(),
        },
    }
}
fn render_quote(ui: &mut egui::Ui, q: &mixed::Quote, pt: bool) {
    for (i, step) in q.steps.iter().enumerate() {
        ui.separator();
        ui.strong(format!(
            "{}. {} · {} XP · {} -> {}",
            i + 1,
            step.label(),
            step.cost(),
            step.balances().0,
            step.balances().1
        ));
        match step {
            mixed::Step::Power(p) => {
                crate::purchase_ui::render_sources(ui, &p.price, pt);
                egui::CollapsingHeader::new(t(pt, "Private declarations", "Declarações privadas"))
                    .id_salt(i)
                    .show(ui, |ui| {
                        crate::purchase_ui::render_declarations(ui, &p.request, pt)
                    });
            }
            mixed::Step::Trait(a) => {
                ui.label(format!(
                    "{}: {} -> {}",
                    a.label, a.price.current, a.price.next
                ));
                ui.label(format!(
                    "{} ({}) · p.{} / PDF{}",
                    a.price.citation.book,
                    a.price.citation.edition,
                    a.price.citation.printed_page,
                    a.price.citation.pdf_page_1based
                ));
                ui.small(&a.price.library_prompt);
                egui::CollapsingHeader::new(t(pt, "Private review", "Revisão privada"))
                    .id_salt(i)
                    .show(ui, |ui| {
                        ui.label(&a.request.actor);
                        ui.label(&a.request.reason);
                        ui.label(&a.request.policy_version);
                        ui.label(&a.request.review_reference);
                    });
            }
        }
    }
    ui.heading(format!(
        "{}: {} XP · {} -> {}",
        t(pt, "Total", "Total"),
        q.total_spend,
        q.before_unspent,
        q.after_unspent
    ));
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
    fn mixed_plan_language_guard_saved_draft_apply_and_player_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mixed-ui.gvlib");
        let (id, requests) = smoke_mixed_fixture(&path);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.select_record(id.clone());
        let lib = app.library.as_ref().unwrap();
        let q = lib
            .quote_mixed_advancement(&id, &requests, lib.revision().unwrap(), Role::Storyteller)
            .unwrap();
        for r in requests {
            assert!(app.queue_mixed_request(id.clone(), r, q.record_hash.clone()));
        }
        app.mixed_action(1);
        let quote = app.mixed.quote.clone().unwrap();
        let requests = app.mixed.requests.clone();
        let ctx = egui::Context::default();
        for language in [Language::English, Language::Portuguese] {
            app.language = language;
            for _ in 0..2 {
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    app.render_mixed_advancement(ui.ctx())
                });
                output.textures_delta.clear();
            }
            assert_eq!(app.mixed.quote.as_ref(), Some(&quote));
            assert_eq!(app.mixed.requests, requests);
        }
        app.edit.name = "Unsaved".into();
        app.mixed_action(2);
        assert_eq!(app.route, WorkArea::Storyteller);
        app.edit = app.saved_edit.clone();
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        app.mixed_action(2);
        assert_eq!(app.route, WorkArea::Builder);
        assert!(!app.mixed.dirty);
        assert!(!app.mixed.review.as_ref().unwrap().applied);
        assert_eq!(
            app.library
                .as_ref()
                .unwrap()
                .xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("20")
        );
        app.mixed_action(2);
        assert!(app.mixed.review.as_ref().unwrap().applied);
        app.mixed_action(2);
        assert_eq!(
            app.library
                .as_ref()
                .unwrap()
                .xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("13")
        );
        app.change_route(WorkArea::Player);
        assert!(app.mixed.requests.is_empty());
        assert!(app.mixed.review.is_none());
    }
}
