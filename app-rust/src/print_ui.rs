use eframe::egui;
use grapevine_workbench::{
    model::Language,
    printing::{DocLanguage, Options, PrintScope, SheetDocument},
};

#[derive(Default)]
pub struct Form {
    pub options: Options,
    pub document: Option<SheetDocument>,
    pub revision: Option<i64>,
    pub message: Option<String>,
}
pub enum Action {
    Preview,
    Export,
}
fn t<'a>(pt: bool, en: &'a str, br: &'a str) -> &'a str {
    if pt { br } else { en }
}

pub fn render(
    ui: &mut egui::Ui,
    form: &mut Form,
    revision: i64,
    language: Language,
    can_private: bool,
) -> Option<Action> {
    let pt = language == Language::Portuguese;
    ui.label(t(pt,"Preview the saved character sheet. Unsaved edits are excluded. Export a standalone HTML file; open it in your browser to print or save as PDF.","Pré-visualize a ficha salva. Edições não salvas ficam de fora. Exporte HTML independente; abra no navegador para imprimir ou salvar em PDF."));
    let before = form.options.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label(t(pt, "Document language", "Idioma do documento"));
        ui.selectable_value(&mut form.options.language, DocLanguage::English, "English");
        ui.selectable_value(
            &mut form.options.language,
            DocLanguage::Portuguese,
            "Português",
        );
    });
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(
            &mut form.options.scope,
            PrintScope::Public,
            t(
                pt,
                "Player-safe projection",
                "Somente dados visíveis ao jogador",
            ),
        );
        if can_private {
            ui.selectable_value(
                &mut form.options.scope,
                PrintScope::PrivateStoryteller,
                t(pt, "Private Storyteller copy", "Cópia privada do Narrador"),
            );
        }
    });
    if form.options.scope == PrintScope::Public {
        form.options.include_notes = false;
    }
    ui.checkbox(
        &mut form.options.large_text,
        t(pt, "Large text", "Texto ampliado"),
    );
    ui.checkbox(
        &mut form.options.include_temporary,
        t(
            pt,
            "Include temporary resource values",
            "Incluir valores temporários de recursos",
        ),
    );
    ui.checkbox(
        &mut form.options.include_references,
        t(
            pt,
            "Include sources for mapped owned powers",
            "Incluir fontes dos poderes conhecidos e vinculados",
        ),
    );
    let mechanics_toggle = ui.checkbox(
        &mut form.options.include_simple_mechanics,
        t(
            pt,
            "Include reviewed simple mechanics",
            "Incluir regras resumidas revisadas",
        ),
    );
    let rect = ui
        .clip_rect()
        .contains_rect(mechanics_toggle.rect)
        .then_some(mechanics_toggle.rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::unique("print_mechanics_toggle"), rect));
    if can_private && form.options.scope == PrintScope::PrivateStoryteller {
        ui.checkbox(
            &mut form.options.include_notes,
            t(
                pt,
                "Include private ST and trait notes in this private copy",
                "Incluir notas privadas do Narrador e das características nesta cópia privada",
            ),
        );
        ui.colored_label(egui::Color32::YELLOW,t(pt,"Private copies can contain secrets. Review the preview before sharing or printing.","Cópias privadas podem conter segredos. Revise a prévia antes de compartilhar ou imprimir."));
    }
    if before != form.options {
        form.document = None;
        form.revision = None;
        form.message = None;
    }
    let mut action = None;
    let preview = ui.button(t(pt, "Preview sheet", "Pré-visualizar ficha"));
    let rect = ui.is_rect_visible(preview.rect).then_some(preview.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique("print_preview"), rect));
    if preview.clicked() {
        action = Some(Action::Preview);
    }
    let current = form.revision == Some(revision)
        && form
            .document
            .as_ref()
            .is_some_and(|d| d.options == form.options);
    if ui
        .add_enabled(
            current,
            egui::Button::new(t(
                pt,
                "Export printable HTML…",
                "Exportar HTML para impressão…",
            )),
        )
        .clicked()
    {
        action = Some(Action::Export);
    }
    if let Some(message) = &form.message {
        ui.label(message);
    }
    if let Some(doc) = &form.document {
        if !current {
            ui.colored_label(
                egui::Color32::YELLOW,
                t(
                    pt,
                    "Library changed; preview again before exporting.",
                    "Biblioteca alterada; atualize a prévia antes de exportar.",
                ),
            );
        }
        egui::ScrollArea::vertical()
            .id_salt("sheet_print_preview")
            .max_height(420.0)
            .show(ui, |ui| {
                ui.scope(|ui| {
                    if doc.options.large_text {
                        ui.style_mut().override_font_id = Some(egui::FontId::proportional(20.0));
                    }
                    ui.heading(&doc.title);
                    ui.label(format!(
                        "{} · {} · {}",
                        doc.kind, doc.status, doc.player_name
                    ));
                    if doc.scope == PrintScope::PrivateStoryteller {
                        ui.strong(t(
                            doc.options.language == DocLanguage::Portuguese,
                            "PRIVATE STORYTELLER COPY",
                            "CÓPIA PRIVADA DO NARRADOR",
                        ));
                    }
                    if doc.build.is_some() {
                        ui.strong(t(
                            doc.options.language == DocLanguage::Portuguese,
                            "BUILD COPY — NOT APPROVED",
                            "CÓPIA DE CONSTRUÇÃO — NÃO APROVADA",
                        ));
                    }
                    for section in &doc.sections {
                        ui.separator();
                        ui.strong(&section.title);
                        for row in &section.rows {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(&row.name);
                                ui.label(&row.value);
                            });
                            if let Some(note) = &row.note {
                                ui.small(note);
                            }
                        }
                    }
                    for reference in &doc.references {
                        ui.separator();
                        ui.label(format!(
                            "{} · {} · {} ({}) · p.{} / PDF{}",
                            reference.family,
                            reference.power,
                            reference.book,
                            reference.edition,
                            reference.printed_page,
                            reference.pdf_page
                        ));
                        ui.small(&reference.library_prompt);
                    }
                    for warning in &doc.warnings {
                        ui.small(warning);
                    }
                });
            });
    }
    action
}
