use super::*;
use grapevine_workbench::vampire_creation::{
    Context as CreationContext, Quote as CreationQuote, Request as CreationRequest,
};

#[derive(Default)]
pub(super) struct CreationWindow {
    pub(super) picker: super::creation_sources_ui::Picker,
    pub(super) open: bool,
    pub(super) context: Option<CreationContext>,
    pub(super) request: Option<CreationRequest>,
    pub(super) quote: Option<CreationQuote>,
    pub(super) dirty: bool,
    pub(super) message: String,
}
impl CreationWindow {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Preview,
    Save,
    Reload,
    Discard,
    Apply,
}

impl InconnuApp {
    pub(super) fn open_vampire_creation(&mut self) {
        if self.active_role() != Some(Role::Storyteller) {
            self.status = Some(
                tr(
                    self.language,
                    "Vampire creation is available to Storytellers only.",
                    "A criação de vampiros está disponível apenas para Narradores.",
                )
                .into(),
            );
            return;
        }
        if self.creation_main_editor_dirty() {
            self.status = Some(
                tr(
                    self.language,
                    "Save or discard the main character editor before opening creation.",
                    "Salve ou descarte o editor principal do personagem antes de abrir a criação.",
                )
                .into(),
            );
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            if !self.creation.dirty {
                self.creation.clear();
            }
            self.status = Some(
                tr(
                    self.language,
                    "Select a character before opening creation.",
                    "Selecione um personagem antes de abrir a criação.",
                )
                .into(),
            );
            return;
        };
        if let Some(context) = &self.creation.context {
            if context.record_id == id && self.creation.dirty {
                self.creation.open = true;
                return;
            }
            if context.record_id != id && self.creation.dirty {
                self.creation.open = true;
                self.creation.message = tr(self.language, "This draft belongs to another character. Reselect it to save or discard before switching.", "Este rascunho pertence a outro personagem. Selecione-o novamente para salvar ou descartar antes de trocar.").into();
                return;
            }
        }
        self.load_creation_context(&id);
    }

    fn creation_main_editor_dirty(&self) -> bool {
        self.library.is_some()
            && (self.edit != self.saved_edit
                || self.sheet_edit != self.saved_sheet
                || (self.xp_form.is_dirty() && self.xp_form != self.xp_saved_form))
    }

    fn creation_can_act(&self) -> bool {
        self.active_role() == Some(Role::Storyteller)
            && !self.creation_main_editor_dirty()
            && self.creation.context.as_ref().is_some_and(|context| {
                context.editable && self.selected_id.as_deref() == Some(context.record_id.as_str())
            })
    }

    fn load_creation_context(&mut self, id: &str) {
        let Some(library) = self.library.as_ref() else {
            self.creation.message = tr(
                self.language,
                "Open a library first.",
                "Abra uma biblioteca primeiro.",
            )
            .into();
            return;
        };
        match library.vampire_creation_context(id, Role::Storyteller) {
            Ok(context) => {
                self.revision = Some(context.revision);
                self.creation = CreationWindow {
                    picker: Default::default(),
                    open: true,
                    request: Some(context.request.clone()),
                    context: Some(context),
                    quote: None,
                    dirty: false,
                    message: String::new(),
                };
            }
            Err(error) => {
                let message = error.message.clone();
                self.log_error("vampire_creation.context", &error);
                self.creation.message = message;
                self.creation.open = true;
            }
        }
    }

    fn preview_vampire_creation(&mut self) {
        if !self.creation_can_act() {
            self.creation.message = tr(self.language, "Preview is blocked until the Storyteller role, selected character, and main editor are ready.", "A prévia está bloqueada até que o papel de Narrador, o personagem selecionado e o editor principal estejam prontos.").into();
            return;
        }
        let result = match (
            &self.library,
            &self.creation.context,
            &self.creation.request,
        ) {
            (Some(lib), Some(context), Some(request)) => lib.quote_vampire_creation(
                &context.record_id,
                request,
                context.revision,
                Role::Storyteller,
            ),
            _ => return,
        };
        match result {
            Ok(quote) => {
                self.creation.message = if quote.ready { tr(self.language, "Preview is ready for review. Apply remains separate.", "A prévia está pronta para revisão. Aplicar continua sendo uma ação separada.") }
                    else { tr(self.language, "Preview created with unresolved issues below.", "Prévia criada com pendências abaixo.") }.into();
                self.creation.quote = Some(quote);
            }
            Err(error) => {
                let message = error.message.clone();
                self.log_error("vampire_creation.quote", &error);
                self.creation.quote = None;
                self.creation.message = message;
            }
        }
    }

    fn save_vampire_creation_draft(&mut self) {
        if !self.creation_can_act() {
            self.creation.message = tr(self.language, "Draft save is blocked until the Storyteller role, selected character, and main editor are ready.", "O salvamento está bloqueado até que o papel de Narrador, o personagem selecionado e o editor principal estejam prontos.").into();
            return;
        }
        let result = match (
            &mut self.library,
            &self.creation.context,
            &self.creation.request,
        ) {
            (Some(lib), Some(context), Some(request)) => lib.save_vampire_creation_draft(
                &context.record_id,
                request,
                context.revision,
                Role::Storyteller,
            ),
            _ => return,
        };
        match result {
            Ok(revision) => {
                let id = self.creation.context.as_ref().unwrap().record_id.clone();
                self.revision = Some(revision);
                self.load_creation_context(&id);
                self.creation.message = tr(
                    self.language,
                    "Unfinished creation draft saved.",
                    "Rascunho de criação incompleto salvo.",
                )
                .into();
            }
            Err(error) => {
                let message = error.message.clone();
                self.log_error("vampire_creation.save_draft", &error);
                self.creation.message = message;
            }
        }
    }

    fn reload_vampire_creation_draft(&mut self) {
        if !self.creation_can_act() {
            self.creation.message = tr(self.language, "Reload is blocked until the Storyteller role, selected character, and main editor are ready.", "A recarga está bloqueada até que o papel de Narrador, o personagem selecionado e o editor principal estejam prontos.").into();
            return;
        }
        let Some(id) = self
            .creation
            .context
            .as_ref()
            .map(|context| context.record_id.clone())
        else {
            return;
        };
        self.load_creation_context(&id);
        self.creation.message = tr(
            self.language,
            "Saved draft reloaded; local edits were discarded.",
            "Rascunho salvo recarregado; as edições locais foram descartadas.",
        )
        .into();
    }

    fn discard_vampire_creation_draft(&mut self) {
        if !self.creation_can_act() {
            self.creation.message = tr(self.language, "Discard is blocked until the Storyteller role, selected character, and main editor are ready.", "O descarte está bloqueado até que o papel de Narrador, o personagem selecionado e o editor principal estejam prontos.").into();
            return;
        }
        let result = match (&mut self.library, &self.creation.context) {
            (Some(lib), Some(context)) => lib.discard_vampire_creation_draft(
                &context.record_id,
                context.revision,
                Role::Storyteller,
            ),
            _ => return,
        };
        match result {
            Ok(revision) => {
                let id = self.creation.context.as_ref().unwrap().record_id.clone();
                self.revision = Some(revision);
                self.load_creation_context(&id);
                self.creation.message = tr(
                    self.language,
                    "Saved creation draft discarded.",
                    "Rascunho de criação salvo descartado.",
                )
                .into();
            }
            Err(error) => {
                let message = error.message.clone();
                self.log_error("vampire_creation.discard_draft", &error);
                self.creation.message = message;
            }
        }
    }

    fn apply_vampire_creation(&mut self) {
        if !self.creation_can_act() {
            self.creation.message = tr(self.language, "Apply is blocked until the Storyteller role, selected character, and main editor are ready.", "A aplicação está bloqueada até que o papel de Narrador, o personagem selecionado e o editor principal estejam prontos.").into();
            return;
        }
        if !self
            .creation
            .request
            .as_ref()
            .is_some_and(|request| request.standard_pools)
        {
            self.creation.message = tr(
                self.language,
                "Apply is available only for standard, uncompressed pools.",
                "A aplicação está disponível apenas para pools padrão, sem compressão.",
            )
            .into();
            return;
        }
        if !self
            .creation
            .quote
            .as_ref()
            .is_some_and(|quote| quote.ready)
        {
            self.creation.message = tr(
                self.language,
                "Create a ready preview before applying.",
                "Crie uma prévia pronta antes de aplicar.",
            )
            .into();
            return;
        }
        let result = match (&mut self.library, &self.creation.quote) {
            (Some(lib), Some(quote)) => lib.commit_vampire_creation(quote, Role::Storyteller),
            _ => return,
        };
        match result {
            Ok(receipt) => {
                let id = self.creation.context.as_ref().unwrap().record_id.clone();
                self.revision = Some(receipt.committed_revision);
                self.creation.dirty = false;
                self.creation.quote = None;
                self.refresh_summaries(true);
                self.load_record(&id, false);
                self.load_creation_context(&id);
                self.creation.message = tr(
                    self.language,
                    "Creation applied. The updated sheet and receipt are loaded.",
                    "Criação aplicada. A ficha atualizada e o recibo foram carregados.",
                )
                .into();
            }
            Err(error) => {
                let message = error.message.clone();
                self.log_error("vampire_creation.commit", &error);
                self.creation.message = message;
            }
        }
    }
    pub(super) fn render_vampire_creation(&mut self, ctx: &egui::Context) {
        if !self.creation.open {
            return;
        }
        if self.active_role() != Some(Role::Storyteller) {
            self.creation.open = false;
            return;
        }
        let language = self.language;
        let selected_id = self.selected_id.clone();
        let main_editor_dirty = self.creation_main_editor_dirty();
        let summaries = &self.summaries;
        let message = self.creation.message.clone();
        let dirty = self.creation.dirty;
        let context = self.creation.context.as_ref();
        let request = self.creation.request.as_mut();
        let quote = self.creation.quote.as_ref();
        let creature_profile = self
            .selected_record
            .as_ref()
            .and_then(|r| grapevine_workbench::creature_profiles::profile(&r.kind))
            .filter(|p| p.kind != "Vampire");
        let creation_label = if creature_profile.is_some() {
            tr(language, "Creature creation", "Criação de criatura")
        } else {
            tr(language, "Vampire creation", "Criação de vampiro")
        };
        let picker = &mut self.creation.picker;
        let mut window_open = self.creation.open;
        let mut changed = false;
        let mut action = None;
        let title = context.map_or_else(
            || creation_label.to_owned(),
            |c| format!("{} · {}", creation_label, c.record_name),
        );
        egui::Window::new(title).id(egui::Id::unique("vampire_creation_window"))
            .default_size([960.0, 760.0]).open(&mut window_open).show(ctx, |ui| {
                ui.label(tr(language,
                    "Source-based creation preview. Local review does not establish OWbN or chronicle approval.",
                    "Prévia de criação baseada em fontes. A revisão local não comprova aprovação da OWbN ou da crônica."));
                ui.small(tr(language, "Character creation: printed pp. 67–72 (PDF pp. 68–73); current Blood: printed p. 106 (PDF p. 107).", "Criação: páginas impressas 67–72 (PDF 68–73); Sangue atual: página impressa 106 (PDF 107)."));
                ui.small(tr(language, "Generation starts at 13th; each Generation Background Trait lowers it by one.", "A Geração começa na 13ª; cada Característica de Geração a reduz em uma."));
                for citation in grapevine_workbench::creation_generation::source_citations() {
                    ui.small(format!("{} ({}) · {} p.{} (PDF p.{})", citation.book, citation.edition, citation.system, citation.printed_page, citation.pdf_page_1based));
                }
                ui.small(tr(language,
                    "[For more detailed info check your library for \"Laws of the Night (Revised).\"]",
                    "[Para informações mais detalhadas, consulte sua biblioteca em \"Laws of the Night (Revised).\"]"));
                if dirty { ui.strong(tr(language, "Unsaved creation edits", "Edições de criação não salvas")); }
                if !message.is_empty() { ui.label(&message); }

                if let (Some(context), Some(request)) = (context, request) {
                    let selection_matches = selected_id.as_deref() == Some(context.record_id.as_str());
                    let can_edit = context.editable && selection_matches && !main_editor_dirty;
                    ui.label(format!("{} · {} · {}", context.record_name, context.record_id,
                        tr(language, if context.editable { "Editable" } else { "Read-only" }, if context.editable { "Editável" } else { "Somente leitura" })));
                    if !context.editable { ui.weak(tr(language, "This character context is read-only.", "O contexto deste personagem é somente leitura.")); }
                    if !selection_matches { ui.colored_label(egui::Color32::YELLOW, tr(language, "Select this character again to continue; this window remains bound to it.", "Selecione este personagem novamente para continuar; esta janela permanece vinculada a ele.")); }
                    if main_editor_dirty { ui.colored_label(egui::Color32::YELLOW, tr(language, "The main editor has unsaved changes. Save or discard them before using creation actions.", "O editor principal tem alterações não salvas. Salve ou descarte antes de usar as ações de criação.")); }

                    // Keep the preview and apply probes at the top, before the long sheet editor.
                    let can_apply = can_edit && request.standard_pools && quote.is_some_and(|q| q.ready);
                    ui.horizontal_wrapped(|ui| {
                        let response = ui.add_enabled(can_edit, egui::Button::new(tr(language, "Preview pools", "Pré-visualizar pools")));
                        remember(ui, &response, "creation_preview");
                        if response.clicked() { action = Some(Action::Preview); }
                        if ui.add_enabled(can_edit, egui::Button::new(tr(language, "Save unfinished draft", "Salvar rascunho incompleto"))).clicked() { action = Some(Action::Save); }
                        if ui.add_enabled(can_edit, egui::Button::new(tr(language, "Reload saved draft · discard local edits", "Recarregar rascunho · descartar edições locais"))).clicked() { action = Some(Action::Reload); }
                        if ui.add_enabled(can_edit, egui::Button::new(tr(language, "Discard saved draft", "Descartar rascunho salvo"))).clicked() { action = Some(Action::Discard); }
                        let response = ui.add_enabled(can_apply, egui::Button::new(tr(language, "Apply ready creation", "Aplicar criação pronta")));
                        remember(ui, &response, "creation_apply");
                        if response.clicked() { action = Some(Action::Apply); }
                    });
                    if !request.standard_pools {
                        ui.colored_label(egui::Color32::YELLOW, tr(language, "Only standard, uncompressed pools can be applied here.", "Somente pools padrão, sem compressão, podem ser aplicados aqui."));
                    }
                    ui.add_enabled_ui(can_edit, |ui| {
                        changed |= ui.checkbox(&mut request.create_as_npc, tr(language, "Create as a Storyteller NPC", "Criar como PNJ do Narrador")).changed();
                        changed |= crate::purchase_ui::render_owbn_evidence(
                            ui, &mut request.owbn_evidence,
                            creature_profile.map_or("Vampire", |p| p.kind.as_str()),
                            language == Language::Portuguese, false,
                        );
                    });
                    if let Some(quote) = quote { render_quote(ui, quote, language); }
                    else { ui.label(tr(language, "No current preview. Preview again after changing any field.", "Sem prévia atual. Pré-visualize novamente após alterar qualquer campo.")); }
                    if !context.history.is_empty() {
                        egui::CollapsingHeader::new(tr(language, "Creation receipts", "Recibos de criação")).show(ui, |ui| {
                            for receipt in &context.history {
                                if receipt.quote.is_configured() {
                                    ui.label(format!("{} {} · {} · {}",tr(language,"Revision","Revisão"),receipt.committed_revision,receipt.quote.request.actor,receipt.quote.request.day));
                                    continue;
                                }
                                ui.label(format!("{} {} · {} · {} · generation {}", tr(language, "Revision", "Revisão"), receipt.committed_revision,
                                    receipt.quote.request.actor, receipt.quote.request.day, receipt.quote.generation.generation));
                            }
                        });
                    }
                    ui.separator();
                    egui::ScrollArea::vertical().id_salt("vampire_creation_form").show(ui, |ui| {
                        ui.add_enabled_ui(can_edit, |ui| {
                            if let Some(profile)=creature_profile {
                                render_configured_form(ui,request,profile,language,summaries,&mut changed);
                            } else {
                                changed |= super::creation_sources_ui::render(ui,&mut request.sheet,picker,language);
                                render_creation_form(ui,request,&context.record_id,language,summaries,&mut changed);
                            }
                        });
                    });
                } else {
                    ui.label(tr(language, "Select a Storyteller-controlled character to load its creation context.", "Selecione um personagem controlado pelo Narrador para carregar o contexto de criação."));
                }
            });
        self.creation.open = window_open;
        if changed {
            self.creation.dirty = true;
            self.creation.quote = None;
        }
        if !window_open {
            return;
        }
        if changed && action == Some(Action::Apply) {
            self.creation.message = tr(
                language,
                "The sheet changed after the preview. Preview again before applying.",
                "A ficha mudou após a prévia. Pré-visualize novamente antes de aplicar.",
            )
            .into();
            return;
        }
        match action {
            Some(Action::Preview) => self.preview_vampire_creation(),
            Some(Action::Save) => self.save_vampire_creation_draft(),
            Some(Action::Reload) => self.reload_vampire_creation_draft(),
            Some(Action::Discard) => self.discard_vampire_creation_draft(),
            Some(Action::Apply) => self.apply_vampire_creation(),
            None => {}
        }
    }
}
fn render_configured_form(
    ui: &mut egui::Ui,
    request: &mut CreationRequest,
    profile: &grapevine_workbench::creature_profiles::Profile,
    language: Language,
    summaries: &[Summary],
    changed: &mut bool,
) {
    let before = request.clone();
    ui.strong(format!(
        "{} · {} / {}",
        profile.kind, profile.primary_power_list, profile.secondary_power_list
    ));
    ui.label(&profile.rules_note);
    if !profile.starting_resources.is_empty() {
        ui.label(tr(
            language,
            "Reviewed starting resources",
            "Recursos iniciais revisados",
        ));
        for (key, value) in &profile.starting_resources {
            ui.label(format!("{key}: {value}"));
        }
    }
    if profile.starting_resources.contains_key("Willpower") && profile.free_trait_costs.is_some() {
        ui.horizontal(|ui| {
            ui.label(tr(
                language,
                "Additional starting Willpower (Free Traits)",
                "Força de Vontade inicial adicional (características livres)",
            ));
            ui.add(egui::DragValue::new(&mut request.budget.willpower_free).range(0..=4096));
        });
    }
    crate::sheet_ui::render(ui, &mut request.sheet, language, summaries);
    for i in 0..3 {
        egui::ComboBox::from_id_salt(("configured_priority", i))
            .selected_text(priority_name(request.budget.priorities[i], language))
            .show_ui(ui, |ui| {
                for priority in 0..3 {
                    ui.selectable_value(
                        &mut request.budget.priorities[i],
                        priority,
                        format!(
                            "{} · {}",
                            priority_name(priority, language),
                            profile.attribute_priorities[priority]
                        ),
                    );
                }
            });
    }
    ui.checkbox(
        &mut request.standard_pools,
        tr(
            language,
            "Use this reviewed standard profile; no unlisted rule adjustments",
            "Usar este perfil padrão revisado; sem ajustes de regras não listados",
        ),
    );
    for (caption, value) in [
        (
            tr(language, "Reviewer / actor", "Revisor / responsável"),
            &mut request.actor,
        ),
        (
            tr(language, "Date (YYYY-MM-DD)", "Data (AAAA-MM-DD)"),
            &mut request.day,
        ),
        (tr(language, "Reason", "Motivo"), &mut request.reason),
        (
            tr(language, "Policy version", "Versão da política"),
            &mut request.policy_version,
        ),
        (
            tr(
                language,
                "Review/source reference",
                "Referência de revisão/fonte",
            ),
            &mut request.review_reference,
        ),
        (
            tr(
                language,
                "Power/source review reference",
                "Referência de revisão/fonte dos poderes",
            ),
            &mut request.manual_power_reference,
        ),
        (
            tr(
                language,
                "Resources/source review reference",
                "Referência de revisão/fonte dos recursos",
            ),
            &mut request.current_resources_reference,
        ),
    ] {
        ui.label(caption);
        ui.text_edit_singleline(value);
    }
    ui.checkbox(
        &mut request.review_confirmed,
        tr(
            language,
            "I reviewed this creature's source and local policy",
            "Revisei a fonte desta criatura e a política local",
        ),
    );
    *changed |= *request != before;
}

fn render_creation_form(
    ui: &mut egui::Ui,
    request: &mut CreationRequest,
    record_id: &str,
    language: Language,
    summaries: &[Summary],
    changed: &mut bool,
) {
    let before_sheet = request.sheet.clone();
    ui.heading(tr(language, "Character sheet", "Ficha do personagem"));
    crate::sheet_ui::render_creation(ui, &mut request.sheet, language, summaries);
    if request.sheet != before_sheet {
        *changed = true;
    }

    ui.separator();
    ui.heading(tr(
        language,
        "Optional current resources",
        "Recursos atuais opcionais",
    ));
    ui.label(tr(language,
        "Choose a value before adding it. No Blood or Willpower value is inferred from generation.",
        "Escolha um valor antes de adicioná-lo. Nenhum valor de Sangue ou Força de Vontade é inferido da Geração."));
    render_optional_resource(
        ui,
        &mut request.sheet.resources,
        record_id,
        "TempBlood",
        tr(language, "Current Blood", "Sangue atual"),
        language,
        changed,
    );
    render_optional_resource(
        ui,
        &mut request.sheet.resources,
        record_id,
        "TempWillpower",
        tr(language, "Current Willpower", "Força de Vontade atual"),
        language,
        changed,
    );
    for key in ["TempBlood", "TempWillpower"] {
        if !request.sheet.resources.contains_key(key)
            && request.sheet.visibility.remove(key).is_some()
        {
            *changed = true;
        }
    }
    text_field(
        ui,
        tr(
            language,
            "Current-resource source/reference",
            "Fonte/referência dos recursos atuais",
        ),
        &mut request.current_resources_reference,
        changed,
    );

    ui.separator();
    ui.heading(tr(language, "Creation choices", "Escolhas de criação"));
    ui.label(tr(language,
        "Trait totals, Negative Traits, Flaw credit, Merit cost, and Basic-power counts are read from the sheet lists above. Unused base pools stay in their own category.",
        "Totais de características, características negativas, crédito de Defeitos, custo de Méritos e poderes Básicos são lidos das listas da ficha acima. Pools base não usados permanecem em sua própria categoria."));
    let areas = [
        tr(language, "Physical", "Físicos"),
        tr(language, "Social", "Sociais"),
        tr(language, "Mental", "Mentais"),
    ];
    for (i, area) in areas.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} {}",
                area,
                tr(language, "priority", "prioridade")
            ));
            egui::ComboBox::from_id_salt(("creation_priority", record_id, i))
                .selected_text(priority_name(request.budget.priorities[i], language))
                .show_ui(ui, |ui| {
                    for rank in 0..=2 {
                        let response = ui.selectable_value(
                            &mut request.budget.priorities[i],
                            rank,
                            priority_name(rank, language),
                        );
                        *changed |= response.changed();
                    }
                });
        });
    }
    checkbox(
        ui,
        tr(
            language,
            "Use Sabbat starting pools",
            "Usar pools iniciais do Sabá",
        ),
        &mut request.budget.sabbat,
        changed,
    );
    checkbox(
        ui,
        tr(
            language,
            "Use standard, uncompressed pools",
            "Usar pools padrão, sem compressão",
        ),
        &mut request.standard_pools,
        changed,
    );

    ui.heading(tr(
        language,
        "Virtues and Morality",
        "Virtudes e Moralidade",
    ));
    checkbox(
        ui,
        tr(
            language,
            "Use Conviction instead of Conscience",
            "Usar Convicção em vez de Consciência",
        ),
        &mut request.budget.conviction,
        changed,
    );
    checkbox(
        ui,
        tr(
            language,
            "Use Instinct instead of Self-Control",
            "Usar Instinto em vez de Autocontrole",
        ),
        &mut request.budget.instinct,
        changed,
    );
    for i in 0..3 {
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} · {}",
                virtue_name(i, language),
                tr(language, "allocated from seven", "alocadas das sete")
            ));
            let response =
                ui.add(egui::DragValue::new(&mut request.budget.virtue_allocated[i]).range(0..=7));
            *changed |= response.changed();
            ui.label(tr(
                language,
                "Free Traits (2 each)",
                "Características Livres (2 cada)",
            ));
            let response =
                ui.add(egui::DragValue::new(&mut request.budget.virtue_free[i]).range(0..=5));
            *changed |= response.changed();
        });
    }
    ui.horizontal(|ui| {
        ui.label(tr(
            language,
            "Morality Traits bought (3 Free Traits each)",
            "Características de Moralidade compradas (3 Características Livres cada)",
        ));
        let response = ui.add(egui::DragValue::new(&mut request.budget.morality_free).range(0..=5));
        *changed |= response.changed();
    });
    ui.horizontal(|ui| {
        ui.label(tr(
            language,
            "Willpower Traits bought (3 Free Traits each)",
            "Características de Força de Vontade compradas (3 Características Livres cada)",
        ));
        let response =
            ui.add(egui::DragValue::new(&mut request.budget.willpower_free).range(0..=20));
        *changed |= response.changed();
    });
    checkbox(
        ui,
        tr(
            language,
            "Optional derangement for two Negative Traits",
            "Perturbação opcional por duas Características Negativas",
        ),
        &mut request.budget.optional_derangement,
        changed,
    );
    checkbox(
        ui,
        tr(
            language,
            "Voluntary one-Trait Morality loss for two Negative Traits",
            "Perda voluntária de uma Característica de Moralidade por duas Características Negativas",
        ),
        &mut request.budget.lower_morality,
        changed,
    );

    ui.separator();
    ui.heading(tr(language, "Specializations", "Especializações"));
    ui.label(tr(language,
        "Each entry is one specialization. Use the exact Ability row name; add the specialty in that Ability's name if it should appear on the public sheet. Private notes are not printed publicly.",
        "Cada entrada é uma especialização. Use o nome exato da linha de Habilidade; inclua a especialidade no nome da Habilidade se ela deve aparecer na ficha pública. Notas privadas não são impressas publicamente."));
    let mut remove = Vec::new();
    for (i, name) in request.specializations.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} {}",
                tr(language, "Ability row", "Linha de Habilidade"),
                i + 1
            ));
            let response = ui.text_edit_singleline(name);
            *changed |= response.changed();
            if ui.button(tr(language, "Remove", "Remover")).clicked() {
                remove.push(i);
                *changed = true;
            }
        });
    }
    for i in remove.into_iter().rev() {
        request.specializations.remove(i);
    }
    if ui
        .button(tr(
            language,
            "Add specialization",
            "Adicionar especialização",
        ))
        .clicked()
    {
        request.specializations.push(String::new());
        *changed = true;
    }

    ui.separator();
    ui.heading(tr(
        language,
        "Source, review, and record details",
        "Fonte, revisão e detalhes do registro",
    ));
    text_field(
        ui,
        tr(language, "Reviewer / actor", "Revisor / responsável"),
        &mut request.actor,
        changed,
    );
    text_field(ui, tr(language, "Date", "Data"), &mut request.day, changed);
    ui.label(tr(language, "Reason", "Motivo"));
    let response = ui.add(egui::TextEdit::multiline(&mut request.reason).desired_rows(3));
    *changed |= response.changed();
    text_field(
        ui,
        tr(language, "Policy version", "Versão da política"),
        &mut request.policy_version,
        changed,
    );
    text_field(
        ui,
        tr(
            language,
            "Review/source reference",
            "Referência de revisão/fonte",
        ),
        &mut request.review_reference,
        changed,
    );
    checkbox(
        ui,
        tr(
            language,
            "I reviewed the selected source and creation policy",
            "Revisei a fonte selecionada e a política de criação",
        ),
        &mut request.review_confirmed,
        changed,
    );
    ui.label(tr(language,
        "Powers not bound to a reviewed published source require an exact manual reference. Source and eligibility still need staff review.",
        "Poderes sem vínculo a uma fonte publicada revisada exigem uma referência manual exata. Fonte e elegibilidade ainda precisam de revisão da equipe."));
    ui.label(tr(
        language,
        "Manual power reference",
        "Referência manual do poder",
    ));
    let response =
        ui.add(egui::TextEdit::multiline(&mut request.manual_power_reference).desired_rows(2));
    *changed |= response.changed();
}
fn render_optional_resource(
    ui: &mut egui::Ui,
    resources: &mut std::collections::BTreeMap<String, i64>,
    record_id: &str,
    key: &str,
    title: &str,
    language: Language,
    changed: &mut bool,
) {
    if let Some(mut value) = resources.get(key).copied() {
        let mut remove = false;
        ui.horizontal(|ui| {
            ui.label(title);
            let response = ui.add(egui::DragValue::new(&mut value).range(0..=4096));
            *changed |= response.changed();
            remove = ui
                .button(tr(language, "Remove value", "Remover valor"))
                .clicked();
        });
        if remove {
            resources.remove(key);
            *changed = true;
        } else if resources.get(key).copied() != Some(value) {
            resources.insert(key.to_owned(), value);
            *changed = true;
        }
    } else {
        let id = egui::Id::unique(("creation_resource_candidate", record_id, key));
        let mut candidate = ui.ctx().data(|data| data.get_temp::<i64>(id)).unwrap_or(0);
        let mut add = false;
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} · {}",
                title,
                tr(language, "not set", "não definido")
            ));
            let _ = ui.add(egui::DragValue::new(&mut candidate).range(0..=4096));
            add = ui
                .button(tr(
                    language,
                    "Add chosen value",
                    "Adicionar valor escolhido",
                ))
                .clicked();
        });
        ui.ctx().data_mut(|data| data.insert_temp(id, candidate));
        if add {
            resources.insert(key.to_owned(), candidate);
            *changed = true;
        }
    }
}

fn render_quote(ui: &mut egui::Ui, quote: &CreationQuote, language: Language) {
    if let (Some(profile), Some(budget)) = (&quote.configured_profile, &quote.configured_budget) {
        ui.heading(format!(
            "{} · {}",
            profile.kind,
            tr(
                language,
                "Configured creation preview",
                "Prévia de criação configurada"
            )
        ));
        ui.label(format!(
            "{}: {:?}",
            tr(
                language,
                "Attribute pools remaining",
                "Pools de atributos restantes"
            ),
            budget.attribute_remaining
        ));
        ui.label(format!(
            "{}: {} · {}: {} · {}: {}",
            tr(language, "Abilities remaining", "Habilidades restantes"),
            budget.ability_remaining,
            tr(language, "Backgrounds remaining", "Antecedentes restantes"),
            budget.background_remaining,
            tr(
                language,
                "Primary powers remaining",
                "Poderes primários restantes"
            ),
            budget.primary_power_remaining
        ));
        ui.label(format!(
            "{}: {}",
            tr(
                language,
                "Free Traits available",
                "Características livres disponíveis"
            ),
            budget.free_available
        ));
        ui.label(format!(
            "{}: {} / {}: {}",
            tr(
                language,
                "Free Traits spent",
                "Características livres gastas"
            ),
            budget
                .free_spent
                .map(|n| n.to_string())
                .unwrap_or_else(|| tr(language, "Unpriced", "Sem custo revisado").into()),
            tr(
                language,
                "Free Traits remaining",
                "Características livres restantes"
            ),
            budget
                .free_remaining
                .map(|n| n.to_string())
                .unwrap_or_else(|| tr(language, "Unpriced", "Sem custo revisado").into())
        ));
        ui.label(tr(language, "Resulting resources", "Recursos resultantes"));
        for (key, value) in &quote.effective_sheet.resources {
            ui.label(format!("{key}: {value}"));
        }
        ui.label(tr(
            language,
            "No Vampire Generation, Blood or Virtue calculation applies to this profile.",
            "Nenhum cálculo de Geração, Sangue ou Virtudes de Vampiro se aplica a este perfil.",
        ));
        for issue in &quote.issues {
            ui.colored_label(egui::Color32::YELLOW, issue);
        }
        for warning in &quote.warnings {
            ui.weak(warning);
        }
        return;
    }

    ui.separator();
    ui.heading(if quote.ready {
        tr(
            language,
            "Ready creation preview",
            "Prévia de criação pronta",
        )
    } else {
        tr(language, "Creation preview", "Prévia de criação")
    });
    let limits = quote.generation;
    ui.label(format!(
        "{} {} · {} {} · {} {} · {} {} · {} {}",
        tr(language, "Generation", "Geração"),
        limits.generation,
        tr(language, "Attribute maximum", "Máximo de Atributos"),
        limits.attribute_max,
        tr(language, "Ability maximum", "Máximo de Habilidades"),
        limits.ability_max,
        tr(language, "Blood capacity", "Capacidade de Sangue"),
        limits.blood_capacity,
        tr(language, "Blood spend per turn", "Sangue gasto por turno"),
        limits.blood_per_turn
    ));
    let final_willpower = quote.effective_sheet.resources.get("Willpower").copied();
    ui.label(format!(
        "{} {} · {} {} · {} {}",
        tr(
            language,
            "Generation starting Willpower",
            "Força de Vontade inicial da Geração"
        ),
        limits.starting_willpower,
        tr(
            language,
            "Final sheet permanent Willpower",
            "Força de Vontade permanente final na ficha"
        ),
        final_willpower.map_or_else(
            || tr(language, "not set", "não definida").to_owned(),
            |v| v.to_string()
        ),
        tr(language, "Willpower maximum", "Máximo de Força de Vontade"),
        limits.willpower_max
    ));
    let r = quote.budget.base_remaining;
    ui.label(format!(
        "{}: {} {} · {} {} · {} {} · {} {} · {} {} · {} {} · {} {}",
        tr(language, "Base pools remaining", "Pools base restantes"),
        tr(language, "Physical", "Físicos"),
        r[0],
        tr(language, "Social", "Sociais"),
        r[1],
        tr(language, "Mental", "Mentais"),
        r[2],
        tr(language, "Abilities", "Habilidades"),
        r[3],
        tr(language, "Backgrounds", "Antecedentes"),
        r[4],
        tr(language, "Basic powers", "Poderes Básicos"),
        r[5],
        tr(language, "Virtues", "Virtudes"),
        r[6]
    ));
    ui.label(format!(
        "{}: {} · {}: {} · {}: {}",
        tr(
            language,
            "Free Traits available",
            "Características Livres disponíveis"
        ),
        quote.budget.free_available,
        tr(language, "spent", "gastas"),
        quote.budget.free_spent,
        tr(language, "remaining", "restantes"),
        quote.budget.free_remaining
    ));
    ui.label(format!(
        "{}: {} / {} / {} · {} {}",
        tr(language, "Final Virtues", "Virtudes finais"),
        quote.budget.virtues[0],
        quote.budget.virtues[1],
        quote.budget.virtues[2],
        tr(language, "Morality", "Moralidade"),
        quote.budget.morality
    ));
    ui.weak(tr(language,
        "Blood capacity is separate from current starting Blood; the evening test, Herd, and character modifiers determine current Blood.",
        "A capacidade de Sangue é separada do Sangue inicial atual; o teste da noite, Rebanho e modificadores do personagem determinam o Sangue atual."));
    if !quote.issues.is_empty() {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            tr(language, "Issues", "Pendências"),
        );
        for issue in &quote.issues {
            ui.colored_label(egui::Color32::LIGHT_RED, issue);
        }
    }
    if !quote.warnings.is_empty() {
        ui.colored_label(egui::Color32::YELLOW, tr(language, "Warnings", "Avisos"));
        for warning in &quote.warnings {
            ui.colored_label(egui::Color32::YELLOW, warning);
        }
    }
}

fn text_field(ui: &mut egui::Ui, title: &str, value: &mut String, changed: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(title);
        let response = ui.text_edit_singleline(value);
        *changed |= response.changed();
    });
}

fn checkbox(ui: &mut egui::Ui, title: &str, value: &mut bool, changed: &mut bool) {
    let response = ui.checkbox(value, title);
    *changed |= response.changed();
}

fn priority_name(rank: usize, language: Language) -> &'static str {
    match rank {
        0 => tr(language, "Primary", "Primária"),
        1 => tr(language, "Secondary", "Secundária"),
        2 => tr(language, "Tertiary", "Terciária"),
        _ => tr(language, "Invalid", "Inválida"),
    }
}

fn virtue_name(index: usize, language: Language) -> &'static str {
    match index {
        0 => tr(
            language,
            "Conscience / Conviction",
            "Consciência / Convicção",
        ),
        1 => tr(
            language,
            "Self-Control / Instinct",
            "Autocontrole / Instinto",
        ),
        _ => tr(language, "Courage", "Coragem"),
    }
}

fn tr<'a>(language: Language, english: &'a str, portuguese: &'a str) -> &'a str {
    match language {
        Language::English => english,
        Language::Portuguese => portuguese,
    }
}

fn remember(ui: &egui::Ui, response: &egui::Response, key: &str) {
    let rect = ui
        .clip_rect()
        .contains_rect(response.rect)
        .then_some(response.rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::unique(key), rect));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> (tempfile::TempDir, InconnuApp) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creation-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        lib.create_character("Vampire", "Synthetic UI newborn", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_vampire_creation();
        (dir, app)
    }
    #[test]
    fn every_creature_opens_shared_forms_without_auto_approval_or_private_state_leaks() {
        for profile in &grapevine_workbench::creature_profiles::builtin().profiles {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("synthetic-ui.gvlib");
            let mut lib = Library::create(&path, "Synthetic UI").unwrap();
            let id = lib
                .create_character(&profile.kind, "Synthetic character", 0, Role::Storyteller)
                .unwrap();
            drop(lib);
            let mut app = InconnuApp::new(Some(path));
            app.change_route(WorkArea::Storyteller);
            app.open_vampire_creation();
            assert!(app.creation.context.is_some(), "{}", profile.kind);
            app.preview_vampire_creation();
            let quote = app.creation.quote.as_ref().unwrap();
            assert!(!quote.ready);
            assert_eq!(quote.is_configured(), profile.kind != "Vampire");
            app.creation.request.as_mut().unwrap().actor = "PRIVATE_CONFIGURED_UI_REVIEWER".into();
            app.creation.dirty = true;
            let before = app.creation.request.clone();
            let ctx = egui::Context::default();
            for language in [Language::English, Language::Portuguese] {
                app.language = language;
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    app.render_vampire_creation(ui.ctx())
                });
                output.textures_delta.clear();
                assert_eq!(app.creation.request, before, "{}", profile.kind);
            }
            app.save_vampire_creation_draft();
            assert!(!app.creation.dirty);
            let public = app
                .library
                .as_ref()
                .unwrap()
                .record(&id, Role::Player)
                .unwrap();
            assert!(
                !serde_json::to_string(&public)
                    .unwrap()
                    .contains("PRIVATE_CONFIGURED_UI_REVIEWER")
            );
            app.open_purchases();
            assert!(app.purchase_open);
            assert_eq!(app.purchase_form.kind, profile.kind);
            app.open_trait_advancement();
            assert!(app.ordinary.context.is_some());
            assert_eq!(app.ordinary.kind, profile.kind);
            app.open_published_sources();
            assert!(app.published.context.is_some());
            app.open_met_reference();
            assert!(app.met_reference.context.is_some());
            app.change_route(WorkArea::Player);
            assert!(app.creation.request.is_none());
            assert!(app.ordinary.context.is_none());
        }
    }
    #[test]
    fn creation_language_navigation_and_private_draft_round_trip() {
        let (_dir, mut app) = app();
        app.creation.request.as_mut().unwrap().actor = "PRIVATE_UI_CREATION_ACTOR".into();
        app.creation.dirty = true;
        let before = app.creation.request.clone();
        let ctx = egui::Context::default();
        for language in [Language::English, Language::Portuguese] {
            app.language = language;
            for _ in 0..2 {
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    app.render_vampire_creation(ui.ctx())
                });
                output.textures_delta.clear();
            }
            assert_eq!(app.creation.request, before);
            assert!(app.has_unsaved_edit());
        }
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        app.save_vampire_creation_draft();
        assert!(!app.creation.dirty);
        assert_eq!(app.creation.request, before);
        let id = app.selected_id.clone().unwrap();
        assert!(
            app.library
                .as_ref()
                .unwrap()
                .record(&id, Role::Player)
                .unwrap()
                .fields
                .get("PhysicalList")
                .is_none()
        );
        app.change_route(WorkArea::Player);
        assert!(app.creation.context.is_none());
        assert!(app.creation.request.is_none());
        app.change_route(WorkArea::Storyteller);
        app.open_vampire_creation();
        assert_eq!(app.creation.request, before);
    }
    #[test]
    fn creation_apply_refreshes_main_sheet_and_main_editor_changes_block_preview() {
        let (_dir, mut app) = app();
        app.edit.name = "Unsaved main name".into();
        app.preview_vampire_creation();
        assert!(app.creation.quote.is_none());
        app.edit = app.saved_edit.clone();
        let request = app.creation.request.as_mut().unwrap();
        for (key, rows) in [
            (
                "PhysicalList",
                serde_json::json!([{"Name":"Strong","Value":"7","Note":""}]),
            ),
            (
                "SocialList",
                serde_json::json!([{"Name":"Charming","Value":"5","Note":""}]),
            ),
            (
                "MentalList",
                serde_json::json!([{"Name":"Alert","Value":"3","Note":""}]),
            ),
            (
                "AbilityList",
                serde_json::json!([{"Name":"Academics","Value":"5","Note":""}]),
            ),
            (
                "BackgroundList",
                serde_json::json!([{"Name":"Generation","Value":"2","Note":""},{"Name":"Resources","Value":"3","Note":""}]),
            ),
            (
                "DisciplineList",
                serde_json::json!([{"Name":"One","Value":"basic","Note":""},{"Name":"Two","Value":"basic","Note":""},{"Name":"Three","Value":"basic","Note":""}]),
            ),
        ] {
            request.sheet.lists.get_mut(key).unwrap()["Traits"] = rows;
        }
        request.standard_pools = true;
        request.review_confirmed = true;
        request.budget.virtue_allocated = [3, 2, 2];
        request.day = "2026-11-13".into();
        request.actor = "Synthetic".into();
        request.reason = "Synthetic".into();
        request.policy_version = "Synthetic".into();
        request.review_reference = "Synthetic eligibility".into();
        request.manual_power_reference = "Synthetic book, edition, page and MET eligibility".into();
        app.creation.dirty = true;
        app.preview_vampire_creation();
        assert!(app.creation.quote.as_ref().is_some_and(|q| q.ready));
        app.apply_vampire_creation();
        assert!(!app.creation.dirty);
        assert_eq!(app.creation.context.as_ref().unwrap().history.len(), 1);
        assert_eq!(
            app.sheet_edit.as_ref().unwrap().resources.get("Willpower"),
            Some(&4)
        );
        assert_eq!(app.sheet_edit, app.saved_sheet);
        assert!(!app.has_unsaved_edit());
    }
}
