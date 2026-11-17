use super::*;
use grapevine_workbench::{
    actions::{Action, ActivitySlot},
    rumors::{Entry, Rumor},
    story_work::{self, Draft, Kind, Snapshot},
};

pub(super) struct StoryWindow {
    pub open: bool,
    pub target: Option<String>,
    pub draft: Draft,
    saved: Draft,
    pub(super) character_id: Option<String>,
    saved_character_id: Option<String>,
    pub(super) binding_missing: bool,
    pub(super) timeline: Option<grapevine_workbench::action_links::Timeline>,
    pub(super) include_rumors: bool,
    pub(super) combined: Option<grapevine_workbench::combined_story::Snapshot>,
    pub(super) prior_action_id: String,
    pub(super) copy_options: grapevine_workbench::action_links::CopyOptions,
    pub(super) copy_preview: Option<grapevine_workbench::action_links::CopyPreview>,
    pub(super) confirm_copy: bool,
    pub(super) revision: Option<i64>,
    retained: bool,
    legacy: bool,
    search: String,
    pub(super) from: String,
    pub(super) through: String,
    incomplete_only: bool,
    pub(super) document_language: Option<Language>,
    pub message: Option<String>,
}

impl Default for StoryWindow {
    fn default() -> Self {
        Self::new(Kind::Action)
    }
}
impl StoryWindow {
    fn new(kind: Kind) -> Self {
        Self {
            open: false,
            target: None,
            draft: Draft::new(kind),
            saved: Draft::new(kind),
            character_id: None,
            saved_character_id: None,
            binding_missing: false,
            timeline: None,
            include_rumors: false,
            combined: None,
            prior_action_id: String::new(),
            copy_options: Default::default(),
            copy_preview: None,
            confirm_copy: false,
            revision: None,
            retained: false,
            legacy: false,
            search: String::new(),
            from: String::new(),
            through: String::new(),
            incomplete_only: false,
            document_language: None,
            message: None,
        }
    }
    pub(super) fn dirty(&self) -> bool {
        self.saved != self.draft || self.character_id != self.saved_character_id
    }
    pub(super) fn kind(&self) -> Kind {
        self.draft.kind()
    }
    pub(super) fn synthetic_linked(
        character: Summary,
        source_id: String,
        revision: Option<i64>,
    ) -> Self {
        let mut window = Self::synthetic(Kind::Action, revision);
        window.character_id = Some(character.id);
        window.prior_action_id = source_id;
        window.copy_options = grapevine_workbench::action_links::CopyOptions {
            carry_unused: true,
            carry_growth: true,
        };
        if let Draft::Action(action) = &mut window.draft {
            action.character = character.name;
            action.day = "2026-11-11".into();
            action.slots.clear();
        }
        window
    }
    pub(super) fn synthetic(kind: Kind, revision: Option<i64>) -> Self {
        let draft = match kind {
            Kind::Action => Draft::Action(Action {
                character: "Synthetic Actor".into(),
                day: "2026-11-10".into(),
                slots: vec![ActivitySlot {
                    name: "Investigation".into(),
                    level: "2".into(),
                    unused: "0".into(),
                    total: "2".into(),
                    growth: "0".into(),
                    action: "Ask about <literal> clues.".into(),
                    result: "Found a key.".into(),
                }],
            }),
            Kind::Rumor => Draft::Rumor(Rumor {
                title: "Synthetic rumor".into(),
                day: "2026-11-10".into(),
                category: 1,
                audience_label: "Declared local audience".into(),
                influence_list: "Influence".into(),
                influence_trait: "Records".into(),
                reviewed: true,
                entries: vec![Entry {
                    level: "2".into(),
                    body: "The key bears <literal> markings.".into(),
                }],
            }),
        };
        Self {
            open: true,
            revision,
            draft,
            from: "2026-11-01".into(),
            through: "2026-11-30".into(),
            document_language: Some(Language::Portuguese),
            ..Self::new(kind)
        }
    }
}

fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}

impl InconnuApp {
    pub(super) fn open_story(&mut self, kind: Kind) {
        if self.story.revision.is_some() && self.story.kind() == kind {
            self.story.open = true;
        } else {
            self.request_action(PendingAction::StoryRecord(kind, None));
        }
    }

    pub(super) fn load_story(&mut self, kind: Kind, id: Option<String>) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let result = match &id {
            Some(id) if kind == Kind::Action => lib
                .linked_action_snapshot(id, Role::Storyteller)
                .map(|linked| (linked.story, linked.binding)),
            Some(id) => lib
                .story_snapshot(id, Role::Storyteller)
                .map(|snapshot| (snapshot, None)),
            None => lib.revision().map(|revision| {
                (
                    Snapshot {
                        revision,
                        draft: Draft::new(kind),
                        legacy: false,
                        retained: false,
                    },
                    None,
                )
            }),
        };
        match result {
            Ok((s, binding)) => {
                let character_id = binding
                    .as_ref()
                    .map(|resolved| resolved.binding.character_id.clone());
                let binding_missing = binding
                    .as_ref()
                    .is_some_and(|resolved| resolved.current_name.is_none());
                self.story = StoryWindow {
                    character_id: character_id.clone(),
                    saved_character_id: character_id,
                    binding_missing,
                    open: true,
                    target: id,
                    draft: s.draft.clone(),
                    saved: s.draft,
                    revision: Some(s.revision),
                    legacy: s.legacy,
                    retained: s.retained,
                    document_language: self.story.document_language,
                    from: self.story.from.clone(),
                    through: self.story.through.clone(),
                    ..StoryWindow::new(kind)
                };
            }
            Err(e) => self.log_error("story.read", &e),
        }
    }

    pub(super) fn save_story_draft(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.story.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(revision)) = (&mut self.library, self.story.revision) else {
            return;
        };
        let result = match (&self.story.target, &self.story.draft) {
            (Some(id), Draft::Action(action)) => lib
                .save_linked_action(
                    id,
                    action,
                    self.story.character_id.as_deref(),
                    revision,
                    Role::Storyteller,
                )
                .map(|_| id.clone()),
            (None, Draft::Action(action)) => lib.create_linked_action(
                action,
                self.story.character_id.as_deref(),
                revision,
                Role::Storyteller,
            ),
            (Some(id), draft) => lib
                .save_story(id, draft, revision, Role::Storyteller)
                .map(|_| id.clone()),
            (None, draft) => lib.create_story(draft, revision, Role::Storyteller),
        };
        match result {
            Ok(id) => {
                self.story.target = Some(id);
                self.story.saved = self.story.draft.clone();
                self.story.saved_character_id = self.story.character_id.clone();
                self.story.timeline = None;
                self.story.combined = None;
                self.story.copy_preview = None;
                self.story.revision = Some(revision + 1);
                self.revision = self.story.revision;
                self.story.message =
                    Some(t(self.language, "Draft saved.", "Rascunho salvo.").into());
                self.refresh_summaries(true);
            }
            Err(e) => self.story.message = Some(format!("{}: {}", e.code, e.message)),
        }
    }

    fn story_document(&self, master: bool) -> grapevine_workbench::model::Result<String> {
        let err = || {
            grapevine_workbench::model::error(
                "STORY_REPORT_NOT_READY",
                "Save the draft and reopen it if the library changed.",
            )
        };
        if self.route != WorkArea::Storyteller || self.story.dirty() {
            return Err(err());
        }
        let lib = self.library.as_ref().ok_or_else(err)?;
        let lang = self.story.document_language.unwrap_or(self.language);
        if master {
            let s = lib.master_story(
                self.story.kind(),
                &self.story.from,
                &self.story.through,
                self.story.incomplete_only,
                Role::Storyteller,
            )?;
            story_work::master_html(&s, lang)
        } else {
            let id = self.story.target.as_ref().ok_or_else(err)?;
            let s = if self.story.kind() == Kind::Action {
                lib.linked_action_snapshot(id, Role::Storyteller)?.story
            } else {
                lib.story_snapshot(id, Role::Storyteller)?
            };
            if Some(s.revision) != self.story.revision {
                return Err(err());
            }
            story_work::html(&s, lang)
        }
    }

    pub(super) fn render_story(&mut self, ctx: &egui::Context) {
        if !self.story.open || self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        let kind = self.story.kind();
        let l = self.language;
        let title = match kind {
            Kind::Action => t(l, "Actions", "Ações"),
            Kind::Rumor => t(l, "Rumors", "Rumores"),
        };
        let rows: Vec<_> = self
            .summaries
            .iter()
            .filter(|r| r.kind == kind.record_kind())
            .map(|r| (r.id.clone(), r.name.clone()))
            .collect();
        let mut open = true;
        let mut switch = None;
        let mut save = false;
        let mut print = None;
        let mut audience = None;
        egui::Window::new(title).id(egui::Id::unique("story_window")).default_size([760.,545.])
            .open(&mut open).vscroll(true).show(ctx,|ui|{
                ui.horizontal_wrapped(|ui|{
                    if ui.button(t(l,"New draft","Novo rascunho")).clicked(){switch=Some(None);}
                    let current=rows.iter().find(|(id,_)|Some(id)==self.story.target.as_ref()).map_or(
                        t(l,"Choose saved record","Escolher registro salvo"),|(_,name)|name);
                    egui::ComboBox::from_id_salt("story_select").selected_text(current).show_ui(ui,|ui|{
                        ui.add(egui::TextEdit::singleline(&mut self.story.search).hint_text(t(l,"Search names","Buscar nomes")));
                        let query=self.story.search.to_lowercase();
                        for (id,name) in &rows{
                            if name.to_lowercase().contains(&query) && ui.selectable_label(self.story.target.as_ref()==Some(id),name).clicked(){
                                switch=Some(Some(id.clone()));
                            }
                        }
                    });
                    let b=ui.add_enabled(self.story.dirty(),egui::Button::new(t(l,"Save draft","Salvar rascunho")));
                    set_probe(ui,&b,"story_save");
                    save=b.clicked();
                    if self.story.dirty(){ui.label(t(l,"Unsaved changes","Alterações não salvas"));}
                });
                ui.horizontal_wrapped(|ui|{
                    let lang=self.story.document_language.get_or_insert(l);
                    ui.selectable_value(lang,Language::English,"English");
                    ui.selectable_value(lang,Language::Portuguese,"Português");
                    let b=ui.add_enabled(self.story.target.is_some()&&!self.story.dirty(),
                        egui::Button::new(t(l,"Save private report…","Salvar relatório privado…")));
                    set_probe(ui,&b,"story_report");
                    if b.clicked(){print=Some(false);}
                    if kind == Kind::Rumor && ui.add_enabled(self.story.target.is_some() && !self.story.dirty(),
                        egui::Button::new(t(l,"Review audience…","Revisar público…"))).clicked(){
                        audience=self.story.target.clone();
                    }
                });
                egui::CollapsingHeader::new(t(l,"Master report by date","Relatório geral por data")).id_salt("story_master").show(ui,|ui|{
                    ui.horizontal_wrapped(|ui|{
                        small_field(ui,t(l,"From","De"),&mut self.story.from,100.);
                        small_field(ui,t(l,"Through","Até"),&mut self.story.through,100.);
                    });
                    ui.label(t(l,"Use YYYY-MM-DD for both dates. Includes saved drafts only.","Use AAAA-MM-DD nas duas datas. Inclui apenas rascunhos salvos."));
                    ui.checkbox(&mut self.story.incomplete_only,t(l,"Incomplete only","Somente incompletos"));
                    if ui.add_enabled(!self.story.dirty(),egui::Button::new(t(l,"Save private master report…","Salvar relatório geral privado…"))).clicked(){print=Some(true);}
                });
                if let Some(msg)=&self.story.message{ui.label(msg);}
                if self.story.legacy || self.story.retained{
                    ui.label(t(l,"Extra legacy data stays in the source. Correct ambiguous dates before saving.","Dados legados adicionais ficam na origem. Corrija datas ambíguas antes de salvar."));
                }
                ui.separator();
                if kind == Kind::Action { self.render_action_context(ui); }
                match &mut self.story.draft{
                    Draft::Action(a)=>render_action(ui,l,a),
                    Draft::Rumor(r)=>render_rumor(ui,l,r),
                }
            });
        self.story.open = open;
        if let Some(id) = audience {
            self.request_action(PendingAction::RumorAudience(id));
        }
        if let Some(id) = switch {
            self.request_action(PendingAction::StoryRecord(kind, id));
        }
        if save {
            self.save_story_draft();
        }
        if let Some(master) = print {
            match self.story_document(master) {
                Ok(html) => {
                    if let Some(path) = FileDialog::new()
                        .set_file_name("Inconnu-private-draft.html")
                        .add_filter("HTML", &["html"])
                        .save_file()
                    {
                        self.story.message = Some(
                            match grapevine_workbench::safe_files::write_new(&path, html.as_bytes())
                            {
                                Ok(()) => t(
                                    l,
                                    "Private report saved for browser print/PDF.",
                                    "Relatório privado salvo para impressão/PDF no navegador.",
                                )
                                .into(),
                                Err(e) => e.message,
                            },
                        );
                    }
                }
                Err(e) => self.story.message = Some(e.message),
            }
        }
    }
}

fn set_probe(ui: &egui::Ui, b: &egui::Response, key: &'static str) {
    let rect = ui.clip_rect().contains_rect(b.rect).then_some(b.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), rect));
}
fn small_field(ui: &mut egui::Ui, label: &str, text: &mut String, width: f32) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(text).desired_width(width));
}
fn text_box(ui: &mut egui::Ui, label: &str, text: &mut String) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(text)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
}

fn render_action(ui: &mut egui::Ui, l: Language, a: &mut Action) {
    ui.label(if a.is_complete() {
        t(
            l,
            "Action and result text complete.",
            "Textos da ação e do resultado completos.",
        )
    } else {
        t(l, "Incomplete action text.", "Texto da ação incompleto.")
    });
    ui.label(t(
        l,
        "The amounts are recorded values. Review previous-value previews before using them.",
        "Os valores são registrados. Revise a prévia dos valores anteriores antes de usá-los.",
    ));
    let mut remove = None;
    for (i, s) in a.slots.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                small_field(ui, t(l, "Activity", "Atividade"), &mut s.name, 210.);
                if ui
                    .button(t(l, "Remove activity", "Remover atividade"))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
            ui.horizontal_wrapped(|ui| {
                small_field(ui, t(l, "Level", "Nível"), &mut s.level, 45.);
                small_field(ui, t(l, "Unused", "Não usado"), &mut s.unused, 45.);
                small_field(ui, "Total", &mut s.total, 45.);
                small_field(ui, t(l, "Growth", "Crescimento"), &mut s.growth, 45.);
            });
            text_box(ui, t(l, "Action", "Ação"), &mut s.action);
            text_box(ui, t(l, "Result", "Resultado"), &mut s.result);
        });
    }
    if let Some(i) = remove {
        a.slots.remove(i);
    }
    if a.slots.len() < 1000
        && ui
            .button(t(l, "Add activity", "Adicionar atividade"))
            .clicked()
    {
        a.slots.push(ActivitySlot {
            level: "0".into(),
            unused: "0".into(),
            total: "0".into(),
            growth: "0".into(),
            ..Default::default()
        });
    }
}

fn render_rumor(ui: &mut egui::Ui, l: Language, r: &mut Rumor) {
    ui.horizontal_wrapped(|ui| {
        small_field(ui, t(l, "Title", "Título"), &mut r.title, 235.);
        small_field(
            ui,
            t(l, "Date (YYYY-MM-DD)", "Data (AAAA-MM-DD)"),
            &mut r.day,
            105.,
        );
    });
    egui::ComboBox::from_id_salt("rumor_category")
        .selected_text(story_work::category_label(r.category, l))
        .show_ui(ui, |ui| {
            for code in 0..=5 {
                ui.selectable_value(&mut r.category, code, story_work::category_label(code, l));
            }
        });
    ui.horizontal_wrapped(|ui| {
        small_field(
            ui,
            t(l, "Audience label", "Nome do público"),
            &mut r.audience_label,
            220.,
        );
        ui.checkbox(
            &mut r.reviewed,
            t(l, "Locally reviewed", "Revisado localmente"),
        );
    });
    if r.category == 1 || !r.influence_list.is_empty() || !r.influence_trait.is_empty() {
        ui.horizontal_wrapped(|ui| {
            small_field(
                ui,
                t(l, "Influence list", "Lista de influência"),
                &mut r.influence_list,
                125.,
            );
            small_field(
                ui,
                t(l, "Influence trait", "Característica de influência"),
                &mut r.influence_trait,
                130.,
            );
        });
    }
    ui.label(t(
        l,
        "Audience labels are notes. Recipients are not looked up or sent anything.",
        "Os nomes de público são notas. Nenhum destinatário é pesquisado ou recebe mensagens.",
    ));
    let mut remove = None;
    for (i, e) in r.entries.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                small_field(ui, t(l, "Level", "Nível"), &mut e.level, 60.);
                if ui.button(t(l, "Remove entry", "Remover entrada")).clicked() {
                    remove = Some(i);
                }
            });
            text_box(ui, t(l, "Rumor text", "Texto do rumor"), &mut e.body);
        });
    }
    if let Some(i) = remove {
        r.entries.remove(i);
    }
    if r.entries.len() < grapevine_workbench::rumors::ROW_LIMIT
        && ui.button(t(l, "Add entry", "Adicionar entrada")).clicked()
    {
        r.entries.push(Entry {
            level: "0".into(),
            body: String::new(),
        });
    }
}
