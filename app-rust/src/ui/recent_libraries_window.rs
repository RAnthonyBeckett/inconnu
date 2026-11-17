use super::*;
use grapevine_workbench::recent_libraries::RecentLibraries;
impl InconnuApp {
    pub fn new_with_recent_history(initial: Option<PathBuf>, file: Option<PathBuf>) -> Self {
        let mut app = Self::new(None);
        if let Some(file) = file {
            app.recent_libraries = RecentLibraries::load(file);
        }
        if let Some(path) = initial {
            app.open_path(path);
        }
        app
    }
    pub(super) fn render_recent_libraries(&mut self, ui: &mut egui::Ui) {
        let mut open = None;
        let mut forget = None;
        let mut clear = false;
        let paths = self.recent_libraries.paths().to_vec();
        let menu=ui.menu_button(self.t("Recent libraries…","Bibliotecas recentes…"),|ui| {
            ui.set_max_width(650.0);
            ui.label(self.t("On this Windows account. Opening uses the normal unsaved-edit check.","Nesta conta do Windows. A abertura usa a verificação normal de edições não salvas."));
            if paths.is_empty(){ui.label(self.t("No recent libraries","Nenhuma biblioteca recente"));}
            for (index,path) in paths.iter().enumerate() {
                ui.horizontal(|ui| {
                    let button=ui.add_sized([ui.available_width().min(530.0),24.0],egui::Button::new(format!("{} — {}",path.file_name().unwrap_or_default().to_string_lossy(),path.parent().unwrap_or(path).display())).truncate()).on_hover_text(path.display().to_string());
                    if index==0 { ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique("recent_first"),button.interact_rect.is_positive().then_some(button.interact_rect))); }
                    if button.clicked(){open=Some(path.clone());ui.close();}
                    if ui.button(self.t("Forget","Esquecer")).clicked(){forget=Some(path.clone());ui.close();}
                });
            }
            ui.separator();
            if self.recent_libraries.warning().is_some(){ui.label(self.t("Recent history could not be read or saved. Normal library use is still available. An unreadable existing history file is kept until Clear list.","Não foi possível ler ou salvar o histórico recente. O uso normal da biblioteca continua disponível. Um arquivo de histórico ilegível é mantido até Limpar lista."));}
            if ui.button(self.t("Clear list (keeps library files)","Limpar lista (mantém os arquivos)")).clicked(){clear=true;ui.close();}
        });
        ui.ctx().data_mut(|d| {
            d.insert_temp(
                egui::Id::unique("recent_menu"),
                menu.response
                    .interact_rect
                    .is_positive()
                    .then_some(menu.response.interact_rect),
            )
        });
        if let Some(path) = forget {
            self.recent_libraries.forget(&path);
        }
        if clear {
            self.recent_libraries.clear();
        }
        if let Some(path) = open {
            self.request_action(PendingAction::OpenRecent(path));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guarded_recent_open_failure_restart_and_forget_preserve_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let history = dir.path().join("state/recent.json");
        let first = dir.path().join("first.gvlib");
        let second = dir.path().join("second.gvlib");
        let mut lib = Library::create(&first, "First").unwrap();
        lib.create_character("Vampire", "Unsaved character", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        drop(Library::create(&second, "Second").unwrap());
        let mut app =
            InconnuApp::new_with_recent_history(Some(first.clone()), Some(history.clone()));
        app.change_route(WorkArea::Storyteller);
        app.edit.name = "Pending name".into();
        app.request_action(PendingAction::OpenRecent(second.clone()));
        assert!(matches!(
            app.pending_action,
            Some(PendingAction::OpenRecent(_))
        ));
        assert_eq!(app.library.as_ref().unwrap().path, first);
        assert_eq!(app.edit.name, "Pending name");
        app.pending_action = None;
        app.edit = app.saved_edit.clone();
        let missing = dir.path().join("missing.gvlib");
        app.request_action(PendingAction::OpenRecent(missing.clone()));
        assert!(app.error.is_some());
        assert!(!missing.exists());
        assert_eq!(app.library.as_ref().unwrap().path, first);
        app.request_action(PendingAction::OpenRecent(second.clone()));
        assert_eq!(app.library.as_ref().unwrap().path, second);
        let mut reopened = InconnuApp::new_with_recent_history(None, Some(history));
        assert!(reopened.library.is_none());
        assert_eq!(
            reopened.recent_libraries.paths(),
            [second.clone(), first.clone()]
        );
        for language in [Language::English, Language::Portuguese] {
            reopened.language = language;
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                reopened.render_recent_libraries(ui)
            });
            output.textures_delta.clear();
            assert_eq!(reopened.recent_libraries.paths()[0], second);
        }
        let first_bytes = std::fs::read(&first).unwrap();
        let second_bytes = std::fs::read(&second).unwrap();
        reopened.recent_libraries.forget(&first);
        reopened.recent_libraries.clear();
        assert_eq!(std::fs::read(first).unwrap(), first_bytes);
        assert_eq!(std::fs::read(second).unwrap(), second_bytes);
    }
}
