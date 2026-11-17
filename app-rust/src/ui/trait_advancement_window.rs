use super::*;
use grapevine_workbench::{
    pricing::PricingProfile, trait_advancement as advance, trait_pricing::Category,
};

pub(super) struct TraitWindow {
    pub kind: String,
    pub open: bool,
    pub target: Option<String>,
    pub context: Option<advance::Context>,
    pub request: advance::Request,
    pub field: String,
    pub index: Option<usize>,
    pub new_name: String,
    pub quote: Option<advance::Quote>,
    pub dirty: bool,
    pub message: Option<String>,
    pub correction: Option<grapevine_workbench::trait_corrections::Request>,
    pub correction_quote: Option<grapevine_workbench::trait_corrections::Quote>,
    correction_return_dirty: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_form_retains_quote_across_language_and_cancel_then_clears_private_state() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("traits-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let id = lib
            .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        lib.initialize_xp(
            &id,
            &grapevine_workbench::xp::Baseline {
                day: "2026-01-01".into(),
                earned: "10".into(),
                unspent: "10".into(),
                actor: "Synthetic".into(),
                reason: "Synthetic opening".into(),
                evidence: "Synthetic".into(),
            },
            1,
            Role::Storyteller,
        )
        .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.open_trait_advancement();
        app.ordinary.request.category = Category::Willpower;
        app.ordinary.field = "Willpower".into();
        app.ordinary.request.profile = Some(PricingProfile::MetLawsOfTheNightRevised);
        app.ordinary.request.day = "2026-01-02".into();
        app.ordinary.request.actor = "PRIVATE_UI_ACTOR".into();
        app.ordinary.request.reason = "PRIVATE_UI_REASON".into();
        app.ordinary.request.policy_version = "synthetic-v1".into();
        app.ordinary.request.review_reference =
            "Synthetic eligibility, maxima and approval review".into();
        app.ordinary.request.review_confirmed = true;
        app.trait_action(Action::Prepare);
        let quote = app.ordinary.quote.clone().unwrap();
        let ctx = egui::Context::default();
        for language in [Language::English, Language::Portuguese] {
            app.language = language;
            for _ in 0..2 {
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    app.render_trait_advancement(ui.ctx())
                });
                output.textures_delta.clear();
            }
            assert_eq!(app.ordinary.quote.as_ref(), Some(&quote));
        }
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        assert_eq!(app.ordinary.request.actor, "PRIVATE_UI_ACTOR");
        app.trait_action(Action::Commit);
        assert!(!app.ordinary.dirty);
        assert_eq!(
            app.ordinary.context.as_ref().unwrap().sheet.resources["Willpower"],
            1
        );
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
        app.ordinary.correction = Some(grapevine_workbench::trait_corrections::Request {
            id: uuid::Uuid::new_v4().to_string(),
            advancement_id: quote.request.id.clone(),
            day: "2026-01-03".into(),
            actor: "PRIVATE_CORRECTION_ACTOR".into(),
            reason: "Synthetic clerical correction".into(),
            evidence: "Synthetic reviewed correction".into(),
        });
        app.ordinary.dirty = true;
        app.trait_action(Action::PrepareCorrection);
        let correction = app.ordinary.correction_quote.clone().unwrap();
        for language in [Language::English, Language::Portuguese] {
            app.language = language;
            let mut output = ctx.run_ui(Default::default(), |ui| {
                app.render_trait_advancement(ui.ctx())
            });
            output.textures_delta.clear();
            assert_eq!(app.ordinary.correction_quote.as_ref(), Some(&correction));
        }
        app.trait_action(Action::CommitCorrection);
        assert_eq!(
            app.ordinary.context.as_ref().unwrap().sheet.resources["Willpower"],
            0
        );
        assert_eq!(
            app.ordinary.context.as_ref().unwrap().corrected_ids,
            vec![quote.request.id.clone()]
        );
        assert_eq!(
            app.library
                .as_ref()
                .unwrap()
                .xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("10")
        );
        let old_revision = app.ordinary.context.as_ref().unwrap().revision;
        Library::open(&path)
            .unwrap()
            .create_character("Mortal", "Other", old_revision, Role::Storyteller)
            .unwrap();
        app.ordinary.request.review_confirmed = true;
        app.trait_action(Action::Prepare);
        assert!(app.ordinary.quote.is_none());
        assert_eq!(
            app.ordinary.context.as_ref().unwrap().revision,
            old_revision
        );
        assert_eq!(app.ordinary.request.actor, "PRIVATE_UI_ACTOR");
        app.perform_action(PendingAction::Route(WorkArea::Player));
        assert!(!app.ordinary.open);
        assert!(app.ordinary.context.is_none());
        assert!(app.ordinary.request.actor.is_empty());
        assert!(app.ordinary.correction.is_none());
        app.open_trait_advancement();
        assert!(!app.ordinary.open);
    }
}
impl Default for TraitWindow {
    fn default() -> Self {
        Self {
            kind: "Vampire".into(),
            open: false,
            target: None,
            context: None,
            field: "AbilityList".into(),
            index: None,
            new_name: String::new(),
            quote: None,
            dirty: false,
            message: None,
            correction: None,
            correction_quote: None,
            correction_return_dirty: false,
            request: advance::Request {
                id: uuid::Uuid::new_v4().to_string(),
                category: Category::Ability,
                target: advance::Target::Trait {
                    field: "AbilityList".into(),
                    index: None,
                    name: String::new(),
                },
                profile: None,
                show_on_player_sheet: false,
                day: String::new(),
                actor: String::new(),
                reason: String::new(),
                policy_version: String::new(),
                review_reference: String::new(),
                review_confirmed: false,
            },
        }
    }
}
fn t<'a>(pt: bool, en: &'a str, br: &'a str) -> &'a str {
    if pt { br } else { en }
}
fn remember(ui: &egui::Ui, response: &egui::Response, key: &str) {
    let rect = (ui.is_rect_visible(response.rect) && ui.clip_rect().contains_rect(response.rect))
        .then_some(response.rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::unique(key), rect));
}
fn category_label(category: Category, pt: bool) -> &'static str {
    match category {
        Category::Attribute => t(pt, "Attribute", "Atributo"),
        Category::Ability => t(pt, "Ability", "Habilidade"),
        Category::Background => t(pt, "Background", "Antecedente"),
        Category::HumanityPath => t(pt, "Humanity / Path", "Humanidade / Trilha"),
        Category::Virtue => t(pt, "Virtue", "Virtude"),
        Category::Willpower => t(pt, "Willpower", "Força de Vontade"),
        Category::PermanentResource => t(pt, "Permanent resource", "Recurso permanente"),
    }
}
fn fields(category: Category) -> &'static [&'static str] {
    match category {
        Category::Attribute => &["PhysicalList", "SocialList", "MentalList"],
        Category::Ability => &["AbilityList"],
        Category::Background => &["BackgroundList"],
        Category::HumanityPath => &["Humanity", "PathTraits"],
        Category::Virtue => &[
            "Conscience",
            "Conviction",
            "Courage",
            "SelfControl",
            "Instinct",
        ],
        Category::Willpower => &["Willpower"],
        Category::PermanentResource => grapevine_workbench::sheet::RESOURCE_FIELDS,
    }
}

fn field_label(field: &str, pt: bool) -> &str {
    match field {
        "PhysicalList" => t(pt, "Physical traits", "Características físicas"),
        "SocialList" => t(pt, "Social traits", "Características sociais"),
        "MentalList" => t(pt, "Mental traits", "Características mentais"),
        "AbilityList" => t(pt, "Abilities", "Habilidades"),
        "BackgroundList" => t(pt, "Backgrounds", "Antecedentes"),
        "Humanity" => t(pt, "Humanity", "Humanidade"),
        "PathTraits" => t(pt, "Path traits", "Características de Trilha"),
        "Conscience" => t(pt, "Conscience", "Consciência"),
        "Conviction" => t(pt, "Conviction", "Convicção"),
        "Courage" => t(pt, "Courage", "Coragem"),
        "SelfControl" => t(pt, "Self-control", "Autocontrole"),
        "Instinct" => t(pt, "Instinct", "Instinto"),
        "Willpower" => t(pt, "Willpower", "Força de Vontade"),
        _ => field,
    }
}
fn render_correction(
    ui: &mut egui::Ui,
    state: &mut TraitWindow,
    pt: bool,
    action: &mut Option<Action>,
) {
    ui.heading(t(
        pt,
        "Correct a recorded increment",
        "Corrigir um aumento registrado",
    ));
    ui.label(t(pt,"A clerical correction restores the prior numeric count and refunds its original XP debit together. This is not an automatic game-rule refund. Later changes can block it.","Uma correção administrativa restaura o valor numérico anterior e reembolsa o débito original de XP juntos. Não é um reembolso automático pelas regras do jogo. Mudanças posteriores podem impedir a operação."));
    let Some(request) = state.correction.as_mut() else {
        return;
    };
    if let Some(q) = state.context.as_ref().and_then(|c| {
        c.history
            .iter()
            .find(|q| q.request.id == request.advancement_id)
    }) {
        ui.strong(format!(
            "{}: {} -> {} · {} XP",
            q.label, q.price.next, q.price.current, q.price.cost
        ));
    }
    let before = request.clone();
    for (caption, value) in [
        (
            t(
                pt,
                "Correction date (YYYY-MM-DD)",
                "Data da correção (AAAA-MM-DD)",
            ),
            &mut request.day,
        ),
        (t(pt, "Recorder", "Responsável"), &mut request.actor),
        (
            t(pt, "Correction reason", "Motivo da correção"),
            &mut request.reason,
        ),
        (
            t(pt, "Evidence reference", "Referência da evidência"),
            &mut request.evidence,
        ),
    ] {
        ui.label(caption);
        ui.text_edit_singleline(value);
    }
    if before != *request {
        state.correction_quote = None;
        state.dirty = true;
    }
    let b = ui.button(t(
        pt,
        "Preview paired correction",
        "Prévia da correção conjunta",
    ));
    remember(ui, &b, "ordinary_correction_prepare");
    if b.clicked() {
        *action = Some(Action::PrepareCorrection);
    }
    if let Some(q) = &state.correction_quote {
        ui.separator();
        ui.strong(format!(
            "{}: {} XP",
            t(pt, "Original-cost refund", "Reembolso do custo original"),
            q.refund
        ));
        ui.label(format!("XP: {} -> {}", q.before_unspent, q.after_unspent));
        ui.weak(t(pt,"The original debit stays in history with a linked reversal. Other fields and current sharing are preserved.","O débito original permanece no histórico com um estorno vinculado. Outros campos e a visibilidade atual são preservados."));
        let b = ui.button(t(
            pt,
            "Save correction and refund together",
            "Salvar correção e reembolso juntos",
        ));
        remember(ui, &b, "ordinary_correction_commit");
        if b.clicked() {
            *action = Some(Action::CommitCorrection);
        }
    }
    if ui
        .button(t(
            pt,
            "Back / discard correction preview",
            "Voltar / descartar prévia da correção",
        ))
        .clicked()
    {
        *action = Some(Action::CancelCorrection);
    }
}

#[derive(Clone, Copy)]
enum Action {
    QueueMixed,
    Reload,
    Prepare,
    Commit,
    PrepareCorrection,
    CommitCorrection,
    CancelCorrection,
}
impl InconnuApp {
    pub(super) fn open_trait_advancement(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if self.ordinary.target.as_ref() != Some(&id) {
            self.ordinary = TraitWindow {
                open: true,
                target: Some(id),
                ..Default::default()
            };
            self.reload_trait_advancement();
        } else {
            self.ordinary.open = true;
        }
        self.ordinary.kind = self
            .selected_record
            .as_ref()
            .map(|r| r.kind.clone())
            .unwrap_or_else(|| "Vampire".into());
    }
    fn reload_trait_advancement(&mut self) {
        self.ordinary.correction = None;
        self.ordinary.correction_quote = None;
        self.ordinary.correction_return_dirty = false;
        self.ordinary.quote = None;
        self.ordinary.request.review_confirmed = false;
        self.ordinary.index = None;
        self.ordinary.new_name.clear();
        self.ordinary.field = fields(self.ordinary.request.category)[0].into();
        self.ordinary.dirty = false;
        let (Some(lib), Some(id)) = (&self.library, &self.ordinary.target) else {
            return;
        };
        match lib.trait_advancement_context(id, Role::Storyteller) {
            Ok(context) => {
                self.ordinary.request.show_on_player_sheet = context
                    .sheet
                    .visibility
                    .get(&self.ordinary.field)
                    .copied()
                    .unwrap_or(false);
                self.ordinary.context = Some(context);
                self.ordinary.message = None;
            }
            Err(error) => {
                self.ordinary.context = None;
                self.ordinary.message = Some(error.message);
            }
        }
    }
    fn trait_action(&mut self, action: Action) {
        if matches!(action, Action::CancelCorrection) {
            self.ordinary.correction = None;
            self.ordinary.correction_quote = None;
            self.ordinary.dirty = self.ordinary.correction_return_dirty;
            return;
        }
        if matches!(action, Action::Reload) {
            self.reload_trait_advancement();
            return;
        }
        if matches!(action, Action::QueueMixed) {
            if let Some(q) = self.ordinary.quote.clone()
                && self.queue_mixed_request(
                    q.record_id.clone(),
                    grapevine_workbench::mixed_advancement::Request::Trait(Box::new(
                        q.request.clone(),
                    )),
                    q.record_hash,
                )
            {
                self.ordinary.quote = None;
                self.ordinary.dirty = false;
                self.ordinary.open = false;
                self.ordinary.request.id = uuid::Uuid::new_v4().to_string();
                self.ordinary.request.review_confirmed = false;
            }
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.ordinary.message = Some(
                self.t(
                    "Save or discard the sheet edits before reviewing advancement.",
                    "Salve ou descarte as edições da ficha antes de revisar o avanço.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(id), Some(context)) = (
            &mut self.library,
            self.ordinary.target.clone(),
            self.ordinary.context.clone(),
        ) else {
            return;
        };
        if matches!(action, Action::PrepareCorrection) {
            self.ordinary.correction_quote = None;
            if let Some(request) = &self.ordinary.correction {
                match lib.quote_trait_correction(&id, request, context.revision, Role::Storyteller)
                {
                    Ok(q) => {
                        self.ordinary.correction_quote = Some(q);
                        self.ordinary.message = None;
                        self.ordinary.dirty = true;
                    }
                    Err(error) => self.ordinary.message = Some(error.message),
                }
            }
        } else if matches!(action, Action::CommitCorrection) {
            if let Some(q) = self.ordinary.correction_quote.clone() {
                if self.ordinary.correction.as_ref() != Some(&q.request) {
                    self.ordinary.correction_quote = None;
                    return;
                }
                match lib.commit_trait_correction(&q, Role::Storyteller) {
                    Ok(_) => {
                        self.revision = lib.revision().ok();
                        self.refresh_summaries(true);
                        self.reload_trait_advancement();
                        self.ordinary.message = Some(
                            self.t(
                                "Correction and original-cost XP refund saved together.",
                                "Correção e reembolso do custo original em XP salvos juntos.",
                            )
                            .into(),
                        );
                    }
                    Err(error) => self.ordinary.message = Some(error.message),
                }
            }
        } else if matches!(action, Action::Prepare) {
            self.ordinary.quote = None;
            let state = &mut self.ordinary;
            state.request.target = if matches!(
                state.request.category,
                Category::Attribute | Category::Ability | Category::Background
            ) {
                let name = if let Some(index) = state.index {
                    context
                        .sheet
                        .lists
                        .get(&state.field)
                        .and_then(|v| v["Traits"].get(index))
                        .and_then(|v| v["Name"].as_str())
                        .unwrap_or("")
                        .to_owned()
                } else {
                    state.new_name.clone()
                };
                advance::Target::Trait {
                    field: state.field.clone(),
                    index: state.index,
                    name,
                }
            } else {
                advance::Target::Resource {
                    field: state.field.clone(),
                }
            };
            match lib.quote_trait_advancement(
                &id,
                &state.request,
                context.revision,
                Role::Storyteller,
            ) {
                Ok(q) => {
                    state.quote = Some(q);
                    state.dirty = true;
                    state.message = None;
                }
                Err(error) => state.message = Some(error.message),
            }
        } else if let Some(q) = self.ordinary.quote.clone() {
            if q.request != self.ordinary.request {
                self.ordinary.quote = None;
                return;
            }
            match lib.commit_trait_advancement(&q, Role::Storyteller) {
                Ok(_) => {
                    self.revision = lib.revision().ok();
                    self.ordinary.dirty = false;
                    self.ordinary.request.id = uuid::Uuid::new_v4().to_string();
                    self.refresh_summaries(true);
                    self.reload_trait_advancement();
                    self.ordinary.message = Some(
                        self.t(
                            "Trait and XP debit saved together.",
                            "Característica e débito de XP salvos juntos.",
                        )
                        .into(),
                    );
                }
                Err(error) => self.ordinary.message = Some(error.message),
            }
        }
    }
    pub(super) fn render_trait_advancement(&mut self, ctx: &egui::Context) {
        if !self.ordinary.open {
            return;
        }
        if self.route != WorkArea::Storyteller || self.ordinary.target != self.selected_id {
            self.ordinary = Default::default();
            return;
        }
        let pt = self.language == Language::Portuguese;
        let current_revision = self
            .library
            .as_ref()
            .and_then(|lib| lib.revision().ok())
            .unwrap_or(-1);
        let mut open = true;
        let mut action = None;
        egui::Window::new(t(pt,"Ordinary trait advancement", "Avanço de características"))
            .id(egui::Id::unique("ordinary_trait_window")).default_size([680.0,620.0]).open(&mut open).show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap().intersect(ui.clip_rect());
            ui.ctx().data_mut(|data| data.insert_temp(egui::Id::unique("ordinary_scroll"), Some(rect)));
            egui::ScrollArea::vertical().id_salt("ordinary_trait_scroll").show(ui, |ui| {
                let state = &mut self.ordinary;
                ui.label(t(pt,"One reviewed XP-funded increment. This is not a creation budget, a Discipline purchase or automatic eligibility approval.", "Um aumento revisado pago com XP. Não é orçamento de criação, compra de Disciplina nem aprovação automática de elegibilidade."));
                if ui.button(t(pt,"Reload current sheet / discard preview", "Recarregar ficha / descartar prévia")).clicked() {action = Some(Action::Reload);}
                if let Some(message) = &state.message {ui.label(message);}
                if state.correction.is_some() {render_correction(ui,state,pt,&mut action);return;}
                let Some(context) = &state.context else {return;};
                ui.strong(&context.record_name);
                ui.label(format!("XP: {}", context.unspent.as_deref().unwrap_or(t(pt,"recorded opening required", "saldo inicial registrado necessário"))));
                let before = (state.request.clone(), state.field.clone(), state.index, state.new_name.clone());
                let target_before = (state.request.category, state.field.clone(), state.index);
                egui::ComboBox::from_id_salt("ordinary_category").selected_text(category_label(state.request.category, pt)).show_ui(ui, |ui| {
                    for category in [Category::Attribute, Category::Ability, Category::Background, Category::HumanityPath, Category::Virtue, Category::Willpower, Category::PermanentResource] {
                        ui.selectable_value(&mut state.request.category, category, category_label(category, pt));
                    }
                });
                let allowed: Vec<_> = fields(state.request.category).iter().copied().filter(|field|state.request.category!=Category::PermanentResource || grapevine_workbench::creature_profiles::profile(&state.kind).is_some_and(|p|p.trait_xp_costs.contains_key(*field))).collect();
                if allowed.is_empty() {ui.label(t(pt,"No configured resource costs for this creature.","Não há custos de recursos configurados para esta criatura."));return;}
                if !allowed.contains(&state.field.as_str()) {state.field = allowed[0].into(); state.index = None;}
                egui::ComboBox::from_id_salt("ordinary_field").selected_text(field_label(&state.field, pt)).show_ui(ui, |ui| {
                    for field in &allowed {ui.selectable_value(&mut state.field, (*field).into(), field_label(field, pt));}
                });
                if target_before.1 != state.field { state.index = None; }
                if matches!(state.request.category, Category::Attribute | Category::Ability | Category::Background) {
                    let rows = context.sheet.lists.get(&state.field).and_then(|v| v["Traits"].as_array());
                    egui::ComboBox::from_id_salt("ordinary_existing").selected_text(state.index.and_then(|i| rows?.get(i)?.get("Name")?.as_str()).unwrap_or(t(pt,"New named trait", "Nova característica"))).show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.index, None, t(pt,"New named trait", "Nova característica"));
                        if let Some(rows) = rows {for (index, row) in rows.iter().enumerate() { ui.selectable_value(&mut state.index, Some(index), format!("{} · {}", row["Name"].as_str().unwrap_or("?"), row["Value"].as_str().unwrap_or("?"))); }}
                    });
                    if state.index.is_none() {ui.label(t(pt,"Name", "Nome")); ui.text_edit_singleline(&mut state.new_name);}
                } else {ui.label(format!("{}: {}", t(pt,"Recorded permanent count", "Valor permanente registrado"), context.sheet.resources.get(&state.field).map(i64::to_string).unwrap_or_else(|| t(pt,"not available; preview checks the stored value", "indisponível; a prévia verifica o valor salvo").into())));}
                if target_before != (state.request.category, state.field.clone(), state.index) {
                    state.request.review_confirmed = false;
                    state.request.show_on_player_sheet = context.sheet.visibility.get(&state.field).copied().unwrap_or(false);
                }
                ui.checkbox(&mut state.request.show_on_player_sheet, t(pt,"Show this section on player sheet", "Mostrar esta seção na ficha do jogador"));
                let mut adopted = state.request.profile.is_some();
                ui.checkbox(&mut adopted, if state.kind=="Vampire" {t(pt,"Use the locally adopted MET Laws of the Night Revised price profile", "Usar o perfil de custos MET Laws of the Night Revised adotado localmente")}else{t(pt,"Use this creature’s reviewed configured price profile","Usar o perfil de custos configurado e revisado desta criatura")});
                state.request.profile = adopted.then_some(if state.kind=="Vampire" {PricingProfile::MetLawsOfTheNightRevised}else{PricingProfile::ConfiguredCreature});
                for (caption, value) in [(t(pt,"Effective date (YYYY-MM-DD)", "Data efetiva (AAAA-MM-DD)"), &mut state.request.day),
                    (t(pt,"Recorder", "Responsável"), &mut state.request.actor), (t(pt,"Reason", "Motivo"), &mut state.request.reason),
                    (t(pt,"Local policy version", "Versão da política local"), &mut state.request.policy_version),
                    (t(pt,"Eligibility / limit / approval review reference", "Referência de revisão de elegibilidade / limite / aprovação"), &mut state.request.review_reference)] {
                    ui.label(caption); ui.text_edit_singleline(value);
                }
                ui.checkbox(&mut state.request.review_confirmed, t(pt,"ST reviewed eligibility, applicable maxima and approval/storyline requirements. This is a local declaration.", "O Narrador revisou elegibilidade, limites aplicáveis e requisitos de aprovação/narrativa. Esta é uma declaração local."));
                if before != (state.request.clone(), state.field.clone(), state.index, state.new_name.clone()) {state.quote = None; state.dirty = true;}
                if current_revision != context.revision {ui.colored_label(egui::Color32::YELLOW, t(pt,"Library changed. Reload and review again; this draft will not be silently rebased.", "A biblioteca mudou. Recarregue e revise novamente; o rascunho não será atualizado silenciosamente."));}
                let b = ui.button(t(pt,"Preview trait and XP change", "Prévia da característica e do XP")); remember(ui, &b, "ordinary_prepare");
                if b.clicked() {action = Some(Action::Prepare);}
                if let Some(q) = &state.quote {
                    let label=if matches!(q.request.target,advance::Target::Resource {..}) {field_label(&q.field,pt)} else {q.label.as_str()};
                    ui.separator(); ui.strong(format!("{}: {} -> {} · {} XP", label, q.price.current, q.price.next, q.price.cost));
                    ui.label(format!("XP: {} -> {}", q.before_unspent, q.after_unspent));
                    ui.label(format!("{} ({}) · p.{} / PDF{}", q.price.citation.book, q.price.citation.edition, q.price.citation.printed_page, q.price.citation.pdf_page_1based));
                    ui.small(&q.price.library_prompt);
                    ui.weak(t(pt,"Price is source-backed. Eligibility, maxima and local approval were declared, not automatically verified. Portuguese rules text is not reviewed.", "O custo tem fonte. Elegibilidade, limites e aprovação local foram declarados, não verificados automaticamente. O texto das regras em português não foi revisado."));
                    let b = ui.button(t(pt,"Save trait and XP together", "Salvar característica e XP juntos")); remember(ui, &b, "ordinary_commit");
                    if b.clicked() {action = Some(Action::Commit);}
                    if ui.button(t(pt,"Add reviewed intent to combined plan","Adicionar intenção revisada ao plano combinado")).clicked() {action=Some(Action::QueueMixed);}
                }
                ui.separator(); ui.label(format!("{}: {}", t(pt,"Recorded ordinary advancements", "Avanços comuns registrados"), context.history.len()));
                let mut first=true;
                let mut selected=None;
                for q in context.history.iter().rev() {
                    let corrected=context.corrected_ids.contains(&q.request.id);
                    ui.horizontal(|ui| {
                        ui.label(format!("{} · {} -> {} · {} XP",q.label,q.price.current,q.price.next,q.price.cost));
                        if corrected {ui.weak(t(pt,"Corrected; history retained","Corrigido; histórico mantido"));}
                        else {
                            let b=ui.button(t(pt,"Correct this increment…","Corrigir este aumento…"));
                            if first {remember(ui,&b,"ordinary_correct");first=false;}
                            if b.clicked() {selected=Some(q.request.id.clone());}
                        }
                    });
                }
                if selected.is_some() && state.dirty {state.message=Some(t(pt,"Finish or explicitly reload/discard the current advancement draft before starting a correction.","Conclua ou recarregue/descarte explicitamente o rascunho de avanço atual antes de iniciar uma correção.").into());selected=None;}
                if let Some(id)=selected {
                    state.correction_return_dirty=state.dirty;
                    state.correction=Some(grapevine_workbench::trait_corrections::Request {id:uuid::Uuid::new_v4().to_string(),advancement_id:id,day:String::new(),actor:state.request.actor.clone(),reason:String::new(),evidence:String::new()});
                    state.quote=None;state.correction_quote=None;state.dirty=true;
                }
            });
        });
        self.ordinary.open = open;
        if let Some(action) = action {
            self.trait_action(action);
        }
    }
}

#[cfg(test)]
mod mixed_queue_tests {
    use super::*;
    #[test]
    fn existing_editors_queue_mixed_intent_and_changed_source_does_not_rebase() {
        use grapevine_workbench::mixed_advancement::Request;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("queue-editors.gvlib");
        let (id, requests) = smoke_mixed_fixture(&path);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.select_record(id.clone());
        app.open_trait_advancement();
        let Request::Trait(a) = &requests[0] else {
            panic!()
        };
        app.ordinary.request = *a.clone();
        app.ordinary.field = "AbilityList".into();
        app.ordinary.index = Some(0);
        app.trait_action(Action::Prepare);
        assert!(app.ordinary.quote.is_some());
        app.trait_action(Action::QueueMixed);
        assert_eq!(app.mixed.requests.len(), 1);
        assert!(!app.ordinary.dirty);
        app.open_purchases();
        let Request::Power(p) = &requests[1] else {
            panic!()
        };
        app.purchase_form.input = *p.clone();
        app.purchase_form.dirty = true;
        app.purchase_action(crate::purchase_ui::Action::Quote);
        assert!(app.purchase_form.quote.is_some());
        app.purchase_action(crate::purchase_ui::Action::AddMixed);
        assert_eq!(app.mixed.requests.len(), 2);
        assert!(!app.purchase_form.dirty);
        app.open_trait_advancement();
        let Request::Trait(a) = &requests[2] else {
            panic!()
        };
        app.ordinary.request = *a.clone();
        app.ordinary.field = "AbilityList".into();
        app.ordinary.index = Some(0);
        app.trait_action(Action::Prepare);
        app.trait_action(Action::QueueMixed);
        app.mixed_action(1);
        assert_eq!(app.mixed.quote.as_ref().unwrap().total_spend, "7");
        let pending = app.mixed.requests.clone();
        let lib = app.library.as_mut().unwrap();
        lib.create_character(
            "Mortal",
            "Other change",
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
        // Unrelated library changes can be reviewed again while the same source stays bound.
        app.mixed_action(1);
        assert!(app.mixed.quote.is_some());
        let lib = app.library.as_mut().unwrap();
        let r = lib.record(&id, Role::Storyteller).unwrap();
        lib.save_edit(
            &id,
            &Edit {
                name: "Changed source".into(),
                status: r.status,
                notes: r.notes,
            },
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
        app.mixed_action(1);
        assert!(app.mixed.quote.is_none());
        assert_eq!(app.mixed.requests, pending);
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
    }
}
