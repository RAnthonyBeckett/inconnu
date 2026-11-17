use std::collections::BTreeSet;

use eframe::egui;
use grapevine_workbench::{
    model::{Language, Summary},
    player_points::{self, Command, Kind, Ledger},
    xp,
};

pub struct Form {
    pub selected_player: Option<String>,
    pub name: String,
    pub opening_balance: String,
    pub day: String,
    pub actor: String,
    pub reason: String,
    pub evidence: String,
    pub amount: String,
    pub reversal_id: String,
    pub kind: Kind,
    pub command_id: String,
    pub dirty: bool,
    pub message: Option<String>,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            selected_player: None,
            name: String::new(),
            opening_balance: String::new(),
            day: String::new(),
            actor: String::new(),
            reason: String::new(),
            evidence: String::new(),
            amount: String::new(),
            reversal_id: String::new(),
            kind: Kind::Award,
            command_id: uuid::Uuid::new_v4().to_string(),
            dirty: false,
            message: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Select(String),
    Create,
    Open,
    Post,
    Refresh,
}

fn t<'a>(pt: bool, en: &'a str, br: &'a str) -> &'a str {
    if pt { br } else { en }
}

fn player_label(id: &str, players: &[Summary]) -> String {
    players
        .iter()
        .find(|player| player.id == id)
        .map(|player| player.name.clone())
        .unwrap_or_else(|| id.to_owned())
}

fn kind_label(kind: Kind, pt: bool) -> &'static str {
    match kind {
        Kind::Award => t(pt, "Award", "Concessão"),
        Kind::Spend => t(pt, "Spend", "Gasto"),
        Kind::Adjustment => t(pt, "Adjustment", "Ajuste"),
        Kind::Reversal => t(pt, "Reversal", "Estorno"),
    }
}

fn positive_amount(value: &str) -> bool {
    xp::amount(value).is_ok_and(|amount| amount > 0)
}

fn remember_button(ui: &egui::Ui, response: &egui::Response, key: &str) {
    let rect = ui.is_rect_visible(response.rect).then_some(response.rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::unique(key), rect));
}

pub fn render(
    ui: &mut egui::Ui,
    form: &mut Form,
    players: &[Summary],
    view: Option<&Ledger>,
    language: Language,
) -> Option<Action> {
    let pt = language == Language::Portuguese;
    let before = (
        form.name.clone(),
        form.opening_balance.clone(),
        form.day.clone(),
        form.actor.clone(),
        form.reason.clone(),
        form.evidence.clone(),
        form.amount.clone(),
        form.reversal_id.clone(),
        form.kind,
        form.command_id.clone(),
    );
    let mut action = None;

    egui::ScrollArea::vertical()
        .id_salt("player_points_form")
        .max_height(690.0)
        .show(ui, |ui| {
            ui.heading(t(pt, "Player points", "Pontos do jogador"));
            ui.label(t(
                pt,
                "This is a separate manual ledger. Entries do not verify OWbN authority or alter character XP.",
                "Este é um livro-razão manual separado. Os lançamentos não verificam autorização da OWbN nem alteram o XP do personagem.",
            ));

            ui.horizontal_wrapped(|ui| {
                ui.label(t(pt, "Player", "Jogador"));
                let old = form.selected_player.clone();
                let selected = form
                    .selected_player
                    .as_deref()
                    .map(|id| player_label(id, players))
                    .unwrap_or_else(|| t(pt, "Select a player", "Selecione um jogador").to_owned());
                egui::ComboBox::from_id_salt("player_points_player")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for player in players {
                            ui.selectable_value(
                                &mut form.selected_player,
                                Some(player.id.clone()),
                                &player.name,
                            );
                        }
                    });
                if old != form.selected_player && let Some(id) = &form.selected_player { action = Some(Action::Select(id.clone())); }
                if ui.button(t(pt, "Refresh", "Atualizar")).clicked() {
                    action = Some(Action::Refresh);
                }
            });

            ui.separator();
            ui.strong(t(pt, "Create a Player record", "Criar registro de jogador"));
            ui.horizontal_wrapped(|ui| {
                ui.label(t(pt, "Name", "Nome"));
                ui.add(
                    egui::TextEdit::singleline(&mut form.name)
                        .desired_width(230.0)
                        .hint_text(t(pt, "Player name", "Nome do jogador")),
                );
                let create = ui.add_enabled(
                    !form.name.trim().is_empty(),
                    egui::Button::new(t(pt, "Create player", "Criar jogador")),
                );
                remember_button(ui, &create, "points_create");
                if create.clicked() {
                    action = Some(Action::Create);
                }
            });

            let Some(selected) = form.selected_player.as_deref() else {
                if let Some(message) = &form.message {
                    ui.label(message);
                }
                return;
            };
            ui.separator();
            ui.heading(player_label(selected, players));

            if let Some(ledger) = view {
                match player_points::replay(ledger) {
                    Ok(balance) => ui.strong(format!(
                        "{}: {balance}",
                        t(pt, "Current balance", "Saldo atual")
                    )),
                    Err(error) => ui.colored_label(egui::Color32::YELLOW, error.message),
                };

                ui.heading(t(pt, "Dated history", "Histórico por data"));
                let mut history: Vec<(usize, &Command)> =
                    ledger.entries.iter().enumerate().collect();
                history.sort_by(|a, b| a.1.day.cmp(&b.1.day).then(a.0.cmp(&b.0)));
                let reversed: BTreeSet<&str> = ledger
                    .entries
                    .iter()
                    .filter(|entry| entry.kind == Kind::Reversal)
                    .filter_map(|entry| entry.reversal_of.as_deref())
                    .collect();
                egui::ScrollArea::vertical()
                    .id_salt("player_points_history")
                    .max_height(190.0)
                    .show(ui, |ui| {
                        if history.is_empty() {
                            ui.small(t(pt, "No entries yet.", "Ainda não há lançamentos."));
                        }
                        for (_, entry) in &history {
                            ui.horizontal_wrapped(|ui| {
                                ui.strong(format!(
                                    "{} · {} · {} {}",
                                    entry.day,
                                    kind_label(entry.kind, pt),
                                    t(pt, "balance delta", "variação do saldo"),
                                    entry.amount
                                ));
                                ui.small(format!("{} · {}", entry.reason, entry.actor));
                            });
                        }
                    });

                ui.separator();
                ui.heading(t(pt, "Manual entry", "Lançamento manual"));
                ui.horizontal_wrapped(|ui| {
                    ui.label(t(pt, "Kind", "Tipo"));
                    egui::ComboBox::from_id_salt("player_points_kind")
                        .selected_text(kind_label(form.kind, pt))
                        .show_ui(ui, |ui| {
                            for kind in [Kind::Award, Kind::Spend, Kind::Adjustment, Kind::Reversal] {
                                ui.selectable_value(
                                    &mut form.kind,
                                    kind,
                                    kind_label(kind, pt),
                                );
                            }
                        });
                    if form.kind == Kind::Reversal {
                        form.amount.clear();
                    }
                    ui.label(t(pt, "Date (YYYY-MM-DD)", "Data (AAAA-MM-DD)"));
                    ui.add(
                        egui::TextEdit::singleline(&mut form.day)
                            .desired_width(120.0)
                            .hint_text("YYYY-MM-DD"),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(t(pt, "Recorded by", "Registrado por"));
                    ui.add(egui::TextEdit::singleline(&mut form.actor).desired_width(220.0));
                });
                ui.label(t(pt, "Reason", "Motivo"));
                ui.add(
                    egui::TextEdit::multiline(&mut form.reason)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.label(t(pt, "Evidence reference", "Referência de evidência"));
                ui.add(
                    egui::TextEdit::multiline(&mut form.evidence)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );

                if form.kind == Kind::Reversal {
                    let candidates: Vec<&Command> = ledger
                        .entries
                        .iter()
                        .filter(|entry| entry.kind != Kind::Reversal && !reversed.contains(entry.id.as_str()))
                        .collect();
                    let selected_target = candidates
                        .iter()
                        .find(|entry| entry.id == form.reversal_id)
                        .map(|entry| format!("{} · {} {}", entry.day, kind_label(entry.kind, pt), entry.amount))
                        .unwrap_or_else(|| t(pt, "Choose an original entry", "Escolha um lançamento original").to_owned());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(t(pt, "Original entry", "Lançamento original"));
                        egui::ComboBox::from_id_salt("player_points_reversal")
                            .selected_text(selected_target)
                            .show_ui(ui, |ui| {
                                for entry in &candidates {
                                    ui.selectable_value(
                                        &mut form.reversal_id,
                                        entry.id.clone(),
                                        format!("{} · {} {}", entry.day, kind_label(entry.kind, pt), entry.amount),
                                    );
                                }
                            });
                    });
                    ui.label(t(
                        pt,
                        "The signed balance delta is derived from the original; leave amount blank.",
                        "A variação assinada é calculada pelo lançamento original; deixe o valor em branco.",
                    ));
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(t(pt, "Amount", "Valor"));
                        ui.add(
                            egui::TextEdit::singleline(&mut form.amount)
                                .desired_width(110.0)
                                .hint_text("0.00"),
                        );
                        if !positive_amount(&form.amount)
                            && (form.kind != Kind::Adjustment || !form.amount.trim().is_empty())
                        {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                t(
                                    pt,
                                    "Award and spend need a positive amount; adjustment must be nonzero.",
                                    "Concessão e gasto exigem valor positivo; ajuste deve ser diferente de zero.",
                                ),
                            );
                        }
                    });
                }

                let post = ui.button(t(pt, "Post entry", "Registrar lançamento"));
                remember_button(ui, &post, "points_post");
                if post.clicked() {
                    action = Some(Action::Post);
                }
            } else {
                ui.label(t(
                    pt,
                    "Opening balance is a manual declaration; no earlier balance is inferred.",
                    "O saldo inicial é uma declaração manual; nenhum saldo anterior é presumido.",
                ));
                ui.horizontal_wrapped(|ui| {
                    ui.label(t(pt, "Opening balance", "Saldo inicial"));
                    ui.add(
                        egui::TextEdit::singleline(&mut form.opening_balance)
                            .desired_width(120.0)
                            .hint_text("0.00"),
                    );
                    ui.label(t(pt, "Opening date", "Data de abertura"));
                    ui.add(
                        egui::TextEdit::singleline(&mut form.day)
                            .desired_width(120.0)
                            .hint_text("YYYY-MM-DD"),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(t(pt, "Recorded by", "Registrado por"));
                    ui.add(egui::TextEdit::singleline(&mut form.actor).desired_width(220.0));
                });
                ui.label(t(pt, "Reason", "Motivo"));
                ui.add(
                    egui::TextEdit::multiline(&mut form.reason)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.label(t(pt, "Evidence reference", "Referência de evidência"));
                ui.add(
                    egui::TextEdit::multiline(&mut form.evidence)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                let opening = ui.button(t(pt, "Open points ledger", "Abrir livro-razão de pontos"));
                remember_button(ui, &opening, "points_opening");
                if opening.clicked() {
                    action = Some(Action::Open);
                }
            }

            if let Some(message) = &form.message {
                ui.label(message);
            }
        });

    let after = (
        form.name.clone(),
        form.opening_balance.clone(),
        form.day.clone(),
        form.actor.clone(),
        form.reason.clone(),
        form.evidence.clone(),
        form.amount.clone(),
        form.reversal_id.clone(),
        form.kind,
        form.command_id.clone(),
    );
    if before != after {
        form.dirty = true;
        form.message = None;
    }
    action
}

impl Form {
    pub fn entry_signature(&self) -> serde_json::Value {
        serde_json::json!([
            self.opening_balance,
            self.day,
            self.actor,
            self.reason,
            self.evidence,
            self.amount,
            self.reversal_id,
            self.kind
        ])
    }
}
