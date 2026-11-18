use eframe::egui;
use grapevine_workbench::{model::Language, power_catalogue, pricing::*, purchases::*};

pub struct PurchaseForm {
    pub kind: String,
    pub power_list: String,
    pub input: PurchaseRequest,
    pub policy_name: String,
    pub reviews: Vec<(grapevine_workbench::purchase_reviews::ReviewBinding, bool)>,
    pub quote: Option<PurchaseQuote>,
    pub receipts: Vec<PurchaseReceipt>,
    pub message: Option<String>,
    pub dirty: bool,
    pub cancel_purchase: Option<PurchaseReceipt>,
    pub cancel_input: grapevine_workbench::cancellations::CancellationRequest,
    pub cancel_quote: Option<grapevine_workbench::cancellations::CancellationQuote>,
    pub plan: Vec<PurchaseRequest>,
    pub plan_quote: Option<grapevine_workbench::purchase_batch::Quote>,
}
impl Default for PurchaseForm {
    fn default() -> Self {
        Self {
            kind: "Vampire".into(),
            power_list: String::new(),
            plan: vec![],
            plan_quote: None,
            policy_name: String::new(),
            reviews: vec![],
            input: PurchaseRequest {
                review_id: None,
                id: uuid::Uuid::new_v4().to_string(),
                power_id: String::new(),
                profile: None,
                relation: None,
                affiliation_name: String::new(),
                day: String::new(),
                available_from: String::new(),
                next_session_reference: String::new(),
                actor: String::new(),
                reason: String::new(),
                policy_version: String::new(),
                policy_reference: String::new(),
                affiliation_evidence: String::new(),
                approval_reference: String::new(),
                eligibility_evidence: String::new(),
                teacher_rule: None,
                teacher_evidence: String::new(),
                standard_cost_confirmed: false,
                owbn_evidence: Default::default(),
            },
            quote: None,
            receipts: vec![],
            message: None,
            dirty: false,
            cancel_purchase: None,
            cancel_input: grapevine_workbench::cancellations::CancellationRequest {
                id: uuid::Uuid::new_v4().to_string(),
                day: String::new(),
                actor: String::new(),
                reason: String::new(),
                evidence: String::new(),
            },
            cancel_quote: None,
        }
    }
}
pub enum Action {
    AddMixed,
    SavePlanDraft,
    AddToPlan,
    RemoveFromPlan(usize),
    ReviewPlan,
    CommitPlan,
    SaveReview,
    ReuseReview(usize),
    SavePolicy,
    LoadPolicy,
    Quote,
    Commit,
    Reset,
    SelectCancel(usize),
    QuoteCancel,
    CommitCancel,
}
fn t<'a>(pt: bool, en: &'a str, br: &'a str) -> &'a str {
    if pt { br } else { en }
}
fn remember(ui: &egui::Ui, r: &egui::Response, key: &str) {
    let rect = ui.is_rect_visible(r.rect).then_some(r.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), rect));
}
pub fn render(
    ui: &mut egui::Ui,
    form: &mut PurchaseForm,
    revision: i64,
    language: Language,
) -> Option<Action> {
    let pt = language == Language::Portuguese;
    let configured = form.kind != "Vampire";
    if configured && let Some(profile) = grapevine_workbench::creature_profiles::profile(&form.kind)
    {
        if !profile.owns_power_list(&form.power_list) {
            form.power_list = profile.primary_power_list.clone();
        }
        let previous = form.power_list.clone();
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut form.power_list,
                profile.primary_power_list.clone(),
                t(pt, "Primary powers", "Poderes principais"),
            );
            ui.selectable_value(
                &mut form.power_list,
                profile.secondary_power_list.clone(),
                t(pt, "Secondary powers", "Poderes secundários"),
            );
        });
        if previous != form.power_list {
            form.input.power_id.clear();
            form.input.review_id = None;
            form.input.profile = None;
            form.input.standard_cost_confirmed = false;
            form.quote = None;
            form.dirty = true;
        }
    }
    let catalogue: Vec<_> = power_catalogue::entries()
        .into_iter()
        .filter(|p| {
            p.template_scope == form.kind
                && (!configured || p.target_list() == Some(form.power_list.as_str()))
        })
        .collect();
    if form.cancel_purchase.is_some() {
        return render_cancellation(ui, form, revision, pt);
    }
    let mut action = None;
    let before = form.input.clone();
    ui.strong(t(
        pt,
        "Local Storyteller purchase — reviewed Discipline tiers",
        "Compra pelo Narrador local — níveis de Disciplinas revisados",
    ));
    ui.label(t(pt,"One commit saves the XP debit, trait and receipt together. Local declarations do not prove external approval.","Uma confirmação salva o débito de XP, a característica e o recibo juntos. Declarações locais não comprovam aprovação externa."));
    ui.small(t(pt,"Save/discard other sheet or XP drafts before quoting. Closing keeps this form; navigation asks before discarding it.","Salve/descarte outras edições de ficha ou XP antes de cotar. Fechar mantém este formulário; a navegação pede confirmação antes de descartá-lo."));
    if let Some(plan_action) = render_plan(ui, form, revision, pt) {
        action = Some(plan_action);
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(t(pt, "Policy template name", "Nome do modelo de regra"));
        if ui.text_edit_singleline(&mut form.policy_name).changed() {
            form.dirty = true;
        }
        if ui
            .button(t(pt, "Save policy template…", "Salvar modelo de regra…"))
            .clicked()
        {
            action = Some(Action::SavePolicy);
        }
        if ui
            .button(t(pt, "Load policy template…", "Carregar modelo de regra…"))
            .clicked()
        {
            action = Some(Action::LoadPolicy);
        }
    });
    ui.small(t(pt,"Templates reuse source, policy version and teacher ruling only. Loading clears approval, eligibility and teacher evidence for fresh review; it grants no authority.","Modelos reutilizam apenas fonte, versão da regra e decisão sobre professor. Carregar limpa aprovação, elegibilidade e evidência do professor para nova revisão; não concede autoridade."));
    if let Some(id) = &form.input.review_id {
        ui.small(format!(
            "{}: {}",
            t(pt, "Linked local review", "Revisão local vinculada"),
            id
        ));
    }
    let button = ui.button(t(
        pt,
        "Save character / power review",
        "Salvar revisão de personagem / poder",
    ));
    remember(ui, &button, "review_save");
    if button.clicked() {
        action = Some(Action::SaveReview);
    }
    if let Some((review, current)) = form.reviews.last() {
        let power = catalogue
            .iter()
            .find(|p| p.stable_id == review.power_id)
            .map(|p| p.name.as_str())
            .unwrap_or(&review.power_id)
            .to_owned();
        ui.small(format!(
            "{} · {} · {} · {}",
            review.reviewed_day,
            power,
            review.affiliation_name,
            if *current {
                t(pt, "Current snapshot", "Estado atual")
            } else {
                t(pt, "Needs new review", "Precisa de nova revisão")
            }
        ));
        let b = ui.add_enabled(
            *current,
            egui::Button::new(t(pt, "Reuse latest review", "Reutilizar última revisão")),
        );
        remember(ui, &b, "review_reuse_latest");
        if b.clicked() {
            action = Some(Action::ReuseReview(form.reviews.len() - 1));
        }
    }
    egui::CollapsingHeader::new(t(pt, "Saved character reviews", "Revisões salvas do personagem")).default_open(false).show(ui, |ui| {
        ui.small(t(pt,"Reviews bind this character, power and policy to the reviewed sheet/XP state. Later sheet or XP changes require a new review. Approval and teacher evidence are always fresh.","Revisões vinculam personagem, poder e regra ao estado revisado da ficha/XP. Alterações posteriores exigem nova revisão. Aprovação e evidência do professor são sempre novas."));
        for (index, (review, current)) in form.reviews.iter().enumerate().rev() {
            ui.label(format!("{} · {} · {} · {}", review.reviewed_day, review.policy.name, review.policy.policy_version, if *current { t(pt,"Current snapshot","Estado atual") } else { t(pt,"Needs new review","Precisa de nova revisão") }));
            let b = ui.add_enabled(*current, egui::Button::new(t(pt,"Reuse this review","Reutilizar esta revisão")));
            if index + 1 == form.reviews.len() { remember(ui, &b, "review_reuse"); }
            if b.clicked() { action = Some(Action::ReuseReview(index)); }
        }
    });
    let r = &mut form.input;
    egui::ComboBox::from_id_salt("purchase_power")
        .selected_text(
            catalogue
                .iter()
                .find(|p| p.stable_id == r.power_id)
                .map(|p| p.name.as_str())
                .unwrap_or(t(pt, "Choose a power", "Escolher poder")),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut r.power_id,
                String::new(),
                t(pt, "Choose a power", "Escolher poder"),
            );
            for power in &catalogue {
                ui.selectable_value(
                    &mut r.power_id,
                    power.stable_id.clone(),
                    format!(
                        "{} — {} · {}",
                        power.family,
                        power.name,
                        tier_label(pt, power.tier)
                    ),
                );
            }
        });
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(
            &mut r.relation,
            None,
            t(pt, "Affiliation unknown", "Afiliação desconhecida"),
        );
        ui.selectable_value(
            &mut r.relation,
            Some(ClanRelation::InClan),
            t(pt, "In-clan", "Do clã"),
        );
        ui.selectable_value(
            &mut r.relation,
            Some(ClanRelation::OutOfClan),
            t(pt, "Out-of-clan", "Fora do clã"),
        );
    });
    let mut standard = r.standard_cost_confirmed;
    let selected_profile = power_catalogue::lookup(&r.power_id)
        .and_then(|p| p.profile)
        .unwrap_or(if configured {
            PricingProfile::ConfiguredCreature
        } else {
            PricingProfile::MetLawsOfTheNightRevised
        });
    if configured {
        ui.label(t(
            pt,
            "Configured creature rules require source review before a purchase is available.",
            "As regras configuradas da criatura exigem revisão de fonte antes da compra.",
        ));
    }
    if ui.checkbox(&mut standard,if configured {t(pt,"Use the source-reviewed configured creature cost profile","Usar o perfil configurado de custos da criatura com fonte revisada")}else{t(pt,"Use exact MET Revised costs: Basic3, Intermediate6, Advanced9; +1 out-of-clan, no house-rule adjustment","Usar custos exatos de MET Revised: básico3, intermediário6, avançado9; +1 fora do clã, sem ajuste local")}).changed(){r.standard_cost_confirmed=standard;r.profile=standard.then_some(selected_profile);}
    egui::CollapsingHeader::new(t(
        pt,
        "Dates and local review evidence",
        "Datas e evidências de revisão local",
    ))
    .default_open(false)
    .show(ui, |ui| {
        for (label, field) in [
            (
                t(pt, "Purchase date YYYY-MM-DD", "Data da compra AAAA-MM-DD"),
                &mut r.day,
            ),
            (
                t(
                    pt,
                    "Available from YYYY-MM-DD (next session)",
                    "Disponível a partir de AAAA-MM-DD (próxima sessão)",
                ),
                &mut r.available_from,
            ),
            (
                t(pt, "Next-session reference", "Referência da próxima sessão"),
                &mut r.next_session_reference,
            ),
            (
                t(pt, "Recorder / local ST", "Responsável / Narrador local"),
                &mut r.actor,
            ),
            (t(pt, "Reason", "Motivo"), &mut r.reason),
            (
                t(
                    pt,
                    "Clan / affiliation at purchase",
                    "Clã / afiliação na compra",
                ),
                &mut r.affiliation_name,
            ),
            (
                t(
                    pt,
                    "Affiliation evidence reference",
                    "Referência de evidência da afiliação",
                ),
                &mut r.affiliation_evidence,
            ),
            (
                t(pt, "Local policy version", "Versão da regra local"),
                &mut r.policy_version,
            ),
            (
                t(
                    pt,
                    "Local policy / adoption reference",
                    "Referência da regra local / adoção",
                ),
                &mut r.policy_reference,
            ),
            (
                t(pt, "Approval reference", "Referência da aprovação"),
                &mut r.approval_reference,
            ),
            (
                t(
                    pt,
                    "Eligibility / prerequisites review reference",
                    "Referência de revisão de elegibilidade / pré-requisitos",
                ),
                &mut r.eligibility_evidence,
            ),
        ] {
            ui.label(label);
            ui.text_edit_singleline(field);
        }
        ui.label(t(
            pt,
            "Teacher interpretation must be explicitly resolved locally.",
            "A interpretação do professor precisa de decisão local explícita.",
        ));
        ui.selectable_value(&mut r.teacher_rule, None, t(pt, "Unresolved", "Pendente"));
        ui.selectable_value(
            &mut r.teacher_rule,
            Some(TeacherRule::Required),
            t(
                pt,
                "Teacher required; identify qualification in evidence",
                "Professor exigido; identificar qualificação na evidência",
            ),
        );
        ui.selectable_value(
            &mut r.teacher_rule,
            Some(TeacherRule::NotRequiredByLocalRuling),
            t(
                pt,
                "Not required under cited local interpretation",
                "Não exigido conforme interpretação local citada",
            ),
        );
        ui.label(t(
            pt,
            "Teacher identity/qualification or explicit local ruling reference",
            "Identidade/qualificação do professor ou referência da decisão local explícita",
        ));
        ui.text_edit_multiline(&mut r.teacher_evidence);
    });
    render_owbn_evidence(ui, &mut form.input.owbn_evidence, &form.kind, pt, true);
    if before != form.input {
        form.dirty = true;
        form.quote = None;
        form.message = None;
    }
    ui.small(t(pt,"Sources: Laws of the Night (Revised), cost p.124 / PDF125; learning p.133 / PDF134. Portuguese mechanics translation remains unreviewed.","Fontes: Laws of the Night (Revised), custo p.124 / PDF125; aprendizado p.133 / PDF134. Tradução das mecânicas em português ainda não revisada."));
    if let Some(power) = catalogue
        .iter()
        .find(|p| p.stable_id == form.input.power_id)
    {
        if power_catalogue::builtin()
            .lookup(&power.stable_id)
            .is_some_and(|definition| {
                definition.effective_authority()
                    == grapevine_workbench::owbn_bylaws::PacketAuthority::NonBindingOrBook
            })
        {
            ui.small(if configured {
                grapevine_workbench::owbn_bylaws::non_binding_badge(pt)
            } else {
                t(
                    pt,
                    "MET rulebook (Local ST Discretion)",
                    "Livro de regras MET (a critério do Narrador local)",
                )
            });
        }
        ui.small(format!(
            "{} · {} · p.{} / PDF{}",
            power.family,
            power.name,
            power.source.map(|s| s.printed_page).unwrap_or_default(),
            power.source.map(|s| s.pdf_page_1based).unwrap_or_default()
        ));
    }
    ui.label("[For more detailed info check your library for \"Laws of the Night (Revised).\"]");
    if let Some(message) = &form.message {
        ui.label(message);
    }
    ui.horizontal_wrapped(|ui| {
        let b = ui.add_enabled(
            form.plan.len() < 32,
            egui::Button::new(t(
                pt,
                "Add this purchase to advancement plan",
                "Adicionar esta compra ao plano de evolução",
            )),
        );
        remember(ui, &b, "purchase_plan_add");
        if b.clicked() {
            action = Some(Action::AddToPlan);
        }
        let b = ui.add_enabled(
            form.plan.is_empty(),
            egui::Button::new(t(pt, "Review purchase quote", "Revisar cotação da compra")),
        );
        remember(ui, &b, "purchase_quote");
        if b.clicked() {
            action = Some(Action::Quote);
        }
        let b = ui.button(t(
            pt,
            "New / discard form and plan",
            "Novo / descartar formulário e plano",
        ));
        if b.clicked() {
            action = Some(Action::Reset);
        }
    });
    if let Some(q) = &form.quote {
        let current = q.revision == revision && q.request == form.input;
        ui.separator();
        ui.heading(&q.record_name);
        ui.strong(format!(
            "{}: {} + {} = {} XP",
            q.price.name,
            q.price.price.base,
            q.price.price.affiliation_adjustment,
            q.price.price.total
        ));
        ui.label(format!(
            "{}: {} -> {} XP",
            t(pt, "Committed balance", "Saldo salvo"),
            q.before_unspent,
            q.after_unspent
        ));
        ui.label(format!(
            "{}: {}",
            t(pt, "Available from", "Disponível a partir de"),
            q.request.available_from
        ));
        if !current {
            ui.colored_label(
                egui::Color32::YELLOW,
                t(
                    pt,
                    "Quote is stale. Review it again before committing.",
                    "Cotação desatualizada. Revise novamente antes de confirmar.",
                ),
            );
        }
        let b = ui.add_enabled(
            current && form.plan.is_empty(),
            egui::Button::new(t(
                pt,
                "Commit XP + trait + receipt",
                "Confirmar XP + característica + recibo",
            )),
        );
        if ui
            .add_enabled(
                current && form.plan.is_empty(),
                egui::Button::new(t(
                    pt,
                    "Add reviewed intent to combined plan",
                    "Adicionar intenção revisada ao plano combinado",
                )),
            )
            .clicked()
        {
            action = Some(Action::AddMixed);
        }
        remember(ui, &b, "purchase_commit");
        if b.clicked() {
            action = Some(Action::Commit);
        }
    }
    egui::CollapsingHeader::new(t(
        pt,
        "Saved purchase receipts",
        "Recibos de compras salvos",
    ))
    .default_open(true)
    .show(ui, |ui| {
        if form.receipts.is_empty() {
            ui.label(t(
                pt,
                "No purchase receipts on this sheet.",
                "Nenhum recibo de compra nesta ficha.",
            ));
        }
        for (index, receipt) in form.receipts.iter().enumerate() {
            let q = &receipt.quote;
            ui.label(format!(
                "{} · {} · {} XP · {}",
                q.request.day, q.price.name, q.price.price.total, q.request.id
            ));
            ui.small(format!(
                "{} · {} · {}",
                q.request.policy_version, q.request.affiliation_name, q.request.available_from
            ));
            if let Some(c) = &receipt.cancellation {
                ui.strong(format!(
                    "{} · {} · {} XP",
                    t(pt, "Cancelled / refunded", "Cancelada / reembolsada"),
                    c.quote.request.day,
                    c.quote.refund
                ));
            } else {
                let grouped = receipt
                    .batch
                    .as_ref()
                    .is_some_and(|binding| binding.count > 1);
                if grouped {
                    ui.small(t(
                        pt,
                        "Grouped plan: automatic cancellation is not yet available.",
                        "Plano agrupado: cancelamento automático ainda indisponível.",
                    ));
                }
                let b = ui.add_enabled(
                    !grouped && !form.dirty && form.quote.is_none(),
                    egui::Button::new(t(pt, "Review cancellation", "Revisar cancelamento")),
                );
                if index == 0 {
                    remember(ui, &b, "cancel_select");
                }
                if b.clicked() {
                    action = Some(Action::SelectCancel(index));
                }
            }
        }
    });
    ui.small(t(pt,"Linked debits cannot be reversed as manual XP entries. Cancellation needs an unchanged purchase snapshot; later edits/history require separate review.","Débitos vinculados não podem ser revertidos como registros manuais de XP. O cancelamento exige dados da compra inalterados; edições ou histórico posteriores precisam de revisão separada."));
    action
}

fn render_plan(
    ui: &mut egui::Ui,
    form: &mut PurchaseForm,
    revision: i64,
    pt: bool,
) -> Option<Action> {
    let mut action = None;
    if form.plan.is_empty() {
        return action;
    }
    ui.separator();
    ui.heading(t(
        pt,
        "Reviewed advancement plan",
        "Plano de evolução revisado",
    ));
    ui.label(t(pt, "Only queued purchases are included. Editing the entry form does not change a queued declaration; remove and add it again. All queued XP, traits and receipts save together, or none do.", "Somente compras na fila são incluídas. Editar o formulário não altera uma declaração na fila; remova e adicione novamente. Todo XP, características e recibos da fila são salvos juntos, ou nada é salvo."));
    ui.label(t(pt,"Automatic undo of a multi-purchase plan is not yet available. Review every queued declaration before applying.","Desfazer automaticamente um plano com várias compras ainda não está disponível. Revise cada declaração da fila antes de aplicar."));
    let catalogue: Vec<_> = power_catalogue::entries()
        .into_iter()
        .filter(|p| p.template_scope == form.kind)
        .collect();
    for (index, request) in form.plan.iter().enumerate() {
        let name = catalogue
            .iter()
            .find(|power| power.stable_id == request.power_id)
            .map(|power| power.name.as_str())
            .unwrap_or(request.power_id.as_str());
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} · {} · {}",
                index + 1,
                name,
                relation_label(pt, request.relation)
            ));
            let b = ui.button(t(pt, "Remove", "Remover"));
            if index == 0 {
                remember(ui, &b, "purchase_plan_remove");
            }
            if b.clicked() {
                action = Some(Action::RemoveFromPlan(index));
            }
        });
        egui::CollapsingHeader::new(format!(
            "{} {}",
            t(pt, "Declarations", "Declarações"),
            index + 1
        ))
        .id_salt(("plan_declarations", index))
        .show(ui, |ui| {
            render_declarations(ui, request, pt);
        });
    }
    let b = ui.button(t(
        pt,
        "Review queued plan against current XP",
        "Revisar plano com o XP atual",
    ));
    remember(ui, &b, "purchase_plan_review");
    if b.clicked() {
        action = Some(Action::ReviewPlan);
    }
    if let Some(quote) = &form.plan_quote {
        let current = quote.revision == revision
            && quote
                .steps
                .iter()
                .map(|step| &step.request)
                .eq(form.plan.iter());
        for step in &quote.steps {
            ui.label(format!(
                "{}: {} + {} = {} XP · {} -> {}",
                step.price.name,
                step.price.price.base,
                step.price.price.affiliation_adjustment,
                step.price.price.total,
                step.before_unspent,
                step.after_unspent
            ));
            render_sources(ui, &step.price, pt);
        }
        ui.strong(format!(
            "{}: {} XP · {} -> {} XP",
            t(pt, "Total plan cost", "Custo total do plano"),
            quote.total_spend,
            quote.before_unspent,
            quote.after_unspent
        ));
        if !current {
            ui.label(t(
                pt,
                "Plan is stale. Review it again; newer live XP is never overwritten.",
                "Plano desatualizado. Revise novamente; o XP atual nunca é substituído.",
            ));
        }
        let b = ui.add_enabled(
            current,
            egui::Button::new(t(
                pt,
                "Apply all queued purchases together",
                "Aplicar todas as compras da fila juntas",
            )),
        );
        remember(ui, &b, "purchase_plan_commit");
        if b.clicked() {
            action = Some(Action::CommitPlan);
        }
        let b = ui.add_enabled(
            current,
            egui::Button::new(t(
                pt,
                "Save plan as a separate advancement draft",
                "Salvar plano como rascunho separado de evolução",
            )),
        );
        remember(ui, &b, "purchase_plan_save");
        if b.clicked() {
            action = Some(Action::SavePlanDraft);
        }
    }
    ui.separator();
    action
}

fn render_cancellation(
    ui: &mut egui::Ui,
    form: &mut PurchaseForm,
    revision: i64,
    pt: bool,
) -> Option<Action> {
    let mut action = None;
    let purchase = form.cancel_purchase.as_ref().unwrap();
    ui.heading(format!(
        "{}: {}",
        t(pt, "Cancel purchase", "Cancelar compra"),
        purchase.quote.price.name
    ));
    ui.label(t(pt,"This removes the exact purchased trait and refunds the original XP in one transaction. The original purchase and debit stay in history.","Remove a característica comprada e reembolsa o XP original em uma transação. A compra e o débito originais permanecem no histórico."));
    ui.small(t(pt,"Only an unchanged purchase snapshot can be cancelled here. Later sheet or XP changes must be reviewed separately. A corrected replacement requires a fresh purchase quote.","Somente dados inalterados da compra podem ser cancelados aqui. Alterações posteriores da ficha ou XP exigem revisão separada. Uma compra substituta exige nova cotação."));
    ui.label(format!(
        "{}: {} XP",
        t(pt, "Original cost / refund", "Custo original / reembolso"),
        purchase.quote.price.price.total
    ));
    let before = form.cancel_input.clone();
    let r = &mut form.cancel_input;
    for (label, field) in [
        (
            t(
                pt,
                "Cancellation date YYYY-MM-DD",
                "Data do cancelamento AAAA-MM-DD",
            ),
            &mut r.day,
        ),
        (
            t(pt, "Recorder / local ST", "Responsável / Narrador local"),
            &mut r.actor,
        ),
        (t(pt, "Reason", "Motivo"), &mut r.reason),
        (
            t(
                pt,
                "Correction / approval evidence reference",
                "Referência de evidência da correção / aprovação",
            ),
            &mut r.evidence,
        ),
    ] {
        ui.label(label);
        ui.text_edit_singleline(field);
    }
    if before != form.cancel_input {
        form.dirty = true;
        form.cancel_quote = None;
        form.message = None;
    }
    if let Some(message) = &form.message {
        ui.label(message);
    }
    ui.horizontal_wrapped(|ui| {
        let b = ui.button(t(
            pt,
            "Review refund and removal",
            "Revisar reembolso e remoção",
        ));
        remember(ui, &b, "cancel_quote");
        if b.clicked() {
            action = Some(Action::QuoteCancel);
        }
        if ui
            .button(t(
                pt,
                "Discard cancellation form",
                "Descartar formulário de cancelamento",
            ))
            .clicked()
        {
            action = Some(Action::Reset);
        }
    });
    if let Some(q) = &form.cancel_quote {
        ui.strong(format!(
            "{}: {} -> {} XP",
            q.record_name, q.before_unspent, q.after_unspent
        ));
        ui.label(t(pt,"The original trait is removed; the original receipt gains a linked cancellation record.","A característica original é removida; o recibo original recebe um registro vinculado de cancelamento."));
        let current = q.revision == revision && q.request == form.cancel_input;
        if !current {
            ui.colored_label(
                egui::Color32::YELLOW,
                t(
                    pt,
                    "Stale quote; review again.",
                    "Cotação desatualizada; revise novamente.",
                ),
            );
        }
        let b = ui.add_enabled(
            current,
            egui::Button::new(t(
                pt,
                "Commit refund + removal",
                "Confirmar reembolso + remoção",
            )),
        );
        remember(ui, &b, "cancel_commit");
        if b.clicked() {
            action = Some(Action::CommitCancel);
        }
    }
    action
}

pub fn render_declarations(ui: &mut egui::Ui, request: &PurchaseRequest, pt: bool) {
    for (label, value) in [
        (t(pt, "Recorder", "Responsável"), request.actor.as_str()),
        (t(pt, "Reason", "Motivo"), request.reason.as_str()),
        (
            t(pt, "Local policy", "Regra local"),
            request.policy_reference.as_str(),
        ),
        (t(pt, "Version", "Versão"), request.policy_version.as_str()),
        (
            t(pt, "Affiliation", "Afiliação"),
            request.affiliation_name.as_str(),
        ),
        (
            t(pt, "Affiliation evidence", "Evidência da afiliação"),
            request.affiliation_evidence.as_str(),
        ),
        (
            t(pt, "Eligibility", "Elegibilidade"),
            request.eligibility_evidence.as_str(),
        ),
        (
            t(pt, "Approval", "Aprovação"),
            request.approval_reference.as_str(),
        ),
        (
            t(pt, "Teacher evidence", "Evidência do professor"),
            request.teacher_evidence.as_str(),
        ),
        (
            t(pt, "Purchase date", "Data da compra"),
            request.day.as_str(),
        ),
        (
            t(pt, "Available from", "Disponível a partir de"),
            request.available_from.as_str(),
        ),
        (
            t(pt, "Next session", "Próxima sessão"),
            request.next_session_reference.as_str(),
        ),
    ] {
        ui.label(format!("{label}: {value}"));
    }
    ui.label(format!(
        "{}: {}",
        t(pt, "Teacher ruling", "Decisão sobre professor"),
        match request.teacher_rule {
            Some(TeacherRule::Required) => t(pt, "Teacher required", "Professor exigido"),
            Some(TeacherRule::NotRequiredByLocalRuling) => t(
                pt,
                "Not required under cited local ruling",
                "Não exigido conforme decisão local citada"
            ),
            None => t(pt, "Unresolved", "Pendente"),
        }
    ));
    ui.label(format!(
        "{}: {}",
        t(pt, "Affiliation pricing", "Custo por afiliação"),
        relation_label(pt, request.relation)
    ));
}

fn relation_label(pt: bool, relation: Option<ClanRelation>) -> &'static str {
    match relation {
        Some(ClanRelation::InClan) => t(pt, "In clan", "Do clã"),
        Some(ClanRelation::OutOfClan) => t(pt, "Out of clan", "Fora do clã"),
        None => t(pt, "Affiliation unresolved", "Afiliação pendente"),
    }
}

pub fn tier_label(pt: bool, tier: MetTier) -> &'static str {
    match tier {
        MetTier::Basic => t(pt, "Basic", "Básico"),
        MetTier::Intermediate => t(pt, "Intermediate", "Intermediário"),
        MetTier::Advanced => t(pt, "Advanced", "Avançado"),
    }
}
pub fn render_sources(
    ui: &mut egui::Ui,
    preview: &grapevine_workbench::purchase_preview::AnimalismPurchasePreview,
    pt: bool,
) {
    let power = &preview.power_source.citation;
    let cost = &preview.price.citation;
    let learning = &preview.learning_source.citation;
    ui.small(format!(
        "{} · {} · {} · {} p.{}/PDF{} · {} p.{}/PDF{} · {} p.{}/PDF{}",
        power.book,
        power.edition,
        tier_label(pt, preview.tier),
        t(pt, "Power", "Poder"),
        power.printed_page,
        power.pdf_page_1based,
        t(pt, "Cost", "Custo"),
        cost.printed_page,
        cost.pdf_page_1based,
        t(pt, "Learning", "Aprendizado"),
        learning.printed_page,
        learning.pdf_page_1based
    ));
}

pub fn render_owbn_evidence(
    ui: &mut egui::Ui,
    evidence: &mut grapevine_workbench::owbn_bylaws::Evidence,
    kind: &str,
    pt: bool,
    teacher: bool,
) -> bool {
    use grapevine_workbench::owbn_bylaws::{Regulation, RegulationEvidence, Selector, builtin};
    let mut changed = false;
    egui::CollapsingHeader::new(t(
        pt,
        "OWbN policy evidence",
        "Referências dos estatutos da OWbN",
    ))
    .show(ui, |ui| {
        changed |= ui
            .checkbox(
                &mut evidence.confirm_non_binding_sources,
                t(
                    pt,
                    "Local Storyteller confirms use of non-binding sources",
                    "O Narrador local confirma o uso de fontes não vinculantes",
                ),
            )
            .changed();
        if teacher {
            ui.label(t(
                pt,
                "Coordinator or packet teacher exception reference",
                "Referência da exceção de professor do coordenador ou do pacote",
            ));
            changed |= ui
                .text_edit_singleline(&mut evidence.teacher_exception_reference)
                .changed();
        }
        let policy = builtin();
        let mut remove = None;
        for (index, item) in evidence.references.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&item.rule_id);
                    egui::ComboBox::from_id_salt("owbn_regulation")
                        .selected_text(item.regulation.label(pt))
                        .show_ui(ui, |ui| {
                            for regulation in [
                                Regulation::CoordinatorNotify,
                                Regulation::CoordinatorApproval,
                                Regulation::CouncilVote,
                            ] {
                                changed |= ui
                                    .selectable_value(
                                        &mut item.regulation,
                                        regulation,
                                        regulation.label(pt),
                                    )
                                    .changed();
                            }
                        });
                    if ui.button(t(pt, "Remove", "Remover")).clicked() {
                        remove = Some(index);
                    }
                });
                ui.label(t(pt, "Local OWbN reference", "Referência local da OWbN"));
                changed |= ui
                    .text_edit_singleline(&mut item.owbn_coordinator_reference)
                    .changed();
            });
        }
        if let Some(index) = remove {
            evidence.references.remove(index);
            changed = true;
        }
        ui.menu_button(
            t(
                pt,
                "Add regulation evidence",
                "Adicionar referência de regulamentação",
            ),
            |ui| {
                for rule in policy.rules.iter().filter(|rule| rule.kind == kind) {
                    if evidence
                        .references
                        .iter()
                        .any(|item| item.rule_id == rule.id)
                    {
                        continue;
                    }
                    let regulation = [rule.pc, rule.npc].into_iter().find(|regulation| {
                        matches!(
                            regulation,
                            Regulation::CoordinatorNotify
                                | Regulation::CoordinatorApproval
                                | Regulation::CouncilVote
                        )
                    });
                    let Some(regulation) = regulation else {
                        continue;
                    };
                    let label = match &rule.selector {
                        Selector::Creature => kind,
                        Selector::Identity { value, .. } | Selector::Power { value, .. } => value,
                    };
                    if ui.button(format!("{label} ({})", rule.id)).clicked()
                        && evidence.references.len() < 64
                    {
                        evidence.references.push(RegulationEvidence {
                            rule_id: rule.id.clone(),
                            regulation,
                            owbn_coordinator_reference: String::new(),
                        });
                        changed = true;
                        ui.close();
                    }
                }
            },
        );
    });
    changed
}
