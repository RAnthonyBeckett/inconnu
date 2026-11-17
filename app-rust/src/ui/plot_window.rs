use super::*;
use grapevine_workbench::plots::{self, Cast, Development, Plot, Snapshot};

#[derive(Default)]
pub(super) struct PlotWindow {
    pub open: bool,
    pub target: Option<String>,
    pub draft: Plot,
    saved: Plot,
    revision: Option<i64>,
    legacy: bool,
    retained: bool,
    as_of: String,
    document_language: Option<Language>,
    notes: bool,
    pub message: Option<String>,
}

impl PlotWindow {
    pub(super) fn synthetic(revision: Option<i64>) -> Self {
        let draft = Plot {
            name: "Synthetic plot".into(),
            narrator: "Sample Narrator".into(),
            start: "2026-11-09".into(),
            end: "2026-11-12".into(),
            outline: "The chronicle begins at a locked door.".into(),
            cast: vec![Cast {
                label: "A guest".into(),
                value: "Witness".into(),
                note: "SYNTHETIC_CAST_NOTE".into(),
            }],
            developments: vec![
                Development {
                    day: "2026-11-11".into(),
                    event: "The key was found.".into(),
                },
                Development {
                    day: "2026-11-10".into(),
                    event: "A message contained <literal> text.".into(),
                },
            ],
        };
        Self {
            open: true,
            revision,
            draft,
            as_of: "2026-11-10".into(),
            document_language: Some(Language::Portuguese),
            ..Self::default()
        }
    }

    pub(super) fn revision_is_set(&self) -> bool {
        self.revision.is_some()
    }
    pub(super) fn dirty(&self) -> bool {
        self.draft != self.saved
    }
}

impl InconnuApp {
    pub(super) fn load_plot(&mut self, id: Option<String>) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        let snapshot = match &id {
            Some(id) => lib.plot_snapshot(id, Role::Storyteller),
            None => lib.revision().map(|revision| Snapshot {
                revision,
                plot: Plot::default(),
                legacy: false,
                retained: false,
            }),
        };
        match snapshot {
            Ok(s) => {
                self.plot = PlotWindow {
                    open: true,
                    target: id,
                    draft: s.plot.clone(),
                    saved: s.plot,
                    revision: Some(s.revision),
                    legacy: s.legacy,
                    retained: s.retained,
                    document_language: self.plot.document_language,
                    as_of: self.plot.as_of.clone(),
                    ..Default::default()
                };
            }
            Err(e) => self.log_error("plot.read", &e),
        }
    }

    fn save_plot_draft(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet {
            self.plot.message = Some(
                self.t(
                    "Save or discard the sheet draft first.",
                    "Salve ou descarte o rascunho da ficha primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(revision)) = (&mut self.library, self.plot.revision) else {
            return;
        };
        let result = if let Some(id) = &self.plot.target {
            lib.save_plot(id, &self.plot.draft, revision, Role::Storyteller)
                .map(|_| id.clone())
        } else {
            lib.create_plot(&self.plot.draft, revision, Role::Storyteller)
        };
        match result {
            Ok(id) => {
                self.plot.target = Some(id);
                self.plot.draft.name = self.plot.draft.name.trim().to_owned();
                self.plot.draft.order_timeline();
                self.plot.saved = self.plot.draft.clone();
                self.plot.revision = Some(revision + 1);
                self.revision = self.plot.revision;
                self.plot.message = Some(self.t("Plot saved.", "Trama salva.").into());
                self.refresh_summaries(true);
            }
            Err(e) => self.plot.message = Some(format!("{}: {}", e.code, e.message)),
        }
    }

    pub(super) fn plot_html(&self) -> grapevine_workbench::model::Result<String> {
        let err = || {
            grapevine_workbench::model::error(
                "PLOT_REPORT_NOT_READY",
                "Save a plot and reopen it if the library changed.",
            )
        };
        if self.route != WorkArea::Storyteller || self.plot.dirty() {
            return Err(err());
        }
        let lib = self.library.as_ref().ok_or_else(err)?;
        let target = self.plot.target.as_ref().ok_or_else(err)?;
        let s = lib.plot_snapshot(target, Role::Storyteller)?;
        if Some(s.revision) != self.plot.revision {
            return Err(err());
        }
        plots::html(
            &s,
            &self.plot.as_of,
            self.plot.document_language.unwrap_or(self.language),
            self.plot.notes,
        )
    }

    pub(super) fn render_plots(&mut self, ctx: &egui::Context) {
        if !self.plot.open || self.route != WorkArea::Storyteller || self.library.is_none() {
            return;
        }
        let language = self.language;
        let t = |en, pt| {
            if language == Language::Portuguese {
                pt
            } else {
                en
            }
        };
        let rows: Vec<_> = self
            .summaries
            .iter()
            .filter(|r| r.kind == "Plot")
            .map(|r| (r.id.clone(), r.name.clone()))
            .collect();
        let mut open = true;
        let mut switch = None;
        let mut save = false;
        let mut export = false;
        egui::Window::new(t("Plots","Tramas")).id(egui::Id::unique("plot_window"))
            .default_size([760.,550.]).vscroll(true).open(&mut open).show(ctx,|ui|{
            ui.horizontal_wrapped(|ui|{
                if ui.button(t("New plot","Nova trama")).clicked(){switch=Some(None);}
                let label=rows.iter().find(|(id,_)|Some(id)==self.plot.target.as_ref()).map_or(t("Choose a saved plot","Escolher uma trama salva"),|(_,name)|name);
                egui::ComboBox::from_id_salt("plot_select").selected_text(label).show_ui(ui,|ui|{
                    for (id,name) in &rows{
                        if ui.selectable_label(Some(id)==self.plot.target.as_ref(),name).clicked(){switch=Some(Some(id.clone()));}
                    }
                });
                let b=ui.add_enabled(self.plot.dirty(),egui::Button::new(t("Save plot","Salvar trama")));
                let visible=ui.clip_rect().contains_rect(b.rect).then_some(b.rect);
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("plot_save"),visible));
                save=b.clicked();
                if self.plot.dirty(){ui.label(t("Unsaved changes","Alterações não salvas"));}
            });
            ui.horizontal_wrapped(|ui|{
                ui.label(t("Report date (YYYY-MM-DD)","Data do relatório (AAAA-MM-DD)"));
                ui.add(egui::TextEdit::singleline(&mut self.plot.as_of).desired_width(95.));
                let doc=self.plot.document_language.get_or_insert(language);
                ui.selectable_value(doc,Language::English,"English");
                ui.selectable_value(doc,Language::Portuguese,"Português");
            });
            ui.horizontal_wrapped(|ui|{
                ui.checkbox(&mut self.plot.notes,t("Include cast notes","Incluir notas do elenco"));
                let b=ui.add_enabled(self.plot.target.is_some()&&!self.plot.dirty(),egui::Button::new(t("Save private report…","Salvar relatório privado…")));
                let visible=ui.clip_rect().contains_rect(b.rect).then_some(b.rect);
                ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique("plot_report"),visible));
                export=b.clicked();
            });
            if let Some(message)=&self.plot.message{ui.label(message);}
            if self.plot.legacy{
                ui.label(t("Original plot data is retained. Use YYYY-MM-DD to correct ambiguous dates. Cast entries are labels, not links.","Os dados originais são preservados. Use AAAA-MM-DD para corrigir datas ambíguas. O elenco contém nomes, não vínculos."));
            }
            if self.plot.retained{
                ui.label(t("Extra legacy details remain in the source and are excluded from the report.","Detalhes legados adicionais permanecem na origem e não aparecem no relatório."));
            }
            ui.separator();
            let p=&mut self.plot.draft;
            ui.label(t("Title","Título"));ui.text_edit_singleline(&mut p.name);
            ui.label(t("Narrator","Narrador"));ui.text_edit_singleline(&mut p.narrator);
            ui.horizontal_wrapped(|ui|{
                ui.label(t("Start","Início"));ui.add(egui::TextEdit::singleline(&mut p.start).desired_width(110.));
                ui.label(t("End","Fim"));ui.add(egui::TextEdit::singleline(&mut p.end).desired_width(110.));
            });
            ui.label(t("Outline","Resumo"));
            ui.add(egui::TextEdit::multiline(&mut p.outline).desired_rows(3).desired_width(f32::INFINITY));
            egui::CollapsingHeader::new(t("Cast labels","Nomes do elenco")).id_salt("plot_cast").show(ui,|ui|{
                let mut remove=None;
                for (i,c) in p.cast.iter_mut().enumerate(){
                    ui.push_id(i,|ui|{
                        ui.horizontal_wrapped(|ui|{
                            ui.add(egui::TextEdit::singleline(&mut c.label).hint_text(t("Cast label","Nome")));
                            ui.add(egui::TextEdit::singleline(&mut c.value).hint_text(t("Role / value","Papel / valor")));
                            if ui.button(t("Remove row","Remover linha")).clicked(){remove=Some(i);}
                        });
                        ui.add(egui::TextEdit::singleline(&mut c.note).hint_text(t("Private cast note","Nota privada")));
                    });
                }
                if let Some(i)=remove{p.cast.remove(i);}
                if p.cast.len()<1000 && ui.button(t("Add cast label","Adicionar nome")).clicked(){p.cast.push(Cast::default());}
            });
            egui::CollapsingHeader::new(t("Development timeline","Cronologia de acontecimentos")).id_salt("plot_timeline").default_open(true).show(ui,|ui|{
                let mut remove=None;
                for (i,e) in p.developments.iter_mut().enumerate(){
                    ui.push_id(i,|ui|{
                        ui.horizontal(|ui|{
                            ui.add(egui::TextEdit::singleline(&mut e.day).hint_text("YYYY-MM-DD"));
                            if ui.button(t("Remove event","Remover acontecimento")).clicked(){remove=Some(i);}
                        });
                        ui.add(egui::TextEdit::multiline(&mut e.event).desired_rows(2).desired_width(f32::INFINITY).hint_text(t("What happened?","O que aconteceu?")));
                    });
                }
                if let Some(i)=remove{p.developments.remove(i);}
                if p.developments.len()<1000 && ui.button(t("Add event","Adicionar acontecimento")).clicked(){
                    p.developments.push(Development::default());ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                }
            });
        });
        self.plot.open = open;
        if let Some(id) = switch {
            self.request_action(PendingAction::PlotRecord(id));
        }
        if save {
            self.save_plot_draft();
        }
        if export {
            match self.plot_html() {
                Ok(html) => {
                    if let Some(path) = FileDialog::new()
                        .set_file_name("Inconnu-private-plot.html")
                        .add_filter("HTML", &["html"])
                        .save_file()
                    {
                        self.plot.message=Some(match grapevine_workbench::safe_files::write_new(&path,html.as_bytes()){
                            Ok(())=>t("Private report saved. Open it in a browser to print or save as PDF.","Relatório privado salvo. Abra no navegador para imprimir ou salvar como PDF.").into(),
                            Err(e)=>e.message,
                        });
                    }
                }
                Err(e) => self.plot.message = Some(e.message),
            }
        }
    }
}
