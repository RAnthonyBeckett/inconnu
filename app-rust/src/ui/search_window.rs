use super::*;
use grapevine_workbench::{
    searches::{self, Clause, Comparison, Field, Mode, Query, Snapshot},
    store::SavedSearch,
};

struct PreparedSearch {
    request: Query,
    snapshot: Snapshot,
    traits: Option<grapevine_workbench::trait_reports::Snapshot>,
    distribution: Option<grapevine_workbench::trait_distribution_report::Snapshot>,
}

pub(super) struct SearchWindow {
    pub open: bool,
    id: Option<String>,
    name: String,
    query: Query,
    saved: (String, Query),
    revision: Option<i64>,
    entries: Vec<SavedSearch>,
    prepared: Option<PreparedSearch>,
    include_players: bool,
    document_language: Language,
    statistics: bool,
    trait_kind: Option<grapevine_workbench::trait_statistics::Kind>,
    distribution_request: Option<grapevine_workbench::trait_distribution::Request>,
    recent: Option<Query>,
    delete_confirmation: bool,
    pub message: Option<String>,
}

impl Default for SearchWindow {
    fn default() -> Self {
        let query = Query {
            mode: Mode::All,
            clauses: Vec::new(),
        };
        Self {
            open: false,
            id: None,
            name: String::new(),
            query: query.clone(),
            saved: (String::new(), query),
            revision: None,
            entries: Vec::new(),
            prepared: None,
            include_players: false,
            document_language: Language::English,
            statistics: false,
            trait_kind: None,
            distribution_request: None,
            recent: None,
            delete_confirmation: false,
            message: None,
        }
    }
}
impl SearchWindow {
    pub(super) fn dirty(&self) -> bool {
        self.name != self.saved.0 || self.query != self.saved.1
    }
    pub(super) fn saved_id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    pub(super) fn prepared(&self) -> Option<&Snapshot> {
        self.prepared.as_ref().map(|prepared| &prepared.snapshot)
    }
    pub(super) fn prepared_traits(&self) -> Option<&grapevine_workbench::trait_reports::Snapshot> {
        self.prepared
            .as_ref()
            .and_then(|prepared| prepared.traits.as_ref())
    }
    pub(super) fn synthetic_traits(kind: grapevine_workbench::trait_statistics::Kind) -> Self {
        Self {
            open: true,
            trait_kind: Some(kind),
            ..Self::default()
        }
    }
    pub(super) fn prepared_distribution(
        &self,
    ) -> Option<&grapevine_workbench::trait_distribution_report::Snapshot> {
        self.prepared.as_ref().and_then(|p| p.distribution.as_ref())
    }
    pub(super) fn synthetic_distribution(unknown: bool) -> Self {
        Self {
            open: true,
            distribution_request: Some(grapevine_workbench::trait_distribution::Request {
                list: if unknown { "FlawList" } else { "InfluenceList" }.into(),
                name: if unknown {
                    "Synthetic flaw"
                } else {
                    "Synthetic influence"
                }
                .into(),
            }),
            document_language: Language::Portuguese,
            ..Self::default()
        }
    }
    pub(super) fn synthetic(revision: Option<i64>) -> Self {
        Self {
            open: true,
            revision,
            name: "Synthetic saved search".into(),
            ..Self::default()
        }
    }
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
fn field_label(field: &Field, l: Language) -> &'static str {
    match field {
        Field::Name => t(l, "Name", "Nome"),
        Field::Family => t(l, "Family", "Família"),
        Field::Player => t(l, "Player", "Jogador"),
        Field::Status => t(l, "Status", "Situação"),
        Field::Group => t(l, "Saved group", "Grupo salvo"),
        Field::Subgroup => t(l, "Saved subgroup", "Subgrupo salvo"),
        Field::Npc => "NPC",
        Field::Trait => t(l, "Trait", "Característica"),
    }
}
fn comparison_label(comparison: &Comparison, l: Language) -> &'static str {
    match comparison {
        Comparison::Equal => t(l, "Equals", "Igual a"),
        Comparison::Contains => t(l, "Contains", "Contém"),
        Comparison::AtLeast => t(l, "At least", "Pelo menos"),
        Comparison::AtMost => t(l, "At most", "No máximo"),
        Comparison::Exists => t(l, "Has a value", "Tem valor"),
    }
}
fn select_field(clause: &mut Clause, field: Field) {
    *clause = Clause {
        field,
        comparison: Comparison::Equal,
        negate: clause.negate,
        value: if field == Field::Trait {
            "AbilityList".into()
        } else {
            String::new()
        },
        trait_name: String::new(),
        number: 0,
    };
}
fn select_comparison(clause: &mut Clause, comparison: Comparison) {
    clause.comparison = comparison;
    if comparison == Comparison::Exists {
        clause.number = 0;
        if clause.field != Field::Trait {
            clause.value.clear();
        }
    }
}

impl InconnuApp {
    pub(super) fn open_searches(&mut self) {
        if self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        if self.searches.revision.is_none() {
            self.load_search(None);
        } else {
            self.searches.open = true;
        }
    }
    pub(super) fn load_search(&mut self, id: Option<String>) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let index = match lib.saved_searches(Role::Storyteller) {
            Ok(index) => index,
            Err(e) => {
                self.searches.message = Some(e.message);
                return;
            }
        };
        let selected = id
            .as_ref()
            .and_then(|id| index.entries.iter().find(|e| &e.id == id));
        if id.is_some() && selected.is_none() {
            self.searches.message = Some(
                t(
                    self.language,
                    "Saved search unavailable. Reload the list.",
                    "Busca salva indisponível. Recarregue a lista.",
                )
                .into(),
            );
            return;
        }
        let (name, query) = selected
            .map(|e| (e.name.clone(), e.query.clone()))
            .unwrap_or_else(|| {
                (
                    String::new(),
                    Query {
                        mode: Mode::All,
                        clauses: Vec::new(),
                    },
                )
            });
        self.searches = SearchWindow {
            open: true,
            id,
            name: name.clone(),
            query: query.clone(),
            saved: (name, query),
            revision: Some(index.revision),
            entries: index.entries,
            document_language: self.searches.document_language,
            recent: self.searches.recent.clone(),
            ..SearchWindow::default()
        };
    }
    fn save_search(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.searches.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(revision)) = (&mut self.library, self.searches.revision) else {
            return;
        };
        match lib.save_search(
            self.searches.id.as_deref(),
            &self.searches.name,
            &self.searches.query,
            revision,
            Role::Storyteller,
        ) {
            Ok(id) => {
                self.searches.name = self.searches.name.trim().into();
                self.searches.saved = (self.searches.name.clone(), self.searches.query.clone());
                self.searches.id = Some(id);
                self.searches.revision = Some(revision + 1);
                self.revision = Some(revision + 1);
                self.searches.prepared = None;
                self.searches.message =
                    Some(t(self.language, "Search saved.", "Busca salva.").into());
                match lib.saved_searches(Role::Storyteller) {
                    Ok(index) => self.searches.entries = index.entries,
                    Err(e) => self.searches.message = Some(e.message),
                }
                self.refresh_summaries(true);
            }
            Err(e) => self.searches.message = Some(format!("{} - {}", e.code, e.message)),
        }
    }
    fn delete_saved_search(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.searches.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(id), Some(revision)) =
            (&mut self.library, &self.searches.id, self.searches.revision)
        else {
            return;
        };
        match lib.delete_search(id, revision, Role::Storyteller) {
            Ok(next) => {
                self.revision = Some(next);
                self.load_search(None);
            }
            Err(e) => self.searches.message = Some(format!("{} - {}", e.code, e.message)),
        }
    }
    fn prepare_search(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let result = if let Some(request) = &self.searches.distribution_request {
            lib.trait_distribution_snapshot(
                &self.searches.query,
                self.searches.include_players,
                request,
                Role::Storyteller,
            )
            .map(|report| (report.search.clone(), None, Some(report)))
        } else {
            match self.searches.trait_kind {
                Some(kind) => lib
                    .trait_statistics_snapshot(
                        &self.searches.query,
                        self.searches.include_players,
                        kind,
                        Role::Storyteller,
                    )
                    .map(|report| (report.search.clone(), Some(report), None)),
                None => lib
                    .search_snapshot(
                        &self.searches.query,
                        self.searches.include_players,
                        Role::Storyteller,
                    )
                    .map(|snapshot| (snapshot, None, None)),
            }
        };
        match result {
            Ok((snapshot, traits, distribution)) => {
                self.searches.recent = Some(self.searches.query.clone());
                self.searches.prepared = Some(PreparedSearch {
                    request: self.searches.query.clone(),
                    snapshot,
                    traits,
                    distribution,
                });
                self.searches.message = None;
            }
            Err(e) => {
                self.searches.prepared = None;
                self.searches.message = Some(e.message);
            }
        }
    }
    fn search_ready(&self) -> bool {
        self.route == WorkArea::Storyteller
            && self.searches.prepared.as_ref().is_some_and(|s| {
                s.request == self.searches.query
                    && s.traits.as_ref().map(|report| report.kind) == self.searches.trait_kind
                    && s.distribution.as_ref().map(|report| &report.request)
                        == self.searches.distribution_request.as_ref()
                    && s.snapshot.include_players == self.searches.include_players
                    && self.library.as_ref().and_then(|l| l.revision().ok())
                        == Some(s.snapshot.revision)
            })
    }
    fn search_document(&self) -> grapevine_workbench::model::Result<String> {
        use grapevine_workbench::model::error;
        if self.route != WorkArea::Storyteller {
            return Err(error("ACCESS_DENIED", "Storyteller view required."));
        }
        let snapshot = self
            .searches
            .prepared
            .as_ref()
            .map(|prepared| &prepared.snapshot)
            .ok_or_else(|| error("SEARCH_PREPARE_REQUIRED", "Run the search first."))?;
        if !self.search_ready() {
            return Err(error(
                "STALE_REVISION",
                "The filters or library changed. Run the search again.",
            ));
        }
        if let Some(report) = self
            .searches
            .prepared
            .as_ref()
            .and_then(|p| p.distribution.as_ref())
        {
            return grapevine_workbench::trait_distribution_report::html(
                report,
                self.searches.document_language,
            );
        }
        if let Some(report) = self
            .searches
            .prepared
            .as_ref()
            .and_then(|prepared| prepared.traits.as_ref())
        {
            return grapevine_workbench::trait_reports::html(
                report,
                self.searches.document_language,
            );
        }
        searches::html(
            snapshot,
            self.searches.document_language,
            self.searches.statistics,
        )
    }
    pub(super) fn render_searches(&mut self, ctx: &egui::Context) {
        if !self.searches.open || self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }

        let l = self.language;
        let mut open = true;
        let mut select = None;
        let mut save = false;
        let mut run = false;
        let mut export = false;
        let mut delete = false;
        let mut character = None;
        let ready = self.search_ready();
        egui::Window::new(t(
            l,
            "Saved searches and statistics",
            "Buscas salvas e estatísticas",
        ))
        .id(egui::Id::unique("search_window"))
        .default_size([780., 570.])
        .vscroll(true)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(t(l, "New search", "Nova busca")).clicked() {
                    select = Some(None);
                }

                let selected = self
                    .searches
                    .entries
                    .iter()
                    .find(|entry| Some(&entry.id) == self.searches.id.as_ref())
                    .map_or(
                        t(l, "Choose saved search", "Escolher busca salva"),
                        |entry| entry.name.as_str(),
                    );

                egui::ComboBox::from_id_salt("saved_search_select")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for entry in &self.searches.entries {
                            if ui
                                .selectable_label(
                                    Some(&entry.id) == self.searches.id.as_ref(),
                                    &entry.name,
                                )
                                .clicked()
                            {
                                select = Some(Some(entry.id.clone()));
                            }
                        }
                    });

                if ui.button(t(l, "Reload list", "Recarregar lista")).clicked() {
                    select = Some(self.searches.id.clone());
                }

                if ui
                    .add_enabled(
                        self.searches.recent.is_some(),
                        egui::Button::new(t(
                            l,
                            "Recent query this session",
                            "Busca recente nesta sessão",
                        )),
                    )
                    .clicked()
                {
                    self.searches.query = self.searches.recent.clone().unwrap();
                }
            });

            ui.horizontal(|ui| {
                ui.label(t(l, "Search name", "Nome da busca"));
                ui.text_edit_singleline(&mut self.searches.name);

                let button = ui.add_enabled(
                    self.searches.dirty(),
                    egui::Button::new(t(l, "Save search", "Salvar busca")),
                );
                let rect = ui.clip_rect().contains_rect(button.rect).then_some(button.rect);
                ui.ctx()
                    .data_mut(|data| data.insert_temp(egui::Id::unique("search_save"), rect));
                save = button.clicked();
            });

            ui.horizontal_wrapped(|ui| {
                ui.label(t(l, "Match", "Corresponder"));
                ui.selectable_value(
                    &mut self.searches.query.mode,
                    Mode::All,
                    t(l, "All conditions", "Todas as condições"),
                );
                ui.selectable_value(
                    &mut self.searches.query.mode,
                    Mode::Any,
                    t(l, "Any condition", "Qualquer condição"),
                );

                if ui
                    .add_enabled(
                        self.searches.query.clauses.len() < 32,
                        egui::Button::new(t(l, "Add condition", "Adicionar condição")),
                    )
                    .clicked()
                {
                    self.searches.query.clauses.push(Clause {
                        field: Field::Name,
                        comparison: Comparison::Contains,
                        negate: false,
                        value: String::new(),
                        trait_name: String::new(),
                        number: 0,
                    });
                }
            });

            ui.label(t(
                l,
                "Uses saved characters. Empty query selects all. Missing values stay unmatched.",
                "Usa personagens salvos. Busca vazia seleciona todos. Valores ausentes ficam sem correspondência.",
            ));

            let mut remove = None;
            for (index, clause) in self.searches.query.clauses.iter_mut().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut clause.negate, t(l, "Not", "Não"));

                        egui::ComboBox::from_id_salt("field")
                            .selected_text(field_label(&clause.field, l))
                            .show_ui(ui, |ui| {
                                for field in [
                                    Field::Name,
                                    Field::Family,
                                    Field::Player,
                                    Field::Status,
                                    Field::Group,
                                    Field::Subgroup,
                                    Field::Npc,
                                    Field::Trait,
                                ] {
                                    let label = field_label(&field, l);
                                    if ui
                                        .selectable_value(&mut clause.field, field, label)
                                        .changed()
                                    {
                                        select_field(clause, field);
                                    }
                                }
                            });

                        let options = match clause.field {
                            Field::Npc => vec![Comparison::Equal],
                            Field::Trait => vec![
                                Comparison::Equal,
                                Comparison::AtLeast,
                                Comparison::AtMost,
                                Comparison::Exists,
                            ],
                            _ => vec![
                                Comparison::Equal,
                                Comparison::Contains,
                                Comparison::Exists,
                            ],
                        };

                        egui::ComboBox::from_id_salt("comparison")
                            .selected_text(comparison_label(&clause.comparison, l))
                            .show_ui(ui, |ui| {
                                for op in options {
                                    let label = comparison_label(&op, l);
                                    if ui.selectable_value(&mut clause.comparison, op, label).changed() {
                                        select_comparison(clause, op);
                                    }
                                }
                            });

                        if clause.field == Field::Npc {
                            ui.selectable_value(&mut clause.number, 1, t(l, "Yes", "Sim"));
                            ui.selectable_value(&mut clause.number, 0, t(l, "No", "Não"));
                        } else if clause.field == Field::Trait {
                            egui::ComboBox::from_id_salt("trait_list")
                                .selected_text(crate::sheet_ui::list_title(&clause.value, l))
                                .show_ui(ui, |ui| {
                                    for key in grapevine_workbench::sheet::LIST_FIELDS {
                                        ui.selectable_value(
                                            &mut clause.value,
                                            (*key).into(),
                                            crate::sheet_ui::list_title(key, l),
                                        );
                                    }
                                });
                            ui.add(
                                egui::TextEdit::singleline(&mut clause.trait_name)
                                    .hint_text(t(l, "Exact trait name", "Nome exato da característica"))
                                    .desired_width(140.),
                            );
                            if clause.comparison != Comparison::Exists {
                                ui.add(
                                    egui::DragValue::new(&mut clause.number)
                                        .range(0..=1_000_000),
                                );
                            }
                        } else if clause.comparison != Comparison::Exists {
                            ui.add(
                                egui::TextEdit::singleline(&mut clause.value).desired_width(140.),
                            );
                        }

                        if ui.button(t(l, "Remove", "Remover")).clicked() {
                            remove = Some(index);
                        }
                    });
                });
            }

            if let Some(index) = remove {
                self.searches.query.clauses.remove(index);
            }

            ui.checkbox(
                &mut self.searches.include_players,
                t(
                    l,
                    "Include Player names in results and print",
                    "Incluir nomes dos Jogadores nos resultados e impressão",
                ),
            );

            ui.horizontal_wrapped(|ui| {
                ui.label(t(l, "Report", "Relatório"));
                if ui.selectable_label(self.searches.trait_kind.is_none() && self.searches.distribution_request.is_none(),t(l,"Search / population","Busca / população")).clicked() {
                    self.searches.trait_kind=None;
                    self.searches.distribution_request=None;
                }
                for kind in [grapevine_workbench::trait_statistics::Kind::Influence,grapevine_workbench::trait_statistics::Kind::MeritsFlaws] {
                    if ui.selectable_label(self.searches.trait_kind==Some(kind),grapevine_workbench::trait_reports::title(kind,l)).clicked() {
                        self.searches.trait_kind=Some(kind);
                        self.searches.distribution_request=None;
                    }
                }
                if ui.selectable_label(self.searches.distribution_request.is_some(),t(l,"Named trait values","Valores de uma característica")).clicked() {
                    self.searches.trait_kind=None;
                    self.searches.distribution_request.get_or_insert(grapevine_workbench::trait_distribution::Request {list:"AbilityList".into(),name:String::new()});
                }
            });
            if let Some(request)=&mut self.searches.distribution_request {
                ui.horizontal_wrapped(|ui| {
                    ui.label(t(l,"Trait list","Lista de características"));
                    egui::ComboBox::from_id_salt("distribution_list")
                        .selected_text(crate::sheet_ui::list_title(&request.list,l))
                        .show_ui(ui,|ui| {
                            for key in grapevine_workbench::sheet::LIST_FIELDS {
                                ui.selectable_value(&mut request.list,(*key).into(),crate::sheet_ui::list_title(key,l));
                            }
                        });
                    ui.label(t(l,"Exact trait name","Nome exato da característica"));
                    ui.text_edit_singleline(&mut request.name);
                });
            }
            ui.horizontal_wrapped(|ui| {
                let button = ui.button(t(l, "Run search", "Executar busca"));
                let rect = ui.clip_rect().contains_rect(button.rect).then_some(button.rect);
                ui.ctx()
                    .data_mut(|data| data.insert_temp(egui::Id::unique("search_run"), rect));
                run = button.clicked();

                ui.selectable_value(
                    &mut self.searches.document_language,
                    Language::English,
                    "English",
                );
                ui.selectable_value(
                    &mut self.searches.document_language,
                    Language::Portuguese,
                    "Português",
                );
                ui.add_enabled(
                    self.searches.trait_kind.is_none() && self.searches.distribution_request.is_none(),
                    egui::Checkbox::new(&mut self.searches.statistics,
                        t(l, "Print statistics", "Imprimir estatísticas")),
                );
                export = ui
                    .add_enabled(
                        ready,
                        egui::Button::new(t(l, "Save private HTML…", "Salvar HTML privado…")),
                    )
                    .clicked();
            });

            if let Some(prepared) = &self.searches.prepared {
                let snapshot = &prepared.snapshot;
                ui.label(format!(
                    "{} {} - {} {}",
                    t(l, "Matches", "Correspondências"),
                    snapshot.rows.len(),
                    t(l, "Library revision", "Revisão da biblioteca"),
                    snapshot.revision
                ));
                if ready {
                    if let Some(distribution)=&prepared.distribution {
                        ui.label(format!("{} {} · {} {} · {} {}",
                            t(l,"Named-trait matches","Correspondências da característica"),distribution.report.matched_characters,
                            t(l,"Missing/unreadable lists","Listas ausentes/ilegíveis"),distribution.report.missing_list_characters,
                            t(l,"Readable list without this trait","Lista legível sem a característica"),distribution.report.unmatched_characters));
                        ui.label(t(l,"Unknown is separate from zero. Different values can count one character in several rows.","Desconhecido é separado de zero. Valores diferentes podem contar um personagem em várias linhas."));
                        let row_height=ui.text_style_height(&egui::TextStyle::Body)+6.;
                        egui::ScrollArea::vertical().id_salt("distribution_values").max_height(130.)
                            .show_rows(ui,row_height,distribution.report.ratings.len(),|ui,range| {
                                for index in range {
                                    let value=&distribution.report.ratings[index];
                                    ui.label(format!("{}: {} {} · {} {}",value.value.as_deref().unwrap_or(t(l,"Unknown","Desconhecido")),
                                        value.characters,t(l,"characters","personagens"),value.occurrences,t(l,"occurrences","ocorrências")));
                                }
                            });
                    }
                    let mut counts = std::collections::BTreeMap::new();
                    for row in &snapshot.rows {
                        *counts.entry((&row.name, &row.family)).or_insert(0usize) += 1;
                    }
                    let height = ui.text_style_height(&egui::TextStyle::Body) + 6.;
                    egui::ScrollArea::vertical()
                        .id_salt("search_results")
                        .max_height(220.)
                        .show_rows(
                            ui,
                            height,
                            snapshot.rows.len(),
                            |ui, range| {
                                for index in range {
                                    let row = &snapshot.rows[index];
                                    let mut label = format!("{} · {}", row.name, row.family);
                                    if counts[&(&row.name, &row.family)] > 1 {
                                        label.push_str(&format!(
                                            " · {}",
                                            row.id.chars().take(8).collect::<String>()
                                        ));
                                    }
                                    ui.push_id(&row.id, |ui| {
                                        if ui.button(label).clicked() {
                                            character = Some(row.id.clone());
                                        }
                                    });
                                }
                            }
                        );
                } else {
                    ui.label(t(
                        l,
                        "Run again to refresh results.",
                        "Execute novamente para atualizar os resultados.",
                    ));
                }
            }
            if let Some(message) = &self.searches.message {
                ui.label(message);
            }
            if self.searches.id.is_some() {
                ui.separator();
                if ui
                    .button(t(l, "Delete saved search…", "Excluir busca salva…"))
                    .clicked()
                {
                    self.searches.delete_confirmation = true;
                }
                if self.searches.delete_confirmation {
                    ui.label(t(
                        l,
                        "Delete this saved definition?",
                        "Excluir esta definição salva?",
                    ));
                    ui.horizontal(|ui| {
                        delete = ui
                            .button(t(l, "Delete definition", "Excluir definição"))
                            .clicked();
                        if ui.button(t(l, "Cancel", "Cancelar")).clicked() {
                            self.searches.delete_confirmation = false;
                        }
                    });
                }
            }
        });

        self.searches.open = open;
        if let Some(id) = select {
            self.request_action(PendingAction::SearchRecord(id));
        }
        if save {
            self.save_search();
        }
        if run {
            self.prepare_search();
        }
        if delete {
            self.delete_saved_search();
        }
        if let Some(id) = character {
            self.request_action(PendingAction::SelectRecord(id));
        }
        if export {
            match self.search_document() {
                Ok(html) => {
                    if let Some(path) = FileDialog::new()
                        .set_file_name("Inconnu-private-search.html")
                        .add_filter("HTML", &["html"])
                        .save_file()
                    {
                        self.searches.message = Some(
                            match grapevine_workbench::safe_files::write_new(&path, html.as_bytes())
                            {
                                Ok(()) => t(l, "Report saved.", "Relatório salvo.").into(),
                                Err(e) => e.message,
                            },
                        );
                    }
                }
                Err(e) => self.searches.message = Some(e.message),
            }
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn criterion_controls_clear_values_from_previous_field_and_operator() {
        let mut clause = Clause {
            field: Field::Trait,
            comparison: Comparison::AtLeast,
            negate: true,
            value: "AbilityList".into(),
            trait_name: "Investigation".into(),
            number: 5,
        };
        select_comparison(&mut clause, Comparison::Exists);
        assert!(
            searches::validate(&Query {
                mode: Mode::All,
                clauses: vec![clause.clone()]
            })
            .is_ok()
        );
        for field in [Field::Npc, Field::Name, Field::Player, Field::Trait] {
            select_field(&mut clause, field);
            if field == Field::Trait {
                clause.trait_name = "Investigation".into();
            } else if field != Field::Npc {
                clause.value = "A value".into();
            }
            assert!(
                searches::validate(&Query {
                    mode: Mode::All,
                    clauses: vec![clause.clone()]
                })
                .is_ok()
            );
            if field != Field::Npc {
                select_comparison(&mut clause, Comparison::Exists);
            }
            assert!(
                searches::validate(&Query {
                    mode: Mode::All,
                    clauses: vec![clause.clone()]
                })
                .is_ok()
            );
            assert!(clause.negate);
        }
    }
    #[test]
    fn search_drafts_and_prepared_snapshots_follow_navigation_privacy_and_revision_guards() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("search-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        lib.create_character("Vampire", "Subject", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.open_searches();
        app.searches.name = "Reusable".into();
        assert!(app.has_unsaved_edit());
        app.request_action(PendingAction::SearchRecord(None));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        app.searches.open = false;
        app.language = Language::Portuguese;
        app.open_searches();
        assert_eq!(app.searches.name, "Reusable");
        app.save_search();
        assert!(!app.searches.dirty());
        assert!(app.searches.saved_id().is_some());
        app.prepare_search();
        assert!(app.search_document().is_ok());
        let character = app.searches.prepared().unwrap().rows[0].id.clone();
        app.request_action(PendingAction::SelectRecord(character));
        assert!(app.searches.open && app.searches.recent.is_some());
        assert!(app.search_document().is_ok());
        app.searches.trait_kind = Some(grapevine_workbench::trait_statistics::Kind::Influence);
        assert!(app.search_document().is_err());
        app.prepare_search();
        assert!(app.search_document().unwrap().contains("Influence report"));
        app.searches.trait_kind = None;
        assert!(app.search_document().is_err());
        app.prepare_search();
        app.searches.include_players = true;
        assert!(app.search_document().is_err());
        app.prepare_search();
        assert!(app.search_document().is_ok());
        app.searches.document_language = Language::Portuguese;
        app.searches.statistics = true;
        assert!(app.search_document().unwrap().contains("Relatório privado"));
        let mut writer = Library::open(&path).unwrap();
        writer
            .create_player("Concurrent", 2, Role::Storyteller)
            .unwrap();
        assert!(app.search_document().is_err());
        app.searches.name = "Stale draft".into();
        app.save_search();
        assert!(app.searches.dirty());
        assert!(
            app.searches
                .message
                .as_ref()
                .unwrap()
                .contains("STALE_REVISION")
        );
        app.clear_purchases();
        app.open_searches();
        assert!(app.searches.recent.is_none());
        app.prepare_search();
        app.change_route(WorkArea::Player);
        assert!(!app.searches.open);
        assert!(app.searches.prepared.is_none());
        assert!(app.search_document().is_err());
    }

    #[test]
    fn named_trait_distribution_binds_request_language_and_library_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("distribution-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let id = lib
            .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        let mut sheet = lib.sheet_edit(&id, Role::Storyteller).unwrap();
        let mut list = grapevine_workbench::sheet::empty_list("InfluenceList");
        list["Traits"] = serde_json::json!([{"Name":"Police","Value":"0","Note":"PRIVATE_TRAIT_NOTE"},{"Name":"Police","Value":"unclear"}]);
        sheet.lists.insert("InfluenceList".into(), list);
        lib.save_sheet(&id, &sheet, 1, Role::Storyteller).unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.searches = SearchWindow::synthetic_distribution(false);
        app.searches.distribution_request.as_mut().unwrap().name = "Police".into();
        app.prepare_search();
        assert!(app.search_ready());
        let report = &app.searches.prepared_distribution().unwrap().report;
        assert_eq!(report.known_occurrences, 1);
        assert_eq!(report.unknown_occurrences, 1);
        assert_eq!(report.ratings[0].value.as_deref(), Some("0"));
        app.language = Language::Portuguese;
        app.searches.document_language = Language::Portuguese;
        let html = app.search_document().unwrap();
        assert!(html.contains("Desconhecido"));
        assert!(!html.contains("PRIVATE_TRAIT_NOTE"));
        assert_eq!(app.library.as_ref().unwrap().revision().unwrap(), 2);
        app.searches.distribution_request.as_mut().unwrap().name = "Other".into();
        assert!(!app.search_ready());
        assert!(app.search_document().is_err());
        app.searches.distribution_request.as_mut().unwrap().name = "Police".into();
        let mut other = Library::open(&path).unwrap();
        other
            .create_player("Concurrent", 2, Role::Storyteller)
            .unwrap();
        assert!(!app.search_ready());
        app.prepare_search();
        assert!(app.search_ready());
        app.change_route(WorkArea::Player);
        assert!(app.searches.distribution_request.is_none());
        assert!(app.searches.prepared_distribution().is_none());
        assert!(app.search_document().is_err());
    }
}
