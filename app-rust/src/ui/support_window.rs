use super::*;
use grapevine_workbench::{
    model::{Result, error},
    support_export::{self, Snapshot},
};

#[derive(Default)]
pub(super) struct SupportWindow {
    pub open: bool,
    pub include_backtraces: bool,
    pub snapshot: Option<Snapshot>,
    source_override: Option<PathBuf>,
    selected_event: Option<usize>,
    pub message: Option<String>,
}
impl SupportWindow {
    pub(super) fn synthetic(source: PathBuf) -> Self {
        Self {
            open: true,
            source_override: Some(source),
            ..Default::default()
        }
    }
    pub(super) fn ready(&self) -> bool {
        self.snapshot
            .as_ref()
            .is_some_and(|s| s.include_backtraces == self.include_backtraces)
    }
    pub(super) fn save_new(&self, path: &std::path::Path, role: Role) -> Result<()> {
        if role != Role::Storyteller {
            return Err(error("ACCESS_DENIED", "Storyteller view required."));
        }
        if !self.ready() {
            return Err(error(
                "SUPPORT_PREPARE_REQUIRED",
                "Prepare the support preview with the selected options first.",
            ));
        }
        let document = support_export::json(self.snapshot.as_ref().expect("checked preview"))?;
        grapevine_workbench::safe_files::write_new(path, document.as_bytes())
    }
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
fn probe(ui: &egui::Ui, response: &egui::Response, key: &'static str) {
    let rect = ui
        .clip_rect()
        .contains_rect(response.rect)
        .then_some(response.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), rect));
}
impl InconnuApp {
    pub(super) fn prepare_support(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let source = self
            .support
            .source_override
            .clone()
            .unwrap_or_else(diagnostics::directory);
        match support_export::prepare(&source, self.support.include_backtraces, Role::Storyteller) {
            Ok(snapshot) => {
                self.support.selected_event = None;
                self.support.snapshot = Some(snapshot);
                self.support.message = None;
            }
            Err(e) => {
                self.support.snapshot = None;
                self.support.message = Some(format!("{}: {}", e.code, e.message));
            }
        }
    }
    pub(super) fn render_support(&mut self, ctx: &egui::Context) {
        if !self.support.open || self.route != WorkArea::Storyteller {
            return;
        }
        let l = self.language;
        let mut open = true;
        let (mut prepare, mut save) = (false, false);
        egui::Window::new(t(l,"Diagnostic support file","Arquivo de diagnóstico para suporte"))
            .id(egui::Id::unique("support_window"))
            .default_size([740.,560.])
            .vscroll(true)
            .open(&mut open)
            .show(ctx,|ui| {
                ui.label(t(l,
                    "Preview captured diagnostic codes and choose what to save locally. Nothing is sent automatically.",
                    "Revise os códigos de diagnóstico capturados e escolha o que salvar localmente. Nada é enviado automaticamente."));
                let option=ui.checkbox(&mut self.support.include_backtraces,t(l,
                    "Include stack traces (may contain local file paths)",
                    "Incluir rastros de execução (podem conter caminhos de arquivos locais)"));
                probe(ui,&option,"support_traces");
                if option.changed() {
                    self.support.snapshot=None;
                    self.support.selected_event=None;
                    self.support.message=None;
                }
                ui.horizontal_wrapped(|ui| {
                    let response=ui.button(t(l,"Prepare preview","Preparar prévia"));
                    probe(ui,&response,"support_prepare");
                    prepare=response.clicked();
                    save=ui.add_enabled(self.support.ready(),egui::Button::new(t(l,"Save support file…","Salvar arquivo de suporte…"))).clicked();
                });
                if let Some(snapshot)=&self.support.snapshot {
                    ui.label(format!("{} {} · {} {} · {} {}",
                        t(l,"Retained events","Eventos preservados"),snapshot.events.len(),
                        t(l,"Malformed lines ignored","Linhas inválidas ignoradas"),snapshot.malformed_lines,
                        t(l,"Older captured events omitted","Eventos capturados antigos omitidos"),snapshot.omitted_events));
                    if snapshot.source_missing {ui.label(t(l,"No saved diagnostic log was found.","Nenhum registro de diagnóstico salvo foi encontrado."));}
                    if snapshot.source_tail_truncated {ui.label(t(l,"Only the newest bounded portion of the log was captured.","Somente a parte recente limitada do registro foi capturada."));}
                    ui.label(t(l,"This is a captured copy. Prepare again to include newer events.","Esta é uma cópia capturada. Prepare novamente para incluir eventos mais recentes."));
                    let row_height=ui.text_style_height(&egui::TextStyle::Body)+6.;
                    egui::ScrollArea::vertical().id_salt("support_events").max_height(200.)
                        .show_rows(ui,row_height,snapshot.events.len(),|ui,range| {
                            for index in range {
                                let event=&snapshot.events[index];
                                let text=format!("{} · {} · {}",event.build,event.operation,event.code);
                                let response=ui.add(egui::Button::selectable(self.support.selected_event==Some(index),&text).truncate());
                                if response.on_hover_text(&text).clicked() {self.support.selected_event=Some(index);}
                            }
                        });
                    if let Some(event)=self.support.selected_event.and_then(|i|snapshot.events.get(i)) {
                        if let Some(trace)=&event.backtrace {
                            if event.trace_truncated {ui.label(t(l,"This stack trace was shortened to the export limit.","Este rastro foi reduzido ao limite de exportação."));}
                            let mut text=trace.as_str();
                            ui.add(egui::TextEdit::multiline(&mut text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).desired_rows(8).interactive(false));
                        } else {ui.label(t(l,"No stack trace is included for this event.","Nenhum rastro de execução está incluído neste evento."));}
                    }
                }
                if let Some(message)=&self.support.message {ui.label(message);}
            });
        self.support.open = open;
        if prepare {
            self.prepare_support();
        }
        if save
            && self.support.ready()
            && let Some(path) = FileDialog::new()
                .set_file_name("grapevine-support.json")
                .save_file()
        {
            match self.support.save_new(&path, Role::Storyteller) {
                Ok(()) => {
                    self.support.message = Some(
                        t(
                            l,
                            "Support file saved locally.",
                            "Arquivo de suporte salvo localmente.",
                        )
                        .into(),
                    )
                }
                Err(e) => self.support.message = Some(format!("{}: {}", e.code, e.message)),
            }
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn support_capture_stays_immutable_and_role_changes_clear_private_state() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("events.jsonl");
        std::fs::write(&source,b"{\"unix_seconds\":1,\"build\":\"0.29.0\",\"operation\":\"test\",\"code\":\"EXPECTED\",\"backtrace\":\"PRIVATE_TRACE\",\"character\":\"PRIVATE_CHARACTER\"}\n").unwrap();
        let mut app = InconnuApp::new(None);
        app.change_route(WorkArea::Storyteller);
        app.support = SupportWindow::synthetic(dir.path().into());
        app.prepare_support();
        let original = support_export::json(app.support.snapshot.as_ref().unwrap()).unwrap();
        assert!(!original.contains("PRIVATE_TRACE") && !original.contains("PRIVATE_CHARACTER"));
        app.support.open = false;
        app.language = Language::Portuguese;
        app.support.open = true;
        let mut log = std::fs::OpenOptions::new()
            .append(true)
            .open(&source)
            .unwrap();
        writeln!(log,"{{\"unix_seconds\":2,\"build\":\"0.29.0\",\"operation\":\"test\",\"code\":\"NEW_EVENT\"}}").unwrap();
        let output = dir.path().join("support.json");
        app.support.save_new(&output, Role::Storyteller).unwrap();
        assert_eq!(std::fs::read_to_string(&output).unwrap(), original);
        assert!(app.support.save_new(&output, Role::Storyteller).is_err());
        assert!(
            app.support
                .save_new(&dir.path().join("denied.json"), Role::Player)
                .is_err()
        );
        app.support.include_backtraces = true;
        assert!(!app.support.ready());
        app.prepare_support();
        assert_eq!(app.support.snapshot.as_ref().unwrap().events.len(), 2);
        assert!(
            support_export::json(app.support.snapshot.as_ref().unwrap())
                .unwrap()
                .contains("PRIVATE_TRACE")
        );
        app.change_route(WorkArea::Player);
        assert!(app.support.snapshot.is_none());
        assert!(!app.support.include_backtraces);
        assert!(!app.support.open);
    }
}
