use super::*;
use grapevine_workbench::{published_powers as powers, published_reference as source};

pub(super) struct PublishedWindow {
    pub open: bool,
    pub target: Option<String>,
    pub target_list: Option<String>,
    pub context: Option<source::Context>,
    pub book_id: Option<String>,
    pub query: String,
    pub request: source::Request,
    pub quote: Option<source::Quote>,
    pub dirty: bool,
    pub message: Option<String>,
}
impl Default for PublishedWindow {
    fn default() -> Self {
        Self {
            open: false,
            target: None,
            target_list: None,
            context: None,
            book_id: None,
            query: String::new(),
            request: source::Request {
                id: uuid::Uuid::new_v4().to_string(),
                trait_index: 0,
                source_id: String::new(),
                day: String::new(),
                actor: String::new(),
                reason: String::new(),
                clear: false,
                rename_to_source: false,
                confirm_alias: false,
                confirm_index_evidence: false,
            },
            quote: None,
            dirty: false,
            message: None,
        }
    }
}
fn t<'a>(pt: bool, en: &'a str, br: &'a str) -> &'a str {
    if pt { br } else { en }
}
fn remember(ui: &egui::Ui, response: &egui::Response, key: &str) {
    let rect = ui.is_rect_visible(response.rect).then_some(response.rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::unique(key), rect));
}
#[derive(Clone, Copy)]
enum Action {
    Reload,
    Prepare,
    Clear,
    Commit,
}
impl InconnuApp {
    pub(super) fn open_published_sources(&mut self) {
        if self.route == WorkArea::Player {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if self.published.target.as_ref() == Some(&id) {
            self.published.open = true;
            return;
        }
        self.published = PublishedWindow {
            open: true,
            target: Some(id),
            ..Default::default()
        };
        self.reload_published_sources();
    }
    fn reload_published_sources(&mut self) {
        let (Some(lib), Some(id)) = (&self.library, &self.published.target) else {
            return;
        };
        self.published.quote = None;
        match lib.published_reference_context_for_list(
            id,
            self.published.target_list.as_deref(),
            Role::Storyteller,
        ) {
            Ok(context) => {
                self.published.context = Some(context);
                self.published.request.trait_index = 0;
                self.published.request.rename_to_source = false;
                self.published.request.confirm_alias = false;
                self.published.request.confirm_index_evidence = false;
                self.published.message = None;
            }
            Err(error) => {
                self.published.context = None;
                self.published.message = Some(error.message);
            }
        }
    }
    fn published_source_action(&mut self, action: Action) {
        if self.route == WorkArea::Player {
            return;
        }
        if matches!(action, Action::Reload) {
            self.reload_published_sources();
            return;
        }
        if self.edit != self.saved_edit
            || self.sheet_edit != self.saved_sheet
            || self.xp_form != self.xp_saved_form
        {
            self.published.message = Some(
                self.t(
                    "Save or discard sheet/XP drafts before reviewing a source.",
                    "Salve ou descarte edições de ficha/XP antes de revisar uma fonte.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(id), Some(context)) = (
            &mut self.library,
            &self.published.target,
            &self.published.context,
        ) else {
            return;
        };
        if matches!(action, Action::Prepare | Action::Clear) {
            self.published.quote = None;
            let mut request = self.published.request.clone();
            request.clear = matches!(action, Action::Clear);
            if request.clear {
                request.source_id.clear();
                request.rename_to_source = false;
            }
            match lib.quote_published_reference_for_list(
                id,
                &request,
                context.revision,
                self.published.target_list.as_deref(),
                Role::Storyteller,
            ) {
                Ok(quote) => {
                    self.published.quote = Some(quote);
                    self.published.message = None;
                    self.published.dirty = true;
                }
                Err(error) => self.published.message = Some(error.message),
            }
        } else if let Some(quote) = self.published.quote.clone() {
            let mut expected = self.published.request.clone();
            expected.clear = quote.request.clear;
            if expected.clear {
                expected.source_id.clear();
                expected.rename_to_source = false;
            }
            if expected != quote.request {
                self.published.quote = None;
                return;
            }
            match lib.commit_published_reference(&quote, Role::Storyteller) {
                Ok(_) => {
                    self.revision = lib.revision().ok();
                    self.published.dirty = false;
                    self.published.quote = None;
                    self.published.request.id = uuid::Uuid::new_v4().to_string();
                    self.refresh_summaries(true);
                    self.reload_published_sources();
                    self.published.message = Some(
                        self.t(
                            "Source attribution saved. No XP or MET tier changed.",
                            "Referência de fonte salva. Nenhum XP ou nível MET foi alterado.",
                        )
                        .into(),
                    );
                }
                Err(error) => self.published.message = Some(error.message),
            }
        }
    }
    pub(super) fn render_published_sources(&mut self, ctx: &egui::Context) {
        if !self.published.open {
            return;
        }
        if self.route == WorkArea::Player || self.published.target != self.selected_id {
            self.published = Default::default();
            return;
        }
        let pt = self.language == Language::Portuguese;
        let revision = self
            .library
            .as_ref()
            .and_then(|lib| lib.revision().ok())
            .unwrap_or(-1);
        let creature_kind = self
            .selected_record
            .as_ref()
            .map(|r| r.kind.clone())
            .unwrap_or_else(|| "Vampire".into());
        let mut open = true;
        let mut action = None;
        egui::Window::new(t(pt,"Published source attribution","Referências de fontes publicadas"))
            .id(egui::Id::unique("published_sources_window")).default_size([720.0,620.0]).open(&mut open).show(ctx,|ui|{
            egui::ScrollArea::vertical().id_salt("published_sources_scroll").show(ui,|ui|{
                let state=&mut self.published;
                if let Some(profile)=grapevine_workbench::creature_profiles::profile(&creature_kind) {
                    let previous=state.target_list.clone();
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut state.target_list,None,t(pt,"Primary powers","Poderes principais"));
                        ui.selectable_value(&mut state.target_list,Some(profile.secondary_power_list.clone()),t(pt,"Secondary powers","Poderes secundários"));
                    });
                    if previous!=state.target_list { state.request.id=uuid::Uuid::new_v4().to_string(); action=Some(Action::Reload); return; }
                }
                ui.label(t(pt,"Record a published name/level reference for an existing trait. XP, the saved game value and MET tier remain unchanged.","Registre uma referência de nome/nível publicado para uma característica existente. XP, valor salvo no jogo e nível MET permanecem iguais."));
                let b=ui.button(t(pt,"Reload current traits / discard preview","Recarregar características / descartar prévia"));
                if b.clicked(){action=Some(Action::Reload);}
                if let Some(message)=&state.message {ui.label(message);}
                let Some(context)=&state.context else{return;};
                ui.strong(&context.record_name);
                let catalogue=match powers::catalogue(){Ok(c)=>c,Err(error)=>{ui.label(error.message);return;}};
                let before=state.request.clone();
                egui::ComboBox::from_id_salt("published_trait").selected_text(context.traits.get(state.request.trait_index).and_then(|row|row["Name"].as_str()).unwrap_or(t(pt,"No existing traits in the selected list","Sem características na lista selecionada"))).show_ui(ui,|ui|{
                    for (index,row) in context.traits.iter().enumerate(){ui.selectable_value(&mut state.request.trait_index,index,format!("{} · {}",row["Name"].as_str().unwrap_or("?"),row["Value"].as_str().unwrap_or("")));}
                });
                let previous_book=state.book_id.clone();
                egui::ComboBox::from_id_salt("published_book").selected_text(state.book_id.as_deref().and_then(|id|catalogue.book(id)).map(|book|book.title.as_str()).unwrap_or(t(pt,"All indexed books","Todos os livros indexados"))).show_ui(ui,|ui|{
                    ui.selectable_value(&mut state.book_id,None,t(pt,"All indexed books","Todos os livros indexados"));
                    for book in catalogue.books.iter().filter(|book|book.applies_to(&creature_kind)) {ui.selectable_value(&mut state.book_id,Some(book.id.clone()),book.title.as_str());}
                });
                if previous_book!=state.book_id {state.request.source_id.clear();state.request.rename_to_source=false;state.request.confirm_alias=false;state.request.confirm_index_evidence=false;}
                if let Some(book)=state.book_id.as_deref().and_then(|id|catalogue.book(id)) {
                    ui.small(match book.coverage {powers::Coverage::Pending=>t(pt,"This book is not yet sourced; no powers are supplied from another edition.","Este livro ainda não foi catalogado; nenhum poder é fornecido de outra edição."),powers::Coverage::Partial=>t(pt,"Only the currently checked subset is available; book coverage is incomplete.","Somente a parte conferida está disponível; a cobertura do livro está incompleta.")});
                }
                ui.small(t(pt,"V20 presents Celerity, Fortitude and Potence levels1-5 as unnamed ratings.","V20 apresenta os níveis1-5 de Celerity, Fortitude e Potence como graduações sem nomes individuais."));
                ui.small(t(pt,"Coverage is partial: contents/index checks identify further chapters, paths, rituals and bloodline sections still to source.","Cobertura parcial: sumário/índice apontam capítulos, linhas, rituais e seções de linhagens ainda pendentes de catalogação."));
                ui.text_edit_singleline(&mut state.query);
                let hits: Vec<_>=catalogue.search(state.book_id.as_deref(),&state.query).into_iter().filter(|power|catalogue.book(&power.book_id).is_some_and(|book|book.applies_to(&creature_kind))).collect();
                ui.label(format!("{}: {}",t(pt,"Matching published entries","Registros publicados encontrados"),hits.len()));
                egui::ScrollArea::vertical().id_salt("published_entries").max_height(145.0).show(ui,|ui|{
                    for power in hits.iter().take(500) {ui.selectable_value(&mut state.request.source_id,power.id.clone(),format!("{:?} · {} · {} · {} {}{}",catalogue.book(&power.book_id).unwrap().edition,power.name,power.discipline,t(pt,"published level","nível publicado"),power.level,power.path.as_ref().map(|path|format!(" · {path}")).unwrap_or_default()));}
                });
                if hits.len()>500 {ui.small(t(pt,"Showing first500; narrow the search.","Mostrando primeiros500; refine a busca."));}
                if let Ok(reference)=catalogue.reference(&state.request.source_id) {source_label(ui,&reference,pt);}
                ui.horizontal(|ui|{ui.label(t(pt,"Review date","Data da revisão"));ui.text_edit_singleline(&mut state.request.day);});
                ui.horizontal(|ui|{ui.label(t(pt,"Recorder","Responsável"));ui.text_edit_singleline(&mut state.request.actor);});
                ui.horizontal(|ui|{ui.label(t(pt,"Reason / evidence","Motivo / evidência"));ui.text_edit_singleline(&mut state.request.reason);});
                ui.checkbox(&mut state.request.rename_to_source,t(pt,"Use the exact source name for this existing trait","Usar o nome exato da fonte nesta característica existente"));
                ui.checkbox(&mut state.request.confirm_alias,t(pt,"I reviewed the different saved name as an alias","Revisei o nome salvo diferente como um apelido"));
                if catalogue.power(&state.request.source_id).is_some_and(|power|power.evidence==powers::Evidence::IndexOnly) {
                    let b=ui.checkbox(&mut state.request.confirm_index_evidence,t(pt,"I checked this indexed entry in the cited book","Conferi este registro indexado no livro citado"));remember(ui,&b,"published_index_confirm");
                }
                if state.request.source_id!=before.source_id||state.request.trait_index!=before.trait_index {
                    state.request.rename_to_source=false;state.request.confirm_alias=false;state.request.confirm_index_evidence=false;
                }
                if state.request!=before {state.quote=None;state.dirty=true;}
                let b=ui.button(t(pt,"Review source attribution","Revisar referência de fonte"));remember(ui,&b,"published_prepare");if b.clicked(){action=Some(Action::Prepare);}
                if ui.button(t(pt,"Review removal of saved published reference","Revisar remoção da referência publicada salva")).clicked(){action=Some(Action::Clear);}
                if let Some(quote)=&state.quote {
                    ui.label(format!("{}: {} · {}",t(pt,"Existing trait","Característica existente"),quote.old_name,quote.old_value));
                    if let Some(reference)=&quote.reference {source_label(ui,reference,pt);}else{ui.strong(t(pt,"Remove published attribution; retain trait and XP","Remover referência publicada; manter característica e XP"));}
                    ui.label(t(pt,"The saved game value and XP ledger remain unchanged. Review name changes above.","O valor salvo no jogo e o histórico de XP permanecem iguais. Revise mudanças de nome acima."));
                    let b=ui.add_enabled(quote.revision==revision,egui::Button::new(t(pt,"Save reviewed source attribution","Salvar referência de fonte revisada")));remember(ui,&b,"published_commit");if b.clicked(){action=Some(Action::Commit);}
                    if quote.revision!=revision {ui.label(t(pt,"Library changed; reload traits and review again.","Biblioteca alterada; recarregue e revise novamente."));}
                }
                egui::CollapsingHeader::new(t(pt,"Private source review history","Histórico privado de revisões de fonte")).show(ui,|ui|{
                    for receipt in context.history.iter().rev() {
                        ui.label(format!("{} · {} · {} · r{}",receipt.quote.request.day,receipt.quote.request.actor,receipt.quote.old_name,receipt.committed_revision));
                        ui.label(&receipt.quote.request.reason);
                        if let Some(reference)=&receipt.quote.reference {source_label(ui,reference,pt);}else{ui.label(t(pt,"Published reference removed","Referência publicada removida"));}
                    }
                });
            });
        });
        self.published.open = open;
        if let Some(action) = action {
            self.published_source_action(action);
        }
    }
}
fn source_label(ui: &mut egui::Ui, reference: &powers::Reference, pt: bool) {
    ui.strong(format!(
        "{} · {} {}",
        reference.power.name,
        t(pt, "Published level", "Nível publicado"),
        reference.power.level
    ));
    ui.small(format!(
        "{} · {:?} · {} {} / PDF{}",
        reference.book_title,
        reference.edition,
        t(pt, "printed page", "página impressa"),
        reference
            .power
            .printed_page
            .map(|page| page.to_string())
            .unwrap_or_else(|| t(pt, "unverified", "não verificada").into()),
        reference.power.pdf_page
    ));
    ui.small(format!(
        "[For more detailed info check your library for \"{}.\"]",
        reference.book_title
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use grapevine_workbench::{
        pricing::{ClanRelation, PricingProfile},
        purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
        purchases::{PurchaseRequest, TeacherRule},
        xp::Baseline,
    };
    #[test]
    fn source_form_renders_retains_language_and_cancelled_navigation_then_clears_for_player() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("source-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let id = lib
            .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        lib.initialize_xp(
            &id,
            &Baseline {
                day: "2026-01-01".into(),
                earned: "10".into(),
                unspent: "10".into(),
                actor: "Synthetic ST".into(),
                reason: "Synthetic opening".into(),
                evidence: "Synthetic fixture".into(),
            },
            1,
            Role::Storyteller,
        )
        .unwrap();
        let purchase = PurchaseRequest {
            review_id: None,
            id: uuid::Uuid::new_v4().to_string(),
            power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
            profile: Some(PricingProfile::MetLawsOfTheNightRevised),
            relation: Some(ClanRelation::InClan),
            affiliation_name: "Synthetic".into(),
            day: "2026-01-02".into(),
            available_from: "2026-01-03".into(),
            next_session_reference: "Synthetic next session".into(),
            actor: "Synthetic ST".into(),
            reason: "Synthetic purchase".into(),
            policy_version: "Synthetic v1".into(),
            policy_reference: "Synthetic policy".into(),
            affiliation_evidence: "Synthetic affiliation".into(),
            approval_reference: "Synthetic approval".into(),
            eligibility_evidence: "Synthetic eligibility".into(),
            teacher_rule: Some(TeacherRule::Required),
            teacher_evidence: "Synthetic teacher".into(),
            standard_cost_confirmed: true,
        };
        let quote = lib
            .quote_purchase(&id, &purchase, 2, Role::Storyteller)
            .unwrap();
        lib.commit_purchase(&quote, Role::Storyteller).unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_published_sources();
        app.published.request.source_id = "v20-core:discipline:animalism:1:feral-whispers".into();
        app.published.request.day = "2026-01-03".into();
        app.published.request.actor = "PRIVATE_SOURCE_ACTOR".into();
        app.published.request.reason = "PRIVATE_SOURCE_REASON".into();
        app.published.dirty = true;
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                app.render_published_sources(ui.ctx())
            });
            output.textures_delta.clear();
        }
        app.published_source_action(Action::Prepare);
        let quote = app.published.quote.clone().unwrap();
        app.language = Language::Portuguese;
        assert_eq!(app.published.quote.as_ref(), Some(&quote));
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        assert_eq!(app.published.request.actor, "PRIVATE_SOURCE_ACTOR");
        app.published_source_action(Action::Commit);
        assert!(!app.published.dirty);
        assert_eq!(app.published.context.as_ref().unwrap().history.len(), 1);
        assert_eq!(
            app.library
                .as_ref()
                .unwrap()
                .xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("7")
        );
        app.published_source_action(Action::Clear);
        assert!(app.published.quote.as_ref().unwrap().reference.is_none());
        app.published_source_action(Action::Commit);
        assert_eq!(app.published.context.as_ref().unwrap().history.len(), 2);
        app.perform_action(PendingAction::Route(WorkArea::Player));
        assert!(!app.published.open);
        assert!(app.published.request.actor.is_empty());
        assert!(app.published.context.is_none());
        app.open_published_sources();
        assert!(!app.published.open);
    }
}
