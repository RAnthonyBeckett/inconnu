use super::*;
use grapevine_workbench::associations::{self, Kind, Link, Snapshot};

#[derive(Default)]
pub(super) struct AssociationWindow {
    pub open: bool,
    snapshot: Option<Snapshot>,
    pub draft: Vec<Link>,
    saved: Vec<Link>,
    kind: Option<Kind>,
    target: Option<String>,
    search: String,
    document_language: Option<Language>,
    notes: bool,
    pub message: Option<String>,
}
impl AssociationWindow {
    pub(super) fn dirty(&self) -> bool {
        self.draft != self.saved
    }
    pub(super) fn character_id(&self) -> Option<&str> {
        self.snapshot.as_ref().map(|s| s.character_id.as_str())
    }
    pub(super) fn synthetic(snapshot: Snapshot, target: Summary) -> Self {
        Self {
            open: true,
            snapshot: Some(snapshot),
            draft: vec![Link {
                target_id: target.id,
                kind: Kind::Item,
                label_at_binding: target.name,
                quantity: 2,
                note: "SYNTHETIC_ASSOCIATION_NOTE".into(),
            }],
            document_language: Some(Language::Portuguese),
            ..Default::default()
        }
    }
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
fn label(kind: Kind, language: Language) -> &'static str {
    grapevine_workbench::world_card::label(kind.record_kind(), language)
}

impl InconnuApp {
    pub(super) fn open_associations(&mut self) {
        if self.associations.snapshot.is_none() {
            let id = self.selected_id.clone().filter(|id| {
                self.summaries.iter().any(|s| {
                    s.id == *id
                        && s.build.is_none()
                        && grapevine_workbench::sheet::is_character_kind(&s.kind)
                })
            });
            self.load_associations(id);
        } else {
            self.associations.open = true;
        }
    }

    pub(super) fn load_associations(&mut self, id: Option<String>) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let loaded = id
            .map(|id| lib.association_snapshot(&id, Role::Storyteller))
            .transpose();
        match loaded {
            Ok(snapshot) => {
                let draft = snapshot.as_ref().map_or_else(Vec::new, |s| {
                    s.linked.iter().map(|r| r.link.clone()).collect()
                });
                self.associations = AssociationWindow {
                    open: true,
                    saved: draft.clone(),
                    draft,
                    snapshot,
                    document_language: self.associations.document_language,
                    ..Default::default()
                };
            }
            Err(e) => self.log_error("association.read", &e),
        }
    }

    fn save_association_draft(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.associations.message = Some(
                t(
                    self.language,
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(snapshot)) = (&mut self.library, &self.associations.snapshot) else {
            return;
        };
        let id = snapshot.character_id.clone();
        let result = lib.save_associations(
            &id,
            &self.associations.draft,
            snapshot.revision,
            Role::Storyteller,
        );
        match result {
            Ok(revision) => {
                self.revision = Some(revision);
                self.associations.saved = self.associations.draft.clone();
                match lib.association_snapshot(&id, Role::Storyteller) {
                    Ok(fresh) => {
                        self.associations.draft =
                            fresh.linked.iter().map(|r| r.link.clone()).collect();
                        self.associations.saved = self.associations.draft.clone();
                        self.associations.snapshot = Some(fresh);
                        self.associations.message =
                            Some(t(self.language, "Assignments saved.", "Vínculos salvos.").into());
                    }
                    Err(e) => {
                        self.associations.message = Some(format!(
                            "Saved at revision {revision}; reopen: {}",
                            e.message
                        ))
                    }
                }
            }
            Err(e) => self.associations.message = Some(format!("{}: {}", e.code, e.message)),
        }
    }

    fn association_document(&self) -> grapevine_workbench::model::Result<String> {
        let not_ready = || {
            grapevine_workbench::model::error(
                "ASSOCIATION_REPORT_NOT_READY",
                "Save changes and reopen the character if the library changed.",
            )
        };
        if self.route != WorkArea::Storyteller || self.associations.dirty() {
            return Err(not_ready());
        }
        let lib = self.library.as_ref().ok_or_else(not_ready)?;
        let shown = self.associations.snapshot.as_ref().ok_or_else(not_ready)?;
        let current = lib.association_snapshot(&shown.character_id, Role::Storyteller)?;
        if current.revision != shown.revision {
            return Err(not_ready());
        }
        associations::html(
            &current,
            self.associations.document_language.unwrap_or(self.language),
            self.associations.notes,
        )
    }

    pub(super) fn render_associations(&mut self, ctx: &egui::Context) {
        if !self.associations.open || self.route != WorkArea::Storyteller || self.library.is_none()
        {
            return;
        }
        let l = self.language;
        let mut open = true;
        let mut select = None;
        let mut save = false;
        let mut print = false;
        egui::Window::new(t(l,"Character associations","Vínculos do personagem"))
            .id(egui::Id::unique("associations_window")).default_size([760.,540.]).vscroll(true).open(&mut open).show(ctx,|ui|{
                ui.horizontal_wrapped(|ui|{
                    let title=self.associations.snapshot.as_ref().map(|s|s.character_name.as_str())
                        .unwrap_or(t(l,"Choose a character","Escolher personagem"));
                    egui::ComboBox::from_id_salt("association_character").selected_text(title).show_ui(ui,|ui|{
                        for row in self.summaries.iter().filter(|s|s.build.is_none()&&grapevine_workbench::sheet::is_character_kind(&s.kind)){
                            if ui.selectable_label(self.associations.character_id()==Some(row.id.as_str()),&row.name).clicked(){
                                select=Some(row.id.clone());
                            }
                        }
                    });
                    let b=ui.add_enabled(self.associations.dirty(),egui::Button::new(t(l,"Save assignments","Salvar vínculos")));
                    set_probe(ui,&b,"association_save");
                    save=b.clicked();
                    if self.associations.dirty(){ui.label(t(l,"Unsaved changes","Alterações não salvas"));}
                });
                ui.horizontal_wrapped(|ui|{
                    let lang=self.associations.document_language.get_or_insert(l);
                    ui.selectable_value(lang,Language::English,"English");
                    ui.selectable_value(lang,Language::Portuguese,"Português");
                    ui.checkbox(&mut self.associations.notes,t(l,"Include private notes","Incluir notas privadas"));
                    let b=ui.add_enabled(self.associations.snapshot.is_some()&&!self.associations.dirty(),
                        egui::Button::new(t(l,"Save private equipment report…","Salvar relatório privado de equipamento…")));
                    set_probe(ui,&b,"association_report");
                    print=b.clicked();
                });
                if let Some(msg)=&self.associations.message{ui.label(msg);}
                let Some(snapshot)=&self.associations.snapshot else {return;};
                ui.separator();
                ui.heading(t(l,"Explicit local assignments","Vínculos locais explícitos"));
                ui.label(t(l,"Choose the exact record. Assignments do not establish ownership or change imported labels.",
                    "Escolha o registro exato. Os vínculos não comprovam propriedade nem alteram os nomes importados."));
                add_target(ui,l,&self.summaries,&mut self.associations.kind,&mut self.associations.target,&mut self.associations.search,&mut self.associations.draft);
                let mut remove=None;
                for (i,link) in self.associations.draft.iter_mut().enumerate(){
                    let old=snapshot.linked.iter().find(|r|r.link.target_id==link.target_id);
                    let current=old.and_then(|r|r.current.as_ref()).map(|e|e.name.as_str()).unwrap_or(&link.label_at_binding);
                    let missing=old.is_some_and(|r|r.current.is_none());
                    ui.push_id(i,|ui|{
                        ui.horizontal_wrapped(|ui|{
                            ui.strong(format!("{} · {}",label(link.kind,l),current));
                            if missing {ui.label(t(l,"Saved target unavailable","Destino salvo indisponível"));}
                            if link.kind==Kind::Item {
                                ui.label(t(l,"Quantity","Quantidade"));
                                ui.add_enabled(!missing,egui::DragValue::new(&mut link.quantity).range(1..=999));
                            }
                            if ui.button(t(l,"Remove link","Remover vínculo")).clicked(){remove=Some(i);}
                        });
                        if current!=link.label_at_binding{
                            ui.label(format!("{}: {}",t(l,"Recorded as","Registrado como"),link.label_at_binding));
                        }
                        ui.add_enabled(!missing,egui::TextEdit::singleline(&mut link.note).hint_text(t(l,"Private assignment note","Nota privada do vínculo")));
                    });
                }
                if let Some(i)=remove{self.associations.draft.remove(i);}
                ui.separator();
                ui.heading(t(l,"Unresolved legacy labels","Nomes legados sem vínculo"));
                ui.label(t(l,"These are shown separately and are never counted as assignments.","Estes nomes aparecem separados e não contam como vínculos."));
                if snapshot.legacy.is_empty(){ui.label(t(l,"No legacy labels in this sheet.","Nenhum nome legado nesta ficha."));}
                for row in &snapshot.legacy{
                    ui.label(format!("{} · {} · {}",label(row.kind,l),row.label,row.value));
                }
                if snapshot.retained>0{
                    ui.label(format!("{}: {}",t(l,"Extra details retained in source","Detalhes adicionais preservados na origem"),snapshot.retained));
                }
            });
        self.associations.open = open;
        if let Some(id) = select {
            self.request_action(PendingAction::AssociationRecord(Some(id)));
        }
        if save {
            self.save_association_draft();
        }
        if print {
            match self.association_document() {
                Ok(html) => {
                    if let Some(path) = FileDialog::new()
                        .set_file_name("Inconnu-private-equipment.html")
                        .add_filter("HTML", &["html"])
                        .save_file()
                    {
                        self.associations.message = Some(
                            match grapevine_workbench::safe_files::write_new(&path, html.as_bytes())
                            {
                                Ok(()) => t(
                                    l,
                                    "Private equipment report saved.",
                                    "Relatório privado de equipamento salvo.",
                                )
                                .into(),
                                Err(e) => e.message,
                            },
                        );
                    }
                }
                Err(e) => self.associations.message = Some(e.message),
            }
        }
    }
}

fn set_probe(ui: &egui::Ui, b: &egui::Response, key: &'static str) {
    let rect = ui.clip_rect().contains_rect(b.rect).then_some(b.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), rect));
}
fn add_target(
    ui: &mut egui::Ui,
    l: Language,
    rows: &[Summary],
    kind: &mut Option<Kind>,
    target: &mut Option<String>,
    search: &mut String,
    draft: &mut Vec<Link>,
) {
    ui.horizontal_wrapped(|ui| {
        let selected = kind.unwrap_or(Kind::Item);
        egui::ComboBox::from_id_salt("association_kind")
            .selected_text(label(selected, l))
            .show_ui(ui, |ui| {
                for choice in [Kind::Item, Kind::Rote, Kind::Location] {
                    if ui
                        .selectable_label(choice == selected, label(choice, l))
                        .clicked()
                    {
                        *kind = Some(choice);
                        *target = None;
                    }
                }
            });
        let selected = kind.unwrap_or(Kind::Item);
        let selected_row = rows
            .iter()
            .find(|s| Some(&s.id) == target.as_ref() && s.kind == selected.record_kind());
        let choices: Vec<_> = rows
            .iter()
            .filter(|s| s.kind == selected.record_kind() && s.build.is_none())
            .collect();
        let mut counts = std::collections::HashMap::new();
        for row in &choices {
            *counts.entry(row.name.as_str()).or_insert(0usize) += 1;
        }
        let display = |row: &Summary| {
            if counts.get(row.name.as_str()).copied().unwrap_or_default() > 1 {
                format!(
                    "{} · {}",
                    row.name,
                    row.id.chars().take(8).collect::<String>()
                )
            } else {
                row.name.clone()
            }
        };
        egui::ComboBox::from_id_salt("association_target")
            .selected_text(selected_row.map_or_else(
                || t(l, "Choose a record", "Escolher registro").into(),
                display,
            ))
            .show_ui(ui, |ui| {
                ui.add(egui::TextEdit::singleline(search).hint_text(t(
                    l,
                    "Search records",
                    "Buscar registros",
                )));
                let query = search.to_lowercase();
                for row in choices {
                    if row.name.to_lowercase().contains(&query)
                        && ui
                            .selectable_label(Some(&row.id) == target.as_ref(), display(row))
                            .clicked()
                    {
                        *target = Some(row.id.clone());
                    }
                }
            });
        if let Some(row) = rows
            .iter()
            .find(|s| Some(&s.id) == target.as_ref() && s.kind == selected.record_kind())
        {
            let can_add = draft.len() < 256 && !draft.iter().any(|link| link.target_id == row.id);
            if ui
                .add_enabled(
                    can_add,
                    egui::Button::new(t(l, "Add assignment", "Adicionar vínculo")),
                )
                .clicked()
            {
                draft.push(Link {
                    target_id: row.id.clone(),
                    kind: selected,
                    label_at_binding: row.name.clone(),
                    quantity: 1,
                    note: String::new(),
                });
                *target = None;
            }
        }
    });
}
