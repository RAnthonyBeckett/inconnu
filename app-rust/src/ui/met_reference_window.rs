use super::*;
use grapevine_workbench::{
    met_references as met,
    power_catalogue::{self, CatalogueEntry},
    pricing::MetTier,
};

#[derive(Default)]
pub(super) struct MetWindow {
    pub open: bool,
    pub target: Option<String>,
    pub context: Option<met::Context>,
    pub request: met::Request,
    pub quote: Option<met::Quote>,
    pub dirty: bool,
    pub message: Option<String>,
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

#[derive(Clone, Copy)]
enum Action {
    Reload,
    Preview,
    PreviewClear,
    Commit,
}

fn value_text(row: &serde_json::Value, field: &str, pt: bool) -> String {
    match row.get(field) {
        Some(value) if value.is_string() => value.as_str().unwrap_or_default().to_owned(),
        Some(value) => value.to_string(),
        None => t(pt, "not recorded", "não registrado").to_owned(),
    }
}

fn row_name(row: &serde_json::Value) -> &str {
    row.get("Name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
}

fn has_selected_met_tier(row: &serde_json::Value, power: &CatalogueEntry) -> bool {
    row.get("Value")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(power.tier.sheet_value()))
}

fn tier_label(tier: MetTier, pt: bool) -> &'static str {
    match tier {
        MetTier::Basic => t(pt, "Basic", "Básico"),
        MetTier::Intermediate => t(pt, "Intermediate", "Intermediário"),
        MetTier::Advanced => t(pt, "Advanced", "Avançado"),
    }
}

fn selected_power<'a>(powers: &'a [CatalogueEntry], power_id: &str) -> Option<&'a CatalogueEntry> {
    powers.iter().find(|power| power.stable_id == power_id)
}

fn metadata_complete(request: &met::Request) -> bool {
    !request.day.trim().is_empty()
        && !request.actor.trim().is_empty()
        && !request.reason.trim().is_empty()
        && !request.review_reference.trim().is_empty()
        && request.confirmed
}

fn mapping_complete(
    request: &met::Request,
    context: &met::Context,
    powers: &[CatalogueEntry],
) -> bool {
    if request.clear {
        return false;
    }
    let Some(row) = context.traits.get(request.trait_index) else {
        return false;
    };
    let Some(power) = selected_power(powers, &request.power_id) else {
        return false;
    };
    metadata_complete(request)
        && (row_name(row) == power.name || request.confirm_alias)
        && (has_selected_met_tier(row, power) || request.confirm_legacy_value)
}

fn quote_preserves_trait_values(quote: &met::Quote, row: &serde_json::Value) -> bool {
    row.get("Name") == quote.updated_trait.get("Name")
        && row.get("Value") == quote.updated_trait.get("Value")
}

fn quote_is_fresh(
    state: &MetWindow,
    context: &met::Context,
    quote: &met::Quote,
    current_revision: i64,
) -> bool {
    quote.record_id == context.record_id
        && quote.revision == context.revision
        && quote.revision == current_revision
        && state.request == quote.request
        && context
            .traits
            .get(quote.request.trait_index)
            .is_some_and(|row| quote_preserves_trait_values(quote, row))
}

fn render_quote(ui: &mut egui::Ui, quote: &met::Quote, powers: &[CatalogueEntry], pt: bool) {
    ui.separator();
    if quote.request.clear {
        ui.strong(t(
            pt,
            "Removal preview · the existing trait row remains",
            "Prévia da remoção · a característica existente será mantida",
        ));
        ui.label(format!("{} · {}", quote.request.day, quote.request.actor));
        ui.label(&quote.request.reason);
        ui.label(&quote.request.review_reference);
        ui.weak(t(
            pt,
            "Only the MET reminder mapping is removed. The saved name, raw value, XP and trait count stay unchanged.",
            "Somente o vínculo do lembrete MET será removido. Nome salvo, valor bruto, XP e quantidade de características permanecem iguais.",
        ));
        return;
    }

    let power = selected_power(powers, &quote.request.power_id);
    ui.strong(format!(
        "{} · {}",
        t(pt, "Quoted MET power", "Poder MET cotado"),
        quote.power_name
    ));
    if let Some(power) = power {
        ui.label(format!(
            "{} · {} · {}",
            power.family,
            tier_label(power.tier, pt),
            power.stable_id
        ));
    } else {
        ui.colored_label(
            egui::Color32::YELLOW,
            t(
                pt,
                "The quoted power is not in the current reviewed catalogue.",
                "O poder cotado não consta no catálogo revisado atual.",
            ),
        );
    }
    if let Some(citation) = &quote.citation {
        ui.small(format!(
            "{} · {} · {} · {} {} / PDF {} · SHA-256 {}",
            citation.book,
            citation.edition,
            citation.system,
            t(pt, "printed page", "página impressa"),
            citation.printed_page,
            citation.pdf_page_1based,
            citation.original_pdf_sha256
        ));
    } else {
        ui.colored_label(
            egui::Color32::YELLOW,
            t(
                pt,
                "No source citation is attached to this quote.",
                "Esta cotação não tem citação de fonte.",
            ),
        );
    }
    ui.label(format!(
        "{}: {} · {}",
        t(pt, "Saved trait remains", "Característica salva permanece"),
        value_text(&quote.updated_trait, "Name", pt),
        value_text(&quote.updated_trait, "Value", pt)
    ));
    ui.label(format!(
        "{} · {} · {}",
        quote.request.day, quote.request.actor, quote.request.reason
    ));
    ui.label(&quote.request.review_reference);
    ui.weak(t(
        pt,
        "This records a source-based mechanics reminder only. It is not automatic eligibility, purchase approval, a trait grant, an XP charge, or a name/value change.",
        "Isto registra somente um lembrete de regras vinculado à fonte. Não aprova automaticamente elegibilidade ou compra, não concede característica, não cobra XP e não altera nome/valor.",
    ));
}

impl InconnuApp {
    pub(super) fn open_met_reference(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if self.met_reference.target.as_ref() == Some(&id) {
            self.met_reference.open = true;
            if self.met_reference.context.is_none() {
                self.load_met_reference_context();
            }
            return;
        }
        self.met_reference = MetWindow {
            open: true,
            target: Some(id),
            ..Default::default()
        };
        self.load_met_reference_context();
    }

    fn load_met_reference_context(&mut self) {
        self.met_reference.quote = None;
        let (Some(lib), Some(id)) = (&self.library, &self.met_reference.target) else {
            self.met_reference.context = None;
            return;
        };
        match lib.met_reference_context(id, Role::Storyteller) {
            Ok(context) => {
                self.met_reference.context = Some(context);
                self.met_reference.message = None;
            }
            Err(error) => {
                self.met_reference.context = None;
                self.met_reference.message = Some(error.message);
            }
        }
    }

    fn reload_met_reference(&mut self) {
        self.met_reference.request = met::Request::default();
        self.met_reference.quote = None;
        self.met_reference.dirty = false;
        self.load_met_reference_context();
    }

    fn met_reference_action(&mut self, action: Action) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if matches!(action, Action::Reload) {
            self.reload_met_reference();
            return;
        }
        if self.edit != self.saved_edit
            || self.sheet_edit != self.saved_sheet
            || (self.xp_form.is_dirty() && self.xp_form != self.xp_saved_form)
        {
            self.met_reference.message = Some(
                self.t(
                    "Save or discard sheet/XP drafts before reviewing a MET reminder.",
                    "Salve ou descarte edições de ficha/XP antes de revisar um lembrete MET.",
                )
                .into(),
            );
            return;
        }
        let Some(id) = self.met_reference.target.clone() else {
            return;
        };
        let Some(expected_revision) = self.met_reference.context.as_ref().map(|c| c.revision)
        else {
            return;
        };

        match action {
            Action::Preview | Action::PreviewClear => {
                let kind = self
                    .selected_record
                    .as_ref()
                    .map(|r| r.kind.as_str())
                    .unwrap_or("Vampire");
                let powers: Vec<_> = power_catalogue::entries()
                    .into_iter()
                    .filter(|p| p.template_scope == kind)
                    .collect();
                let ready = self.met_reference.context.as_ref().is_some_and(|context| {
                    if matches!(action, Action::PreviewClear) {
                        self.met_reference.request.clear
                            && self.met_reference.request.power_id.is_empty()
                            && context
                                .traits
                                .get(self.met_reference.request.trait_index)
                                .is_some()
                            && metadata_complete(&self.met_reference.request)
                    } else {
                        !self.met_reference.request.clear
                            && !self.met_reference.request.power_id.is_empty()
                            && mapping_complete(&self.met_reference.request, context, &powers)
                    }
                });
                if !ready {
                    self.met_reference.message = Some(
                        self.t(
                            "Complete the selected-row, power, review and confirmation fields for the chosen operation.",
                            "Preencha os campos de característica, poder, revisão e confirmação da operação escolhida.",
                        )
                        .into(),
                    );
                    return;
                }
                let request = self.met_reference.request.clone();
                self.met_reference.quote = None;
                let result = match self.library.as_mut() {
                    Some(lib) => {
                        lib.quote_met_reference(&id, &request, expected_revision, Role::Storyteller)
                    }
                    None => {
                        self.met_reference.message = Some(
                            self.t(
                                "Open the library before reviewing a MET reminder.",
                                "Abra a biblioteca antes de revisar um lembrete MET.",
                            )
                            .into(),
                        );
                        return;
                    }
                };
                match result {
                    Ok(quote) => {
                        self.met_reference.quote = Some(quote);
                        self.met_reference.message = None;
                        self.met_reference.dirty = true;
                    }
                    Err(error) => self.met_reference.message = Some(error.message),
                }
            }
            Action::Commit => {
                let Some(quote) = self.met_reference.quote.clone() else {
                    return;
                };
                let current_revision = self
                    .library
                    .as_ref()
                    .and_then(|lib| lib.revision().ok())
                    .unwrap_or(-1);
                let context = self.met_reference.context.clone();
                let fresh = context.as_ref().is_some_and(|context| {
                    quote_is_fresh(&self.met_reference, context, &quote, current_revision)
                });
                if !fresh {
                    self.met_reference.quote = None;
                    self.met_reference.message = Some(
                        self.t(
                            "The library or request changed. Reload and review the MET reminder again.",
                            "A biblioteca ou a solicitação mudou. Recarregue e revise novamente o lembrete MET.",
                        )
                        .into(),
                    );
                    return;
                }
                let result = match self.library.as_mut() {
                    Some(lib) => lib.commit_met_reference(&quote, Role::Storyteller),
                    None => {
                        self.met_reference.message = Some(
                            self.t(
                                "Open the library before saving a MET reminder.",
                                "Abra a biblioteca antes de salvar um lembrete MET.",
                            )
                            .into(),
                        );
                        return;
                    }
                };
                match result {
                    Ok(_) => {
                        self.revision = self.library.as_ref().and_then(|lib| lib.revision().ok());
                        let was_clear = quote.request.clear;
                        self.met_reference.dirty = false;
                        self.met_reference.quote = None;
                        self.met_reference.request = met::Request::default();
                        self.refresh_summaries(true);
                        self.load_met_reference_context();
                        self.met_reference.message = Some(
                            self.t(
                                if was_clear {
                                    "MET reminder mapping removed. The existing trait and XP are unchanged."
                                } else {
                                    "MET reminder saved. The existing trait and XP are unchanged."
                                },
                                if was_clear {
                                    "Vínculo do lembrete MET removido. A característica existente e o XP não mudaram."
                                } else {
                                    "Lembrete MET salvo. A característica existente e o XP não mudaram."
                                },
                            )
                            .into(),
                        );
                    }
                    Err(error) => self.met_reference.message = Some(error.message),
                }
            }
            Action::Reload => {}
        }
    }

    pub(super) fn render_met_reference(&mut self, ctx: &egui::Context) {
        if !self.met_reference.open {
            return;
        }
        if self.route != WorkArea::Storyteller || self.met_reference.target != self.selected_id {
            self.met_reference = Default::default();
            return;
        }

        let pt = self.language == Language::Portuguese;
        let powers = power_catalogue::entries();
        let current_revision = self
            .library
            .as_ref()
            .and_then(|lib| lib.revision().ok())
            .unwrap_or(-1);
        let context = self.met_reference.context.clone();
        let message = self.met_reference.message.clone();
        let ready_to_preview = context
            .as_ref()
            .is_some_and(|context| mapping_complete(&self.met_reference.request, context, &powers));
        let ready_to_clear = context.as_ref().is_some_and(|context| {
            self.met_reference.request.clear
                && self.met_reference.request.power_id.is_empty()
                && context
                    .traits
                    .get(self.met_reference.request.trait_index)
                    .is_some()
                && metadata_complete(&self.met_reference.request)
        });
        let quote_fresh = context.as_ref().is_some_and(|context| {
            self.met_reference.quote.as_ref().is_some_and(|quote| {
                quote_is_fresh(&self.met_reference, context, quote, current_revision)
            })
        });
        let quote_preserves_values = context
            .as_ref()
            .and_then(|context| {
                self.met_reference.quote.as_ref().and_then(|quote| {
                    context
                        .traits
                        .get(quote.request.trait_index)
                        .map(|row| (quote, row))
                })
            })
            .is_some_and(|(quote, row)| quote_preserves_trait_values(quote, row));
        let mut open = true;
        let mut action = None;
        egui::Window::new(t(
            pt,
            "MET Laws of the Night Revised reference",
            "Referência MET Laws of the Night Revised",
        ))
        .id(egui::Id::unique("met_reference_window"))
        .default_size([760.0, 650.0])
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let preview = ui.add_enabled(
                    ready_to_preview,
                    egui::Button::new(t(pt, "Preview mapping", "Prévia do vínculo")),
                );
                remember(ui, &preview, "met_reference_preview");
                if preview.clicked() {
                    action = Some(Action::Preview);
                }
                let clear = ui.add_enabled(
                    ready_to_clear,
                    egui::Button::new(t(pt, "Preview removal", "Prévia da remoção")),
                );
                remember(ui, &clear, "met_reference_clear");
                if clear.clicked() {
                    action = Some(Action::PreviewClear);
                }
                let commit = ui.add_enabled(
                    quote_fresh && quote_preserves_values,
                    egui::Button::new(t(pt, "Save reviewed reminder", "Salvar lembrete revisado")),
                );
                remember(ui, &commit, "met_reference_commit");
                if commit.clicked() {
                    action = Some(Action::Commit);
                }
                if ui
                    .button(t(pt, "Reload / discard draft", "Recarregar / descartar rascunho"))
                    .clicked()
                {
                    action = Some(Action::Reload);
                }
            });
            if let Some(message) = &message {
                ui.label(message);
            }
            egui::ScrollArea::vertical()
                .id_salt("met_reference_scroll")
                .show(ui, |ui| {
                    let state = &mut self.met_reference;
                    ui.label(t(
                        pt,
                        "Attach one exact MET power to an existing Discipline row in an original Vampire character. Imported and active characters are allowed. This adds a mechanics reminder only; it does not grant a trait, charge XP, rename a row, or change its saved value.",
                        "Associe um poder MET específico a uma linha de Disciplina já existente em uma ficha Vampire original. Personagens importados e ativos são aceitos. Isto adiciona apenas um lembrete de regras; não concede característica, cobra XP, renomeia a linha nem altera o valor salvo.",
                    ));
                    let Some(context) = &context else {
                        return;
                    };
                    ui.strong(&context.record_name);

                    let before = state.request.clone();
                    if ui
                        .checkbox(
                            &mut state.request.clear,
                            t(
                                pt,
                                "Remove the MET reminder from this existing row",
                                "Remover o lembrete MET desta linha existente",
                            ),
                        )
                        .changed()
                    {
                        state.request.power_id.clear();
                        state.request.confirmed = false;
                        state.request.confirm_alias = false;
                        state.request.confirm_legacy_value = false;
                    }
                    ui.label(t(pt, "Existing Discipline row", "Linha de Disciplina existente"));
                    if context.traits.is_empty() {
                        ui.weak(t(
                            pt,
                            "No existing Discipline rows are available. This window does not add traits.",
                            "Não há linhas de Disciplina existentes. Esta janela não adiciona características.",
                        ));
                    } else {
                        state.request.trait_index = state.request.trait_index.min(context.traits.len() - 1);
                        let selected_text = context
                            .traits
                            .get(state.request.trait_index)
                            .map(|row| format!("{} · {}", row_name(row), value_text(row, "Value", pt)))
                            .unwrap_or_else(|| t(pt, "Select a row", "Selecione uma linha").into());
                        egui::ComboBox::from_id_salt("met_reference_trait")
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for (index, row) in context.traits.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut state.request.trait_index,
                                        index,
                                        format!(
                                            "{} · {}",
                                            row_name(row),
                                            value_text(row, "Value", pt)
                                        ),
                                    );
                                }
                            });
                    }

                    ui.label(t(pt, "Canonical MET power and tier", "Poder e nível MET canônicos"));
                    let selected = selected_power(&powers, &state.request.power_id);
                    let power_label = selected
                        .map(|power| {
                            format!(
                                "{} · {} · {}",
                                power.family,
                                power.name,
                                tier_label(power.tier, pt)
                            )
                        })
                        .unwrap_or_else(|| t(pt, "Choose one of the reviewed powers", "Escolha um dos poderes revisados").into());
                    ui.add_enabled_ui(!state.request.clear, |ui| {
                        egui::ComboBox::from_id_salt("met_reference_power")
                            .selected_text(power_label)
                            .show_ui(ui, |ui| {
                                for power in &powers {
                                    ui.selectable_value(
                                        &mut state.request.power_id,
                                        power.stable_id.clone(),
                                        format!(
                                            "{} · {} · {}",
                                            power.family,
                                            power.name,
                                            tier_label(power.tier, pt)
                                        ),
                                    );
                                }
                            });
                    });
                    if let Some(power) = selected {
                        ui.small(format!(
                            "{} · {}",
                            power.name,
                            tier_label(power.tier, pt)
                        ));
                        if let Some(source) = power.source {
                            ui.small(format!(
                                "{} {} / PDF {}",
                                t(pt, "Printed page", "Página impressa"),
                                source.printed_page,
                                source.pdf_page_1based
                            ));
                        }
                        ui.weak(t(
                            pt,
                            "The citation identifies source text; local prerequisites, eligibility and approval remain separate.",
                            "A citação identifica o texto da fonte; pré-requisitos, elegibilidade e aprovação locais continuam separados.",
                        ));
                    }

                    if let Some(row) = context.traits.get(state.request.trait_index) {
                        ui.label(format!(
                            "{}: {} · {}",
                            t(pt, "Stored row, unchanged", "Linha salva, sem alterações"),
                            row_name(row),
                            value_text(row, "Value", pt)
                        ));
                        if !state.request.clear {
                            if let Some(power) = selected {
                                if row_name(row) != power.name {
                                    ui.checkbox(
                                        &mut state.request.confirm_alias,
                                        t(
                                            pt,
                                            "I reviewed the different saved name as an alias; keep the saved name unchanged.",
                                            "Revisei o nome salvo diferente como alias; mantenha o nome salvo sem alterações.",
                                        ),
                                    );
                                } else {
                                    state.request.confirm_alias = false;
                                }
                                if !has_selected_met_tier(row,power) {
                                    ui.checkbox(
                                        &mut state.request.confirm_legacy_value,
                                        t(
                                            pt,
                                            "I reviewed this non-tier stored value; preserve it exactly.",
                                            "Revisei este valor salvo que não é um nível MET; preserve-o exatamente.",
                                        ),
                                    );
                                } else {
                                    state.request.confirm_legacy_value = false;
                                }
                            }
                        } else {
                            state.request.confirm_alias = false;
                            state.request.confirm_legacy_value = false;
                        }
                    }

                    ui.separator();
                    for (caption, value) in [
                        (
                            t(pt, "Review date (YYYY-MM-DD)", "Data da revisão (AAAA-MM-DD)"),
                            &mut state.request.day,
                        ),
                        (t(pt, "Reviewer", "Responsável pela revisão"), &mut state.request.actor),
                        (t(pt, "Reason", "Motivo"), &mut state.request.reason),
                        (
                            t(pt, "Review reference", "Referência da revisão"),
                            &mut state.request.review_reference,
                        ),
                    ] {
                        ui.label(caption);
                        ui.text_edit_singleline(value);
                    }
                    ui.checkbox(
                        &mut state.request.confirmed,
                        t(
                            pt,
                            "I reviewed the selected row and the exact MET power/tier change or removal. This does not approve a purchase or grant a trait.",
                            "Revisei a linha selecionada e a alteração ou remoção exata do poder/nível MET. Isto não aprova uma compra nem concede característica.",
                        ),
                    );
                    ui.weak(t(
                        pt,
                        "A source citation establishes what the book says. Prerequisites, local eligibility, organizational approval and chronicle policy remain separate review questions.",
                        "A citação estabelece o que o livro diz. Pré-requisitos, elegibilidade local, aprovação organizacional e política da crônica continuam sendo questões separadas.",
                    ));

                    let target_changed = state.request.trait_index != before.trait_index
                        || state.request.power_id != before.power_id
                        || state.request.clear != before.clear;
                    if target_changed {
                        state.request.confirmed = false;
                        state.request.confirm_alias = false;
                        state.request.confirm_legacy_value = false;
                    }
                    let request_changed = state.request.id != before.id
                        || state.request.trait_index != before.trait_index
                        || state.request.power_id != before.power_id
                        || state.request.day != before.day
                        || state.request.actor != before.actor
                        || state.request.reason != before.reason
                        || state.request.review_reference != before.review_reference
                        || state.request.confirmed != before.confirmed
                        || state.request.confirm_alias != before.confirm_alias
                        || state.request.confirm_legacy_value != before.confirm_legacy_value
                        || state.request.clear != before.clear;
                    if request_changed {
                        state.quote = None;
                        state.dirty = true;
                    }

                    if let Some(quote) = &state.quote {
                        render_quote(ui, quote, &powers, pt);
                        if !quote_preserves_values {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                t(
                                    pt,
                                    "The quote changes the stored name or value, so it cannot be saved here.",
                                    "A cotação altera o nome ou valor salvo e não pode ser salva aqui.",
                                ),
                            );
                        }
                        if !quote_fresh {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                t(
                                    pt,
                                    "The library changed after this quote. Reload and review again.",
                                    "A biblioteca mudou após esta cotação. Recarregue e revise novamente.",
                                ),
                            );
                        }
                    }

                    egui::CollapsingHeader::new(t(
                        pt,
                        "Private MET reminder review history",
                        "Histórico privado de revisões dos lembretes MET",
                    ))
                    .show(ui, |ui| {
                        for receipt in context.history.iter().rev() {
                            let quote = &receipt.quote;
                            ui.label(format!(
                                "{} · {} · r{}",
                                quote.request.day, quote.request.actor, receipt.committed_revision
                            ));
                            ui.label(&quote.request.reason);
                            ui.label(&quote.request.review_reference);
                            if quote.request.clear {
                                ui.label(t(pt, "MET reminder removed", "Lembrete MET removido"));
                            } else {
                                ui.label(format!(
                                    "{} · {}",
                                    quote.power_name,
                                    quote.request.power_id
                                ));
                                if let Some(citation) = &quote.citation {
                                    ui.small(format!(
                                        "{} · {} {} / PDF {}",
                                        citation.book,
                                        t(pt, "printed page", "página impressa"),
                                        citation.printed_page,
                                        citation.pdf_page_1based
                                    ));
                                }
                            }
                        }
                    });
                });
        });
        self.met_reference.open = open;
        if let Some(action) = action {
            self.met_reference_action(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_mapping_language_guard_and_save_keep_original_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("met-window.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let id = lib
            .create_character("Vampire", "Legacy source", 0, Role::Storyteller)
            .unwrap();
        let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
        edit.lists.get_mut("DisciplineList").unwrap()["Traits"] =
            serde_json::json!([{"Name":"Animal speech","Value":"1","Note":"PRIVATE_MET_UI_NOTE"}]);
        lib.save_sheet(&id, &edit, lib.revision().unwrap(), Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_met_reference();
        app.met_reference.request = met::Request {
            power_id: grapevine_workbench::purchase_preview::ANIMALISM_FERAL_WHISPERS_ID.into(),
            day: "2026-11-14".into(),
            actor: "PRIVATE_MET_UI_ACTOR".into(),
            reason: "Synthetic review".into(),
            review_reference: "Synthetic legacy meaning and source review".into(),
            confirmed: true,
            confirm_alias: true,
            confirm_legacy_value: true,
            ..Default::default()
        };
        app.met_reference.dirty = true;
        app.met_reference_action(Action::Preview);
        let quote = app.met_reference.quote.clone().unwrap();
        let request = app.met_reference.request.clone();
        let ctx = egui::Context::default();
        for language in [Language::English, Language::Portuguese] {
            app.language = language;
            for _ in 0..2 {
                let mut output =
                    ctx.run_ui(Default::default(), |ui| app.render_met_reference(ui.ctx()));
                output.textures_delta.clear();
            }
            assert_eq!(app.met_reference.request, request);
            assert_eq!(app.met_reference.quote.as_ref(), Some(&quote));
        }
        app.edit.name = "Unsaved main edit".into();
        app.met_reference_action(Action::Commit);
        assert!(
            app.met_reference
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
        app.met_reference_action(Action::Commit);
        assert!(!app.met_reference.dirty);
        assert_eq!(app.met_reference.context.as_ref().unwrap().history.len(), 1);
        assert_eq!(
            app.sheet_edit.as_ref().unwrap().lists["DisciplineList"]["Traits"][0]["Value"],
            "1"
        );
        assert_eq!(app.sheet_edit, app.saved_sheet);
        app.change_route(WorkArea::Player);
        assert!(app.met_reference.context.is_none());
        assert!(app.met_reference.quote.is_none());
    }
}
