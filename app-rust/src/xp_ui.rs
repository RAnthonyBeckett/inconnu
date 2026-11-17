use eframe::egui;
use grapevine_workbench::{
    model::Language,
    xp::{self, Baseline, CostEvidence, Ledger, XpCommand, XpKind, XpView},
};

fn text<'a>(language: Language, en: &'a str, pt: &'a str) -> &'a str {
    match language {
        Language::English => en,
        Language::Portuguese => pt,
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct XpForm {
    pub opening_earned: String,
    pub opening_unspent: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
    pub amount: String,
    pub reversal_id: String,
    pub kind: XpKind,
    pub base: String,
    pub affiliation_adjustment: String,
    pub affiliation: String,
    pub source: String,
    pub edition: String,
    pub page: String,
    pub policy_version: String,
    pub cost_enabled: bool,
    pub affiliation_name: String,
    pub power_family: String,
    pub approval_reference: String,
}

impl Default for XpForm {
    fn default() -> Self {
        Self {
            opening_earned: String::new(),
            opening_unspent: String::new(),
            day: String::new(),
            actor: String::new(),
            reason: String::new(),
            evidence: String::new(),
            amount: String::new(),
            reversal_id: String::new(),
            kind: XpKind::Award,
            base: String::new(),
            affiliation_adjustment: String::new(),
            affiliation: "unknown".into(),
            source: String::new(),
            edition: String::new(),
            page: String::new(),
            policy_version: String::new(),
            cost_enabled: false,
            affiliation_name: String::new(),
            power_family: String::new(),
            approval_reference: String::new(),
        }
    }
}

impl XpForm {
    pub fn is_dirty(&self) -> bool {
        self != &Self::default()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum XpAction {
    Initialize(Baseline),
    Post(Box<XpCommand>),
    ExportAudit,
}

pub fn render(
    ui: &mut egui::Ui,
    view: &XpView,
    form: &mut XpForm,
    language: Language,
) -> Option<XpAction> {
    ui.heading(text(language, "Experience", "Experiência"));
    ui.label(&view.record_name);
    balances(ui, view, language);

    let mut action = None;
    if view.is_build {
        ui.weak(text(
            language,
            "Build copies are read-only here. XP cannot be posted or initialized.",
            "Cópias de construção são somente leitura aqui. Não é possível registrar ou iniciar XP.",
        ));
    } else if let Some(ledger) = &view.ledger {
        action = posting(ui, ledger, form, language);
    } else {
        action = initialize(ui, form, language);
    }

    if let Some(ledger) = &view.ledger {
        timeline(ui, &ledger.entries, language);
    }
    egui::CollapsingHeader::new(text(
        language,
        "Imported XP history — unverified",
        "Histórico de XP importado — não verificado",
    ))
    .show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("legacy_xp_history")
            .max_height(240.0)
            .show(ui, |ui| {
                for event in &view.legacy_history {
                    ui.push_id(&event.id, |ui| {
                        ui.label(format!(
                            "{} · {} · {} · {}",
                            event.id, event.date, event.kind, event.amount
                        ));
                        ui.label(format!(
                            "{}: {} · {}: {}",
                            text(language, "Earned", "Conquistado"),
                            event.earned,
                            text(language, "Unspent", "Disponível"),
                            event.unspent
                        ));
                        ui.label(&event.reason);
                        ui.separator();
                    });
                }
            });
    });
    findings(ui, view, language);
    ui.separator();
    ui.label(coverage_text(language));
    if action.is_none()
        && ui
            .button(text(
                language,
                "Export audit report…",
                "Exportar relatório de auditoria…",
            ))
            .clicked()
    {
        action = Some(XpAction::ExportAudit);
    }
    action
}

fn balances(ui: &mut egui::Ui, view: &XpView, language: Language) {
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "{}: {}",
            text(language, "Earned", "Conquistado"),
            view.earned.as_deref().unwrap_or("—")
        ));
        ui.separator();
        ui.label(format!(
            "{}: {}",
            text(language, "Unspent", "Disponível"),
            view.unspent.as_deref().unwrap_or("—")
        ));
    });
    ui.label(format!(
        "{} · {}: {} · {}: {} · {}: {}",
        text(
            language,
            "Imported legacy data (unverified)",
            "Dados legados importados (não verificados)"
        ),
        text(language, "earned", "conquistado"),
        view.legacy_earned.as_deref().unwrap_or("—"),
        text(language, "unspent", "disponível"),
        view.legacy_unspent.as_deref().unwrap_or("—"),
        text(language, "entries", "registros"),
        view.legacy_history_count
    ));
}

fn initialize(ui: &mut egui::Ui, form: &mut XpForm, language: Language) -> Option<XpAction> {
    ui.separator();
    ui.heading(text(
        language,
        "Declare an opening balance",
        "Declare um saldo inicial",
    ));
    ui.label(text(
        language,
        "This is a local declaration, not an independently verified historical balance.",
        "Esta é uma declaração local, não um saldo histórico verificado de forma independente.",
    ));
    ui.horizontal(|ui| {
        field(
            ui,
            text(language, "Earned", "Conquistado"),
            &mut form.opening_earned,
        );
        field(
            ui,
            text(language, "Unspent", "Disponível"),
            &mut form.opening_unspent,
        );
    });
    evidence_fields(ui, form, language);
    let baseline = Baseline {
        day: form.day.clone(),
        earned: form.opening_earned.clone(),
        unspent: form.opening_unspent.clone(),
        actor: form.actor.clone(),
        reason: form.reason.clone(),
        evidence: form.evidence.clone(),
    };
    let valid = xp::validate_baseline(&baseline).is_ok();
    if !valid && any_baseline_input(form) {
        ui.small(text(
            language,
            "Enter valid nonnegative totals, a YYYY-MM-DD date, recorder, reason and evidence reference.",
            "Informe totais não negativos válidos, data AAAA-MM-DD, responsável, motivo e referência da evidência.",
        ));
    }
    if action_button(
        ui,
        valid,
        text(language, "Initialize ledger", "Iniciar livro XP"),
        "xp_init_probe",
    ) {
        Some(XpAction::Initialize(baseline))
    } else {
        None
    }
}

fn any_baseline_input(form: &XpForm) -> bool {
    !form.opening_earned.is_empty()
        || !form.opening_unspent.is_empty()
        || !form.day.is_empty()
        || !form.actor.is_empty()
        || !form.reason.is_empty()
        || !form.evidence.is_empty()
}

fn evidence_fields(ui: &mut egui::Ui, form: &mut XpForm, language: Language) {
    field(
        ui,
        text(
            language,
            "Effective date (YYYY-MM-DD)",
            "Data efetiva (AAAA-MM-DD)",
        ),
        &mut form.day,
    );
    field(
        ui,
        text(language, "Recorded by", "Registrado por"),
        &mut form.actor,
    );
    field(ui, text(language, "Reason", "Motivo"), &mut form.reason);
    field(
        ui,
        text(language, "Evidence reference", "Referência da evidência"),
        &mut form.evidence,
    );
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        let response = ui.text_edit_singleline(value);
        if label == "Amount" || label == "Quantidade" {
            ui.ctx().data_mut(|data| {
                data.insert_temp(
                    egui::Id::unique("xp_amount_probe"),
                    ui.clip_rect()
                        .contains_rect(response.rect)
                        .then_some(response.rect),
                )
            });
        }
    });
}

fn posting(
    ui: &mut egui::Ui,
    ledger: &Ledger,
    form: &mut XpForm,
    language: Language,
) -> Option<XpAction> {
    ui.separator();
    ui.heading(text(
        language,
        "Record a manual XP entry",
        "Registrar lançamento manual de XP",
    ));
    ui.label(text(
        language,
        "This records evidence only. It does not verify rules, calculate a purchase, or grant a trait.",
        "Este registro guarda evidências. Não verifica regras, calcula compras nem concede características.",
    ));
    egui::ComboBox::from_id_salt("xp_kind")
        .selected_text(kind_text(form.kind, language))
        .show_ui(ui, |ui| {
            for kind in [
                XpKind::Award,
                XpKind::Spend,
                XpKind::Adjustment,
                XpKind::Reversal,
            ] {
                ui.selectable_value(&mut form.kind, kind, kind_text(kind, language));
            }
        });
    evidence_fields(ui, form, language);

    let reversal_target = if form.kind == XpKind::Reversal {
        reversal_picker(ui, ledger, form, language)
    } else {
        field(ui, text(language, "Amount", "Quantidade"), &mut form.amount);
        None
    };

    if form.kind == XpKind::Spend {
        cost_evidence(ui, form, language);
    }

    let cost = (form.kind == XpKind::Spend && form.cost_enabled).then(|| CostEvidence {
        affiliation_name: form.affiliation_name.clone(),
        power_family: form.power_family.clone(),
        approval_reference: form.approval_reference.clone(),
        base: form.base.clone(),
        affiliation_adjustment: form.affiliation_adjustment.clone(),
        affiliation: form.affiliation.clone(),
        source: form.source.clone(),
        edition: form.edition.clone(),
        page: form.page.clone(),
        policy_version: form.policy_version.clone(),
    });
    let amount = reversal_target
        .map(|target| target.command.amount.clone())
        .unwrap_or_else(|| form.amount.clone());
    let candidate = XpCommand {
        id: "draft-entry".into(),
        kind: form.kind,
        amount,
        day: form.day.clone(),
        actor: form.actor.clone(),
        reason: form.reason.clone(),
        evidence: form.evidence.clone(),
        reversal_of: (form.kind == XpKind::Reversal)
            .then(|| form.reversal_id.clone())
            .filter(|id| !id.is_empty()),
        cost,
    };
    let valid = reversal_target.is_some() || form.kind != XpKind::Reversal;
    let valid = valid && xp::canonical_command(ledger, &candidate).is_ok();
    if !valid && any_entry_input(form) {
        ui.small(text(
            language,
            "Check the date, required evidence and amount; reversals need an eligible original entry.",
            "Confira a data, as evidências obrigatórias e a quantidade; estornos exigem um lançamento original elegível.",
        ));
    }
    if action_button(
        ui,
        valid,
        text(language, "Post manual entry", "Registrar lançamento manual"),
        "xp_post_probe",
    ) {
        let mut command = candidate;
        command.id = uuid::Uuid::new_v4().to_string();
        Some(XpAction::Post(Box::new(command)))
    } else {
        None
    }
}

fn any_entry_input(form: &XpForm) -> bool {
    !form.day.is_empty()
        || !form.actor.is_empty()
        || !form.reason.is_empty()
        || !form.evidence.is_empty()
        || !form.amount.is_empty()
        || !form.reversal_id.is_empty()
}

fn reversal_picker<'a>(
    ui: &mut egui::Ui,
    ledger: &'a Ledger,
    form: &mut XpForm,
    language: Language,
) -> Option<&'a grapevine_workbench::xp::Entry> {
    egui::ComboBox::from_id_salt("xp_reversal_target")
        .selected_text(
            reversal_label(ledger, &form.reversal_id).unwrap_or_else(|| {
                text(
                    language,
                    "Select original entry",
                    "Selecione o lançamento original",
                )
                .to_owned()
            }),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut form.reversal_id,
                String::new(),
                text(
                    language,
                    "Select original entry",
                    "Selecione o lançamento original",
                ),
            );
            for entry in ledger
                .entries
                .iter()
                .filter(|entry| entry.command.kind != XpKind::Reversal)
            {
                let label = format!(
                    "{} · {} · {}",
                    entry.command.id, entry.command.day, entry.command.amount
                );
                ui.selectable_value(&mut form.reversal_id, entry.command.id.clone(), label);
            }
        });
    ledger.entries.iter().find(|entry| {
        entry.command.id == form.reversal_id && entry.command.kind != XpKind::Reversal
    })
}

fn reversal_label(ledger: &Ledger, id: &str) -> Option<String> {
    ledger
        .entries
        .iter()
        .find(|entry| entry.command.id == id && entry.command.kind != XpKind::Reversal)
        .map(|entry| {
            format!(
                "{} · {} · {}",
                entry.command.id, entry.command.day, entry.command.amount
            )
        })
}

fn cost_evidence(ui: &mut egui::Ui, form: &mut XpForm, language: Language) {
    ui.collapsing(text(language, "Optional purchase evidence", "Evidência opcional da compra"), |ui| {
        ui.checkbox(&mut form.cost_enabled, text(language, "Record declared cost components", "Registrar componentes de custo declarados"));
        if !form.cost_enabled { return; }
            field(
                ui,
                text(language, "Affiliation at purchase", "Afiliação na compra"),
                &mut form.affiliation_name,
            );
            field(
                ui,
                text(
                    language,
                    "Power family / purchase",
                    "Grupo de poderes / compra",
                ),
                &mut form.power_family,
            );
            field(
                ui,
                text(
                    language,
                    "Approval or exception reference",
                    "Referência de aprovação ou exceção",
                ),
                &mut form.approval_reference,
            );

        field(ui, text(language, "Base cost", "Custo base"), &mut form.base);
        field(ui, text(language, "Affiliation adjustment", "Ajuste de afiliação"), &mut form.affiliation_adjustment);
        egui::ComboBox::from_id_salt("xp_affiliation")
            .selected_text(affiliation_text(&form.affiliation, language))
            .show_ui(ui, |ui| {
                for (value, label) in [
                    ("in", text(language, "In affiliation", "Da afiliação")),
                    ("out", text(language, "Outside affiliation", "Fora da afiliação")),
                    ("unknown", text(language, "Unknown", "Desconhecida")),
                ] {
                    ui.selectable_value(&mut form.affiliation, value.into(), label);
                }
            });
        field(ui, text(language, "Source", "Fonte"), &mut form.source);
        field(ui, text(language, "Edition", "Edição"), &mut form.edition);
        field(ui, text(language, "Page", "Página"), &mut form.page);
        field(ui, text(language, "Policy version", "Versão da política"), &mut form.policy_version);
        match xp::amount(&form.base).ok().and_then(|base| {
            xp::amount(&form.affiliation_adjustment).ok().and_then(|adjustment| base.checked_add(adjustment))
        }) {
            Some(total) => {
                ui.label(format!("{}: {}", text(language, "Declared total", "Total declarado"), xp::decimal(total)));
                if xp::amount(&form.amount).ok() != Some(total) {
                    ui.weak(text(language, "This is a manual comparison; no cost rule is being applied.", "Esta é uma comparação manual; nenhuma regra de custo está sendo aplicada."));
                }
            }
            None => {ui.weak(text(language, "Enter valid component amounts to preview their sum.", "Informe valores válidos para pré-visualizar a soma."));},
        }
        ui.small(text(language, "Declared evidence is not independently verified; no universal affiliation surcharge is assumed.", "As evidências declaradas não foram verificadas de forma independente; nenhum adicional universal de afiliação é presumido."));
    });
}

fn affiliation_text(value: &str, language: Language) -> &str {
    match value {
        "in" => text(language, "In affiliation", "Da afiliação"),
        "out" => text(language, "Outside affiliation", "Fora da afiliação"),
        _ => text(language, "Unknown", "Desconhecida"),
    }
}

fn kind_text(kind: XpKind, language: Language) -> &'static str {
    match kind {
        XpKind::Award => text(language, "Award", "Crédito"),
        XpKind::Spend => text(language, "Spending", "Gasto"),
        XpKind::Adjustment => text(language, "Signed adjustment", "Ajuste com sinal"),
        XpKind::Reversal => text(language, "Reversal", "Estorno"),
    }
}

fn timeline(ui: &mut egui::Ui, entries: &[grapevine_workbench::xp::Entry], language: Language) {
    ui.separator();
    ui.heading(text(
        language,
        "XP timeline — local Storyteller view",
        "Linha do tempo de XP — visão local de Storyteller",
    ));
    if entries.is_empty() {
        ui.label(text(
            language,
            "No ledger entries yet.",
            "Ainda não há lançamentos.",
        ));
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("xp_timeline")
        .max_height(260.0)
        .show(ui, |ui| {
            for entry in entries {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!(
                        "{} · {}",
                        entry.command.day,
                        kind_text(entry.command.kind, language)
                    ));
                    ui.label(format!(
                        "{}: {}",
                        text(language, "Amount", "Quantidade"),
                        entry.command.amount
                    ));
                    ui.small(format!(
                        "{}: {}",
                        text(language, "Recorded", "Registrado"),
                        unix_date(entry.recorded_unix_seconds)
                    ));
                });
                ui.label(format!(
                    "{}: {}",
                    text(language, "Reason", "Motivo"),
                    entry.command.reason
                ));
                ui.small(format!(
                    "{}: {}",
                    text(language, "Evidence", "Evidência"),
                    grapevine_workbench::trait_advancement::evidence_summary(
                        &entry.command,
                        language == Language::Portuguese
                    )
                    .unwrap_or_else(|| entry.command.evidence.clone())
                ));
                ui.separator();
            }
        });
}

fn unix_date(seconds: u64) -> String {
    let z = (seconds / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} UTC")
}

fn findings(ui: &mut egui::Ui, view: &XpView, language: Language) {
    ui.separator();
    ui.heading(text(
        language,
        "Audit findings",
        "Constatações da auditoria",
    ));
    for finding in &view.findings {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(severity_text(&finding.severity, language));
                ui.label(&finding.code);
                if let Some(id) = &finding.entry_id {
                    ui.small(format!("{}: {id}", text(language, "Entry", "Lançamento")));
                }
            });
            ui.label(finding_message(&finding.code, &finding.message, language));
        });
    }
}

fn severity_text(severity: &str, language: Language) -> &str {
    match severity {
        "discrepancy" => text(language, "Discrepancy", "Divergência"),
        "review" => text(language, "Review", "Revisão"),
        _ => text(language, "Unverified", "Não verificado"),
    }
}

fn finding_message(code: &str, original: &str, language: Language) -> String {
    if language == Language::English {
        return original.to_owned();
    }
    let translated = match code {
        "AUTHORIZATION_UNVERIFIED" => {
            "Declarações locais e histórico importado não comprovam autorização externa de crédito ou compra."
        }
        "DECLARED_BASELINE" => {
            "O saldo inicial é uma declaração local; a cobertura anterior e as evidências externas precisam de revisão."
        }
        "MANUAL_ADJUSTMENT" => {
            "O ajuste manual com sinal exige revisão do motivo e das evidências."
        }
        "POSSIBLE_DUPLICATE_AWARD" => {
            "Outro crédito tem a mesma data efetiva, quantidade e referência de evidência; confira se ambos eram pretendidos."
        }
        "COST_UNVERIFIED" => {
            "O gasto registrado não contém evidência estruturada de custo base, ajuste de afiliação e fonte; não há validação automática de custo."
        }
        "COST_MISMATCH" => {
            "O gasto registrado difere da soma declarada do custo base e do ajuste de afiliação."
        }
        "AFFILIATION_COST_UNVERIFIED" => {
            "A afiliação ou a evidência de fonte/edição/página/política está incompleta; o custo base não deve ser presumido correto."
        }
        "DECLARED_COST_POLICY" => {
            "Os componentes de custo declarados não foram verificados de forma independente contra regras ou aprovação organizacional."
        }
        "OPENING_REQUIRED" => {
            "Não há livro XP local exato aberto; revise o histórico importado antes de declarar um saldo inicial."
        }
        "LEGACY_HISTORY_UNVERIFIED" => {
            "Os eventos importados foram preservados como evidência; a aritmética e a autorização precisam de revisão separada."
        }
        "LEGACY_NEGATIVE_SNAPSHOT" => {
            "O histórico importado registra saldo conquistado ou disponível negativo."
        }
        "XP_NEGATIVE_HISTORY" => {
            "Este lançamento criaria um saldo negativo ou inconsistente no histórico."
        }
        "XP_ALREADY_REVERSED" => "Um lançamento só pode ser estornado uma vez.",
        "XP_REVERSAL_TARGET" => {
            "Selecione um lançamento original elegível e registre o estorno na data efetiva ou depois dela."
        }
        "XP_CORRUPT_LEDGER" => {
            "O livro contém um lançamento inválido ou duplicado; preserve-o para revisão."
        }
        "INVALID_XP" => {
            "Informe uma quantidade decimal válida, sem arredondamento e dentro dos limites permitidos."
        }
        "XP_LIMIT" => "A quantidade ou o saldo excede os limites de armazenamento.",
        "XP_EVIDENCE_REQUIRED" => {
            "Informe ID, data válida, responsável, motivo e referência da evidência."
        }
        "INVALID_XP_BASELINE" => {
            "Os saldos iniciais devem ser não negativos, com saldo disponível menor ou igual ao conquistado."
        }
        _ => return original.to_owned(),
    };
    translated.to_owned()
}

fn coverage_text(language: Language) -> &'static str {
    text(
        language,
        "Coverage: local arithmetic and linked purchase receipts where present. External authorization and pre-opening completeness remain unverified. Manual XP entries do not grant traits.",
        "Cobertura: aritmética local e recibos de compra vinculados quando presentes. Autorização externa e histórico anterior ao saldo inicial permanecem não verificados. Registros manuais de XP não concedem características.",
    )
}

fn action_button(ui: &mut egui::Ui, enabled: bool, label: &str, probe: &str) -> bool {
    let response = ui.add_enabled(enabled, egui::Button::new(label));
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            egui::Id::unique(probe),
            ui.clip_rect()
                .contains_rect(response.rect)
                .then_some(response.rect),
        )
    });
    response.clicked()
}
