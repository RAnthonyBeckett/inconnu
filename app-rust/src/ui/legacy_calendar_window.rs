use super::*;
use grapevine_workbench::legacy_calendar as calendar;
#[derive(Default)]
pub(super) struct CalendarWindow {
    pub open: bool,
    pub sources: Vec<calendar::Source>,
    pub context: Option<calendar::Context>,
    pub request: calendar::Request,
    pub quote: Option<calendar::Quote>,
    pub rows: Vec<(bool, calendar::Choice)>,
    pub dirty: bool,
    pub message: Option<String>,
}
fn t(pt: bool, en: &str, br: &str) -> String {
    if pt { br.into() } else { en.into() }
}
impl InconnuApp {
    pub(super) fn open_legacy_calendar(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if self.chronicle.dirty() {
            self.chronicle.message = Some(
                self.t(
                    "Save or discard game-information edits first.",
                    "Salve ou descarte as edições das informações do jogo primeiro.",
                )
                .into(),
            );
            return;
        }
        self.chronicle.open = false;
        self.legacy_calendar.open = true;
        if self.legacy_calendar.context.is_some() {
            return;
        }
        let Some(lib) = &self.library else {
            return;
        };
        match lib.legacy_calendar_sources(Role::Storyteller) {
            Ok(sources) => self.legacy_calendar.sources = sources,
            Err(e) => {
                self.legacy_calendar.message = Some(e.message);
                return;
            }
        }
        if let Some(source) = self.legacy_calendar.sources.first() {
            self.load_legacy_calendar(source.hash.clone());
        }
    }
    fn load_legacy_calendar(&mut self, hash: String) {
        self.legacy_calendar.request = calendar::Request {
            source_hash: hash.clone(),
            ..Default::default()
        };
        self.legacy_calendar.quote = None;
        self.legacy_calendar.dirty = false;
        self.legacy_calendar.rows.clear();
        let Some(lib) = &self.library else {
            return;
        };
        match lib.legacy_calendar_context(&hash, Role::Storyteller) {
            Ok(c) => {
                self.legacy_calendar.rows = c
                    .entries
                    .iter()
                    .map(|e| {
                        (
                            false,
                            calendar::Choice {
                                entry_index: e.index,
                                day: e.proposed_day.clone().unwrap_or_default(),
                                title: format!(
                                    "{} {}",
                                    self.t("Legacy game date", "Data de jogo legada"),
                                    e.index + 1
                                ),
                            },
                        )
                    })
                    .collect();
                self.legacy_calendar.context = Some(c);
                self.legacy_calendar.message = None;
            }
            Err(e) => {
                self.legacy_calendar.context = None;
                self.legacy_calendar.message = Some(e.message);
            }
        }
    }
    pub(super) fn legacy_calendar_action(&mut self, action: u8) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if action == 0 {
            let hash = self.legacy_calendar.request.source_hash.clone();
            self.load_legacy_calendar(hash);
            return;
        }
        let own = std::mem::take(&mut self.legacy_calendar.dirty);
        let blocked = self.has_unsaved_edit();
        self.legacy_calendar.dirty = own;
        if blocked {
            self.legacy_calendar.message = Some(
                self.t(
                    "Save or discard other pending edits first.",
                    "Salve ou descarte outras edições pendentes primeiro.",
                )
                .into(),
            );
            return;
        }
        let (Some(lib), Some(c)) = (&mut self.library, &self.legacy_calendar.context) else {
            return;
        };
        if action == 1 {
            self.legacy_calendar.quote = None;
            match lib.quote_legacy_calendar(
                &self.legacy_calendar.request,
                c.revision,
                Role::Storyteller,
            ) {
                Ok(q) => {
                    self.legacy_calendar.quote = Some(q);
                    self.legacy_calendar.message = None;
                }
                Err(e) => self.legacy_calendar.message = Some(e.message),
            }
        } else if action == 2
            && let Some(q) = self.legacy_calendar.quote.clone()
        {
            if q.request != self.legacy_calendar.request {
                return;
            }
            match lib.commit_legacy_calendar(&q, Role::Storyteller) {
                Ok(_) => {
                    self.legacy_calendar.dirty = false;
                    self.revision = lib.revision().ok();
                    self.chronicle = Default::default();
                    self.refresh_summaries(true);
                    self.load_legacy_calendar(q.request.source_hash);
                    self.legacy_calendar.message=Some(self.t("Calendar plans saved. No attendance or XP was recorded. Open Game sessions to review a plan later.","Planos do calendário salvos. Nenhuma presença ou XP foi registrada. Abra Sessões de jogo para revisar um plano depois.").into());
                }
                Err(e) => self.legacy_calendar.message = Some(e.message),
            }
        }
    }
    pub(super) fn render_legacy_calendar(&mut self, ctx: &egui::Context) {
        if self.route != WorkArea::Storyteller {
            self.legacy_calendar = Default::default();
            return;
        }
        if !self.legacy_calendar.open {
            return;
        }
        let pt = self.language == Language::Portuguese;
        let revision = self.library.as_ref().and_then(|l| l.revision().ok());
        let mut action = None;
        let mut source = None;
        let mut open = true;
        let state = &mut self.legacy_calendar;
        egui::Window::new(t(pt,"Review legacy calendar dates","Revisar datas do calendário legado")).id(egui::Id::unique("legacy_calendar_window")).open(&mut open).default_width(780.0).default_height(620.0).show(ctx,|ui|{
   ui.horizontal_wrapped(|ui|{
    let b=ui.button(t(pt,"Preview calendar plans","Visualizar planos do calendário"));mark_calendar(ui,"legacy_calendar_prepare",b.interact_rect);if b.clicked(){action=Some(1);}
    let enabled=state.quote.as_ref().is_some_and(|q|Some(q.revision)==revision&&q.request==state.request);
    let b=ui.add_enabled(enabled,egui::Button::new(t(pt,"Save reviewed plans","Salvar planos revisados")));mark_calendar(ui,"legacy_calendar_apply",b.interact_rect);if b.clicked(){action=Some(2);}
    if ui.button(t(pt,"Reload / discard draft","Recarregar / descartar rascunho")).clicked(){action=Some(0);}
   });
   if let Some(message)=&state.message{ui.label(message);}
   egui::ScrollArea::vertical().show(ui,|ui|{
    ui.label(t(pt,"Date-only plans. No attendance, XP, completed session or official approval is inferred. Time/place/notes remain private source evidence. Changing source clears this draft.","Planos apenas com data. Nenhuma presença, XP, sessão concluída ou aprovação oficial é presumida. Horário/local/notas permanecem como evidência privada da fonte. Mudar a fonte limpa este rascunho."));
    egui::ComboBox::from_id_salt("legacy_calendar_source").selected_text(state.context.as_ref().map(|c|c.source_name.as_str()).unwrap_or("—")).show_ui(ui,|ui|{for s in &state.sources{if ui.selectable_label(state.request.source_hash==s.hash,format!("{} · {}",s.name,s.entries)).clicked(){source=Some(s.hash.clone());}
if let Some(issue)=&s.issue{ui.weak(issue);}}});
    if state.sources.is_empty(){ui.label(t(pt,"No retained calendar sources. Import a legacy game first.","Nenhuma fonte de calendário retida. Importe um jogo legado primeiro."));}
    let Some(c)=&state.context else{return;};
    let before=state.request.clone();let mut changed=false;
    let imported_indices=c.history.iter().filter(|h|h.request.source_hash==c.source_hash).flat_map(|h|h.plans.iter().map(|p|p.entry_index)).collect::<std::collections::HashSet<_>>();
    for (entry,(selected,choice)) in c.entries.iter().zip(&mut state.rows){
     let imported=imported_indices.contains(&entry.index);
     ui.push_id(entry.index,|ui|{
      ui.separator();ui.horizontal_wrapped(|ui|{
       changed|=ui.add_enabled(!imported,egui::Checkbox::new(selected,format!("{} {}",t(pt,"Entry","Entrada"),entry.index+1))).changed();
       ui.label(format!("{}: {}",t(pt,"Original date","Data original"),entry.raw_date));
       if imported{ui.weak(t(pt,"Already imported; not recreated","Já importada; não será recriada"));}
      });
      if let Some(issue)=&entry.issue{ui.weak(issue);}
      if *selected&&!imported{
       ui.horizontal_wrapped(|ui|{ui.label(t(pt,"Reviewed ISO day (YYYY-MM-DD)","Data ISO revisada (AAAA-MM-DD)"));changed|=ui.text_edit_singleline(&mut choice.day).changed();});
       ui.label(t(pt,"Neutral plan title","Título neutro do plano"));changed|=ui.text_edit_singleline(&mut choice.title).changed();
      }
      ui.collapsing(t(pt,"Private source time / place / notes","Horário / local / notas privados da fonte"),|ui|{ui.label(&entry.time);ui.label(&entry.place);ui.label(&entry.notes);});
     });
    }
    if changed{state.request.choices=state.rows.iter().filter(|(selected,_)|*selected).map(|(_,choice)|choice.clone()).collect();}
    ui.separator();
    for (en,br,value) in [("Review date (YYYY-MM-DD)","Data da revisão (AAAA-MM-DD)",&mut state.request.review_day),("Staff name","Nome da pessoa da equipe",&mut state.request.actor),("Reason","Motivo",&mut state.request.reason),("Review reference","Referência da revisão",&mut state.request.review_reference)]{ui.label(t(pt,en,br));ui.text_edit_singleline(value);}
    ui.checkbox(&mut state.request.confirm_past,t(pt,"I reviewed any past dates. Plans do not establish attendance or completion.","Revisei quaisquer datas passadas. Planos não comprovam presença ou conclusão."));
    ui.checkbox(&mut state.request.confirm_overlap,t(pt,"I reviewed same-day overlaps. Separate entries remain separate plans.","Revisei sobreposições na mesma data. Entradas separadas permanecem planos separados."));
    ui.checkbox(&mut state.request.confirmed,t(pt,"I reviewed these exact source entries, ISO dates and titles. No attendance or XP is being recorded.","Revisei estas entradas exatas da fonte, datas ISO e títulos. Nenhuma presença ou XP está sendo registrada."));
    if changed||state.request!=before{state.quote=None;state.dirty=true;}
    if let Some(q)=&state.quote{ui.separator();ui.heading(t(pt,"Plans to save together","Planos a salvar juntos"));for p in &q.plans{ui.label(format!("{} · {}",p.request.day,p.request.title));}}
    ui.separator();ui.heading(t(pt,"Private import history","Histórico privado de importação"));for receipt in c.history.iter().rev(){ui.collapsing(format!("{} · {}",receipt.request.review_day,receipt.source_name),|ui|{ui.label(&receipt.request.actor);ui.label(&receipt.request.reason);ui.label(&receipt.request.review_reference);for plan in &receipt.plans{ui.label(format!("{} · {}",plan.request.day,plan.request.title));}});}
   });
  });
        self.legacy_calendar.open = open;
        if let Some(source) = source {
            self.load_legacy_calendar(source);
        }
        if let Some(action) = action {
            self.legacy_calendar_action(action);
        }
    }
}
fn mark_calendar(ui: &egui::Ui, key: &str, rect: egui::Rect) {
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), Some(rect)));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bilingual_calendar_review_guards_apply_load_and_clear_player_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("calendar.gvlib");
        smoke_legacy_calendar_fixture(&path);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_legacy_calendar();
        let hash = app
            .legacy_calendar
            .context
            .as_ref()
            .unwrap()
            .source_hash
            .clone();
        app.legacy_calendar.request = smoke_legacy_calendar_request(hash);
        for (selected, choice) in &mut app.legacy_calendar.rows {
            *selected = true;
            *choice = app.legacy_calendar.request.choices[choice.entry_index].clone();
        }
        app.legacy_calendar.dirty = true;
        app.legacy_calendar_action(1);
        let quote = app.legacy_calendar.quote.clone().unwrap();
        let ctx = egui::Context::default();
        for language in [Language::English, Language::Portuguese] {
            app.language = language;
            for _ in 0..2 {
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    app.render_legacy_calendar(ui.ctx())
                });
                out.textures_delta.clear();
            }
            assert_eq!(app.legacy_calendar.quote.as_ref(), Some(&quote));
        }
        app.edit.name = "Pending edit".into();
        app.legacy_calendar_action(2);
        assert!(
            app.library
                .as_ref()
                .unwrap()
                .scheduled_sessions(Role::Storyteller)
                .unwrap()
                .is_empty()
        );
        app.edit = app.saved_edit.clone();
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        app.legacy_calendar_action(2);
        assert_eq!(
            app.legacy_calendar.context.as_ref().unwrap().history.len(),
            1
        );
        assert!(!app.legacy_calendar.dirty);
        let plans = app
            .library
            .as_ref()
            .unwrap()
            .scheduled_sessions(Role::Storyteller)
            .unwrap();
        assert_eq!(plans.len(), 2);
        app.load_scheduled_session(plans[0].request.clone());
        assert!(app.session_form.request.attendance.is_empty());
        assert!(app.session_form.legacy_origin.is_some());
        assert!(
            app.session_form
                .rows
                .values()
                .all(|r| !r.included && !r.present && !r.award_enabled)
        );
        app.session_open = false;
        app.change_route(WorkArea::Player);
        assert!(app.legacy_calendar.context.is_none());
        assert!(app.legacy_calendar.request.actor.is_empty());
    }
}
