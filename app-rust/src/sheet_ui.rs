use eframe::egui;
use grapevine_workbench::{
    model::{Language, PlayerLink, SheetEdit, Summary},
    sheet::{IDENTITY_FIELDS, LIST_FIELDS, RESOURCE_FIELDS, editable_list, empty_list},
};
use serde_json::{Value, json};

const MAX_ROWS: usize = 4096;
const MAX_TEXT_BYTES: usize = 16_000;

/// Creation shares the sheet editor; permanent resources come from the reviewed profile.
pub fn render_creation(
    ui: &mut egui::Ui,
    edit: &mut SheetEdit,
    language: Language,
    players: &[Summary],
) {
    render_identity(ui, edit, language);
    ui.label(label(language, "MET lists use adjective counts; list each Basic power with value 'basic'. Trait notes remain private.", "As listas MET usam contagens de adjetivos; liste cada poder Básico com valor 'basic'. As notas permanecem privadas."));
    render_add_list(ui, edit, language);
    for key in edit.lists.keys().cloned().collect::<Vec<_>>() {
        if let Some(list) = edit.lists.get_mut(&key) {
            egui::CollapsingHeader::new(list_title(&key, language))
                .id_salt(("creation_trait_list", key.as_str()))
                .show(ui, |ui| {
                    visibility(ui, &mut edit.visibility, &key, language);
                    render_list(ui, &key, list, language);
                });
        }
    }
    render_player_link(ui, edit, language, players);
}

fn label<'a>(language: Language, english: &'a str, portuguese: &'a str) -> &'a str {
    match language {
        Language::English => english,
        Language::Portuguese => portuguese,
    }
}

pub fn render(ui: &mut egui::Ui, edit: &mut SheetEdit, language: Language, players: &[Summary]) {
    render_identity(ui, edit, language);
    ui.heading(label(
        language,
        "Traits and resources",
        "Características e recursos",
    ));
    ui.label(label(
        language,
        "Edit descriptive traits and legacy values in this draft.",
        "Edite características descritivas e valores legados neste rascunho.",
    ));

    ui.separator();
    ui.heading(label(language, "Trait lists", "Listas de características"));
    render_add_list(ui, edit, language);

    let list_keys = edit.lists.keys().cloned().collect::<Vec<_>>();
    if list_keys.is_empty() {
        ui.label(label(
            language,
            "No trait lists are present.",
            "Nenhuma lista de características presente.",
        ));
    }
    for key in list_keys {
        if let Some(list) = edit.lists.get_mut(&key) {
            egui::CollapsingHeader::new(list_title(&key, language))
                .id_salt(("sheet_trait_list", key.as_str()))
                .default_open(key == "DisciplineList")
                .show(ui, |ui| {
                    visibility(ui, &mut edit.visibility, &key, language);
                    render_list(ui, &key, list, language);
                });
        }
    }

    ui.separator();
    ui.heading(label(
        language,
        "Permanent and temporary values",
        "Valores permanentes e temporários",
    ));
    render_add_resource(ui, edit, language);
    let resource_keys = edit.resources.keys().cloned().collect::<Vec<_>>();
    if resource_keys.is_empty() {
        ui.label(label(
            language,
            "No resource values are present.",
            "Nenhum valor de recurso presente.",
        ));
    }
    for key in resource_keys {
        if let Some(value) = edit.resources.get_mut(&key) {
            ui.push_id(("sheet_resource", key.as_str()), |ui| {
                ui.horizontal(|ui| {
                    ui.label(resource_title(&key, language));
                    ui.add(egui::DragValue::new(value).range(i16::MIN as i64..=i16::MAX as i64));
                });
                visibility(ui, &mut edit.visibility, &key, language);
            });
        }
    }

    ui.separator();
    ui.heading(label(language, "Player link", "Vínculo de jogador"));
    render_player_link(ui, edit, language, players);
}

fn visibility(
    ui: &mut egui::Ui,
    choices: &mut std::collections::BTreeMap<String, bool>,
    key: &str,
    language: Language,
) {
    ui.checkbox(
        choices.entry(key.to_owned()).or_insert(false),
        label(
            language,
            "Show on player sheet",
            "Mostrar na ficha do jogador",
        ),
    );
}

fn identity_title(key: &str, language: Language) -> &str {
    match key {
        "Clan" => label(language, "Clan", "Clã"),
        "Sect" => label(language, "Sect", "Seita"),
        "Nature" => label(language, "Nature", "Natureza"),
        "Demeanor" => label(language, "Demeanor", "Comportamento"),
        "Tribe" => label(language, "Tribe", "Tribo"),
        "Kith" => label(language, "Kith", "Kith"),
        "Guild" => label(language, "Guild", "Guilda"),
        "Faction" => label(language, "Faction", "Facção"),
        "Title" => label(language, "Title", "Título"),
        "Path" => label(language, "Humanity / Path", "Humanidade / Trilha"),
        _ => key,
    }
}

fn render_identity(ui: &mut egui::Ui, edit: &mut SheetEdit, language: Language) {
    ui.heading(label(
        language,
        "Identity and player-sheet visibility",
        "Identidade e visibilidade na ficha",
    ));
    ui.weak(label(language,
        "Sharing is explicit. ST notes and trait notes remain private; this does not grant approval or spend XP.",
        "O compartilhamento é explícito. Anotações do Narrador e das características permanecem privadas; isto não concede aprovação nem gasta XP."));
    for (key, value) in &mut edit.identity {
        ui.push_id(("sheet_identity", key.as_str()), |ui| {
            ui.label(identity_title(key, language));
            let response = ui.text_edit_singleline(value);
            if key == "Clan" {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::unique("sheet_clan_bounds"), Some(response.rect))
                });
                let rect = ui
                    .clip_rect()
                    .contains_rect(response.rect)
                    .then_some(response.rect);
                ui.ctx()
                    .data_mut(|data| data.insert_temp(egui::Id::unique("sheet_clan_probe"), rect));
            }
            visibility(ui, &mut edit.visibility, key, language);
        });
    }
    let missing = IDENTITY_FIELDS
        .iter()
        .filter(|key| !edit.identity.contains_key(**key))
        .copied()
        .collect::<Vec<_>>();
    if let Some(first) = missing.first() {
        let selector = ui.make_persistent_id("sheet_add_identity");
        let mut choice = ui
            .data_mut(|data| data.get_temp::<String>(selector))
            .filter(|value| missing.contains(&value.as_str()))
            .unwrap_or_else(|| (*first).to_owned());
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(selector)
                .selected_text(identity_title(&choice, language))
                .show_ui(ui, |ui| {
                    for key in missing {
                        ui.selectable_value(
                            &mut choice,
                            key.to_owned(),
                            identity_title(key, language),
                        );
                    }
                });
            if ui
                .button(label(
                    language,
                    "Add identity field",
                    "Adicionar campo de identidade",
                ))
                .clicked()
            {
                edit.identity.insert(choice.clone(), String::new());
                edit.visibility.insert(choice.clone(), false);
            }
        });
        ui.data_mut(|data| data.insert_temp(selector, choice));
    }
    ui.separator();
}

fn render_add_list(ui: &mut egui::Ui, edit: &mut SheetEdit, language: Language) {
    let missing = LIST_FIELDS
        .iter()
        .filter(|key| !edit.lists.contains_key(**key))
        .map(|key| (*key).to_owned())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        ui.label(label(
            language,
            "All supported lists are present.",
            "Todas as listas disponíveis já estão presentes.",
        ));
        return;
    }

    let selector_id = ui.make_persistent_id("sheet_add_list_choice");
    let mut choice = ui
        .data_mut(|data| data.get_temp::<String>(selector_id))
        .filter(|key| missing.contains(key))
        .unwrap_or_else(|| missing[0].clone());
    let mut add = false;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(selector_id)
            .selected_text(list_title(&choice, language))
            .show_ui(ui, |ui| {
                for key in &missing {
                    ui.selectable_value(&mut choice, key.clone(), list_title(key, language));
                }
            });
        add = ui
            .button(label(language, "Add list", "Adicionar lista"))
            .clicked();
    });
    ui.data_mut(|data| data.insert_temp(selector_id, choice.clone()));
    if add {
        edit.lists.insert(choice.clone(), empty_list(&choice));
    }
}

pub(crate) fn render_list(ui: &mut egui::Ui, key: &str, list: &mut Value, language: Language) {
    if !editable_list(list) {
        ui.label(label(
            language,
            "This list has an unsupported structure and is kept unchanged.",
            "A estrutura desta lista não é compatível; ela será mantida sem alterações.",
        ));
        return;
    }
    let Some(rows) = list.get_mut("Traits").and_then(Value::as_array_mut) else {
        return;
    };

    let visible_width = ui
        .available_width()
        .min(ui.ctx().content_rect().right() - ui.cursor().left() - 16.0);
    let fields_width = (visible_width - 110.0).max(120.0);
    let name_width = fields_width * 0.32;
    let value_width = fields_width * 0.24;
    let note_width = fields_width * 0.44;
    ui.horizontal(|ui| {
        ui.add_sized(
            [name_width, 18.0],
            egui::Label::new(label(language, "Name", "Nome")),
        );
        ui.add_sized(
            [value_width, 18.0],
            egui::Label::new(label(language, "Value", "Valor")),
        );
        ui.add_sized(
            [note_width, 18.0],
            egui::Label::new(label(language, "Private note", "Nota privada")),
        );
        ui.label("");
    });

    let mut remove_at = None;
    for (index, row) in rows.iter_mut().enumerate() {
        ui.push_id(("sheet_trait_row", key, index), |ui| {
            ui.horizontal(|ui| {
                edit_row_text(ui, row, "Name", name_width, false);
                edit_row_text(
                    ui,
                    row,
                    "Value",
                    value_width,
                    key == "DisciplineList" && index == 0,
                );
                edit_row_text(ui, row, "Note", note_width, false);
                if ui.button(label(language, "Remove", "Remover")).clicked() {
                    remove_at = Some(index);
                }
            });
        });
    }
    if let Some(index) = remove_at {
        rows.remove(index);
    }
    if rows.len() >= MAX_ROWS {
        ui.label(label(
            language,
            "This list has reached the 4096-row limit.",
            "Esta lista atingiu o limite de 4096 linhas.",
        ));
    } else if ui
        .button(label(language, "Add trait", "Adicionar característica"))
        .clicked()
    {
        rows.push(json!({"Name": "", "Value": "", "Note": ""}));
    }
}

fn edit_row_text(ui: &mut egui::Ui, row: &mut Value, field: &str, width: f32, capture_probe: bool) {
    let mut draft = row
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let response = ui.add_sized([width, 22.0], egui::TextEdit::singleline(&mut draft));
    if capture_probe {
        let visible = ui.clip_rect().contains_rect(response.rect);
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                egui::Id::unique("sheet_value_probe"),
                if visible { Some(response.rect) } else { None },
            )
        });
    }
    if response.changed() {
        while draft.len() > MAX_TEXT_BYTES {
            draft.pop();
        }
        if let Some(object) = row.as_object_mut() {
            object.insert(field.to_owned(), Value::String(draft));
        }
    }
}

fn render_add_resource(ui: &mut egui::Ui, edit: &mut SheetEdit, language: Language) {
    let missing = RESOURCE_FIELDS
        .iter()
        .filter(|key| !edit.resources.contains_key(**key))
        .map(|key| (*key).to_owned())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        ui.label(label(
            language,
            "All supported values are present.",
            "Todos os valores disponíveis já estão presentes.",
        ));
        return;
    }

    let selector_id = ui.make_persistent_id("sheet_add_resource_choice");
    let mut choice = ui
        .data_mut(|data| data.get_temp::<String>(selector_id))
        .filter(|key| missing.contains(key))
        .unwrap_or_else(|| missing[0].clone());
    let mut add = false;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(selector_id)
            .selected_text(choice.clone())
            .show_ui(ui, |ui| {
                for key in &missing {
                    ui.selectable_value(&mut choice, key.clone(), resource_title(key, language));
                }
            });
        add = ui
            .button(label(language, "Add value", "Adicionar valor"))
            .clicked();
    });
    ui.data_mut(|data| data.insert_temp(selector_id, choice.clone()));
    if add {
        edit.resources.insert(choice, 0);
    }
}

fn render_player_link(
    ui: &mut egui::Ui,
    edit: &mut SheetEdit,
    language: Language,
    players: &[Summary],
) {
    let current = match &edit.player_link {
        PlayerLink::Keep => label(
            language,
            "Keep current link (including unresolved)",
            "Manter vínculo atual (incluindo não resolvido)",
        )
        .to_owned(),
        PlayerLink::Clear => {
            label(language, "Clear player link", "Remover vínculo de jogador").to_owned()
        }
        PlayerLink::Assign(id) => players
            .iter()
            .find(|player| player.kind.eq_ignore_ascii_case("Player") && player.id.as_str() == id)
            .map(|player| player.name.clone())
            .unwrap_or_else(|| {
                format!(
                    "{} ({id})",
                    label(language, "Unresolved player", "Jogador não encontrado")
                )
            }),
    };
    egui::ComboBox::from_id_salt("sheet_player_link")
        .selected_text(current)
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut edit.player_link,
                PlayerLink::Keep,
                label(
                    language,
                    "Keep current link (including unresolved)",
                    "Manter vínculo atual (incluindo não resolvido)",
                ),
            );
            ui.selectable_value(
                &mut edit.player_link,
                PlayerLink::Clear,
                label(language, "Clear player link", "Remover vínculo de jogador"),
            );
            for player in players
                .iter()
                .filter(|player| player.kind.eq_ignore_ascii_case("Player"))
            {
                ui.selectable_value(
                    &mut edit.player_link,
                    PlayerLink::Assign(player.id.clone()),
                    player.name.as_str(),
                );
            }
        });
}

pub(crate) fn list_title(key: &str, language: Language) -> String {
    let name = key.strip_suffix("List").unwrap_or(key);
    if language == Language::Portuguese {
        format!("Lista de {name}")
    } else {
        name.to_owned()
    }
}

fn resource_title(key: &str, language: Language) -> String {
    if let Some(base) = key.strip_prefix("Temp") {
        return format!(
            "{} · {}",
            label(language, "Temporary", "Temporário"),
            resource_title(base, language)
        );
    }
    if let Some(base) = key.strip_suffix("Max") {
        return format!(
            "{} · {}",
            label(language, "Maximum", "Máximo"),
            resource_title(base, language)
        );
    }
    if language == Language::Portuguese {
        match key {
            "Willpower" => return "Força de Vontade".into(),
            "Generation" => return "Geração".into(),
            "Blood" => return "Sangue".into(),
            "Conscience" => return "Consciência".into(),
            "Conviction" => return "Convicção".into(),
            "SelfControl" => return "Autocontrole".into(),
            "Instinct" => return "Instinto".into(),
            "Courage" => return "Coragem".into(),
            "PathTraits" => return "Características de Humanidade / Trilha".into(),
            "Physical" => return "Físicos".into(),
            "Social" => return "Sociais".into(),
            "Mental" => return "Mentais".into(),
            _ => {}
        }
    }
    let mut text = String::new();
    for (index, ch) in key.chars().enumerate() {
        if index > 0 && ch.is_uppercase() {
            text.push(' ');
        }
        text.push(ch);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_private_fields_in_both_languages_never_changes_sharing_or_values() {
        let mut edit = SheetEdit::default();
        edit.identity
            .insert("Clan".into(), "Private imported clan".into());
        edit.visibility.insert("Clan".into(), false);
        edit.lists
            .insert("DisciplineList".into(), empty_list("DisciplineList"));
        edit.visibility.insert("DisciplineList".into(), false);
        edit.resources.insert("Blood".into(), 4);
        edit.visibility.insert("Blood".into(), false);
        let original = edit.clone();
        let ctx = egui::Context::default();
        for scale in [1.0, 1.25] {
            ctx.set_zoom_factor(scale);
            for language in [Language::English, Language::Portuguese] {
                for _ in 0..2 {
                    let mut output = ctx.run_ui(Default::default(), |ui| {
                        render(ui, &mut edit, language, &[])
                    });
                    output.textures_delta.clear();
                    assert_eq!(edit, original);
                }
            }
        }
    }
}
