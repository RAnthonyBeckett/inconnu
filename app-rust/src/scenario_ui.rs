//! Temporary, read-only XP allocation comparisons. No purchase/storage action lives here.
use eframe::egui;
use grapevine_workbench::{
    model::{Language, Result},
    power_catalogue::{CatalogueEntry, ReviewStatus},
    pricing::{ClanRelation, MetTier, PricingProfile},
    scenarios::{self, ScenarioChoice, ScenarioPreview},
};
use std::collections::BTreeMap;

#[derive(Clone, Default, PartialEq)]
pub struct Allocation {
    pub profile: bool,
    pub choices: BTreeMap<String, AllocationChoice>,
    pub assumed_budget: bool,
    pub budget: String,
}

#[derive(Clone, Default, PartialEq)]
pub struct AllocationChoice {
    pub selected: bool,
    pub relation: Option<ClanRelation>,
}

#[derive(Default)]
pub struct ScenarioForm {
    pub family: usize,
    pub tier: TierFilter,
    pub allocation: Allocation,
    pub comparisons: [Option<Allocation>; 2],
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum TierFilter {
    #[default]
    Basic,
    Intermediate,
    Advanced,
    All,
}
impl TierFilter {
    pub fn matches(self, tier: MetTier) -> bool {
        self == Self::All
            || matches!(
                (self, tier),
                (Self::Basic, MetTier::Basic)
                    | (Self::Intermediate, MetTier::Intermediate)
                    | (Self::Advanced, MetTier::Advanced)
            )
    }
    fn label(self, pt: bool) -> &'static str {
        match self {
            Self::All => t(pt, "All tiers", "Todos os níveis"),
            Self::Basic => crate::purchase_ui::tier_label(pt, MetTier::Basic),
            Self::Intermediate => crate::purchase_ui::tier_label(pt, MetTier::Intermediate),
            Self::Advanced => crate::purchase_ui::tier_label(pt, MetTier::Advanced),
        }
    }
}

fn powers(kind: &str) -> Vec<CatalogueEntry> {
    grapevine_workbench::power_catalogue::entries()
        .into_iter()
        .filter(|entry| {
            entry.template_scope == kind && entry.review.source_identity == ReviewStatus::Reviewed
        })
        .collect()
}
fn families(kind: &str) -> Vec<String> {
    let mut families = Vec::new();
    for entry in powers(kind) {
        if !families.contains(&entry.family) {
            families.push(entry.family);
        }
    }
    families
}

impl Allocation {
    pub fn evaluate(&self, kind: &str, committed: Option<&str>) -> Result<ScenarioPreview> {
        let choices: Vec<_> = self
            .choices
            .iter()
            .filter(|(_, choice)| choice.selected)
            .map(|(id, choice)| ScenarioChoice {
                power_id: id.clone(),
                relation: choice.relation,
            })
            .collect();
        scenarios::preview(
            kind,
            if self.assumed_budget {
                Some(self.budget.as_str())
            } else {
                committed
            },
            self.profile.then_some(if kind == "Vampire" {
                PricingProfile::MetLawsOfTheNightRevised
            } else {
                PricingProfile::ConfiguredCreature
            }),
            &choices,
            &[],
        )
    }
}

fn t<'a>(pt: bool, en: &'a str, br: &'a str) -> &'a str {
    if pt { br } else { en }
}
fn remember(ui: &egui::Ui, response: &egui::Response, key: &str) {
    // Determine visibility before taking the context's data lock.
    let visible = ui.is_rect_visible(response.rect).then_some(response.rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), visible));
}

pub fn render(
    ui: &mut egui::Ui,
    form: &mut ScenarioForm,
    kind: &str,
    committed: Option<&str>,
    language: Language,
    existing_names: &[String],
) {
    let pt = language == Language::Portuguese;
    ui.label(t(
        pt,
        "Temporary XP scenarios. No XP is spent and no sheet is changed.",
        "Cenários temporários de XP. Nenhum XP é gasto e a ficha não é alterada.",
    ));
    if kind == "Vampire" {
        ui.small(t(pt,"Forty reviewed powers across eight Disciplines and three MET tiers. Family/tier filters retain the combined cart. Other templates and point pools remain unpriced.","Quarenta poderes revisados em oito Disciplinas e três níveis MET. Filtros de família/nível mantêm as escolhas combinadas. Outros modelos e tipos de pontos ainda não têm preços."));
    } else {
        ui.small(t(pt,"Only source-reviewed catalogue entries scoped to this character kind are shown. Unreviewed configured rules remain unavailable.","Somente entradas de catálogo revisadas e vinculadas a este tipo de personagem são exibidas. Regras configuradas não revisadas permanecem indisponíveis."));
    }
    ui.small(t(pt,"Closing retains choices. Changing character, library, work area or using main Refresh clears them. Reload baseline retains and recalculates them.","Fechar mantém as escolhas. Trocar personagem, biblioteca, área ou usar Atualizar limpa os dados. Recarregar a base mantém e recalcula as escolhas."));
    let a = &mut form.allocation;
    let p = ui.checkbox(
        &mut a.profile,
        if kind == "Vampire" {
            t(
                pt,
                "Preview using Laws of the Night (Revised)",
                "Simular com Laws of the Night (Revised)",
            )
        } else {
            t(
                pt,
                "Preview using reviewed creature rules",
                "Simular com regras revisadas do tipo de personagem",
            )
        },
    );
    remember(ui, &p, "scenario_profile");
    ui.small(t(
        pt,
        "Choosing a book for simulation does not establish local rules adoption.",
        "Escolher um livro para simulação não estabelece sua adoção pelas regras locais.",
    ));
    ui.horizontal_wrapped(|ui| {
        let b = ui.checkbox(
            &mut a.assumed_budget,
            t(
                pt,
                "Use hypothetical XP budget",
                "Usar saldo hipotético de XP",
            ),
        );
        remember(ui, &b, "scenario_assumed");
        if a.assumed_budget {
            let b = ui.add(
                egui::TextEdit::singleline(&mut a.budget)
                    .desired_width(100.0)
                    .hint_text("XP"),
            );
            remember(ui, &b, "scenario_budget");
        } else {
            ui.label(format!(
                "{}: {}",
                t(pt, "Committed ledger balance", "Saldo do livro salvo"),
                committed.unwrap_or(t(pt, "unknown", "desconhecido"))
            ));
        }
    });
    if !a.assumed_budget && committed.is_none() {
        ui.small(t(pt,"No supported committed XP baseline. Costs can be compared; affordability is unknown.","Sem saldo inicial de XP salvo e compatível. Os custos podem ser comparados; a disponibilidade é desconhecida."));
    }
    let available_powers = powers(kind);
    let families = families(kind);
    if families.is_empty() {
        if kind != "Vampire" {
            ui.small(t(
                pt,
                "No reviewed power entries are available for this character kind.",
                "Não há poderes revisados disponíveis para este tipo de personagem.",
            ));
        }
        return;
    }
    let family_index = form.family.min(families.len() - 1);
    let family_menu = egui::ComboBox::from_id_salt("scenario_family")
        .selected_text(families[family_index].as_str())
        .show_ui(ui, |ui| {
            for (index, name) in families.iter().enumerate() {
                let option = ui.selectable_value(&mut form.family, index, name.as_str());
                if index == 1 {
                    remember(ui, &option, "scenario_family_auspex");
                }
            }
        });
    remember(ui, &family_menu.response, "scenario_family_menu");
    let tier_menu = egui::ComboBox::from_id_salt("scenario_tier")
        .selected_text(form.tier.label(pt))
        .show_ui(ui, |ui| {
            for tier in [
                TierFilter::Basic,
                TierFilter::Intermediate,
                TierFilter::Advanced,
                TierFilter::All,
            ] {
                let b = ui.selectable_value(&mut form.tier, tier, tier.label(pt));
                if tier == TierFilter::Intermediate {
                    remember(ui, &b, "scenario_tier_intermediate");
                }
                if tier == TierFilter::Advanced {
                    remember(ui, &b, "scenario_tier_advanced");
                }
            }
        });
    remember(ui, &tier_menu.response, "scenario_tier_menu");
    for (i, power) in available_powers.iter().enumerate().filter(|(_, p)| {
        p.family.as_str() == families[family_index].as_str() && form.tier.matches(p.tier)
    }) {
        ui.push_id(i,|ui| {
            let choice = a.choices.entry(power.stable_id.clone()).or_default();
            let b=ui.checkbox(&mut choice.selected,power.name.as_str());
            remember(ui,&b,&format!("scenario_power{i}"));
            if choice.selected {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut choice.relation,None,t(pt,"Affiliation unknown","Afiliação desconhecida"));
                    let b=ui.selectable_value(&mut choice.relation,Some(ClanRelation::InClan),t(pt,"In-clan","Do clã"));
                    remember(ui,&b,&format!("scenario_in{i}"));
                    let b=ui.selectable_value(&mut choice.relation,Some(ClanRelation::OutOfClan),t(pt,"Out-of-clan","Fora do clã"));
                    remember(ui,&b,&format!("scenario_out{i}"));
                });
                if existing_names.iter().any(|n| n.eq_ignore_ascii_case(&power.name)) {
                    ui.colored_label(egui::Color32::YELLOW,t(pt,"A matching label already appears on the committed sheet. Identity/eligibility needs review.","Um nome igual já aparece na ficha salva. Identidade e elegibilidade precisam de revisão."));
                }
            }
        });
    }
    let result = form.allocation.evaluate(kind, committed);
    match &result {
        Err(e) => {
            ui.colored_label(egui::Color32::YELLOW, &e.message);
        }
        Ok(q) => {
            ui.separator();
            for item in &q.items {
                ui.label(format!(
                    "{}: {} + {} = {} XP",
                    item.name, item.price.base, item.price.affiliation_adjustment, item.price.total
                ));
            }
            summary(ui, q, form.allocation.assumed_budget, pt);
            if let Some(first) = q.items.first() {
                egui::CollapsingHeader::new(t(pt,"Sources and unresolved requirements","Fontes e requisitos pendentes")).show(ui,|ui| {
                    ui.label(format!("{} · {} · {}",first.price.citation.book,first.price.citation.edition,first.price.citation.system));
                    if kind == "Vampire" {
                        ui.label(t(pt,"Cost: printed p.124 / PDF p.125; learning: printed p.133 / PDF p.134.","Custo: p.124 impressa / p.125 PDF; aprendizado: p.133 impressa / p.134 PDF."));
                    }
                    for item in &q.items {
                        let family = item.catalogue_binding.as_ref().map(|binding| binding.family.as_str()).unwrap_or_else(|| item.power.family());
                        ui.small(format!("{} · {} · p.{} / PDF{}",family,item.name,item.power_source.citation.printed_page,item.power_source.citation.pdf_page_1based));
                    }
                    ui.label(&first.price.library_prompt);
                    ui.small(t(pt,"Teacher interpretation, local adoption, affiliation evidence and approval are unresolved. Portuguese source-content review is pending. No prerequisite is inferred from listing order.","Interpretação do professor, adoção local, evidência de afiliação e aprovação estão pendentes. Revisão do conteúdo em português pendente. A ordem da lista não define pré-requisito."));
                    ui.small(format!("PDF SHA256 {}",first.price.citation.original_pdf_sha256));
                });
            }
        }
    }
    ui.horizontal_wrapped(|ui| {
        for i in 0..2 {
            let b = ui.add_enabled(
                result.is_ok(),
                egui::Button::new(format!(
                    "{} {}",
                    t(pt, "Keep comparison", "Guardar comparação"),
                    if i == 0 { "A" } else { "B" }
                )),
            );
            remember(
                ui,
                &b,
                if i == 0 {
                    "scenario_keep_a"
                } else {
                    "scenario_keep_b"
                },
            );
            if b.clicked() {
                form.comparisons[i] = Some(form.allocation.clone());
            }
        }
        let b = ui.button(t(pt, "Reset all", "Limpar tudo"));
        remember(ui, &b, "scenario_reset");
        if b.clicked() {
            *form = ScenarioForm::default();
        }
    });
    for i in 0..2 {
        if let Some(saved) = form.comparisons[i].clone() {
            ui.group(|ui| {
                ui.strong(format!(
                    "{} {}",
                    t(pt, "Comparison", "Comparação"),
                    if i == 0 { "A" } else { "B" }
                ));
                ui.small(
                    powers(kind)
                        .iter()
                        .filter(|power| {
                            saved
                                .choices
                                .get(power.stable_id.as_str())
                                .is_some_and(|c| c.selected)
                        })
                        .map(|power| power.name.as_str())
                        .collect::<Vec<_>>()
                        .join(" + "),
                );
                match saved.evaluate(kind, committed) {
                    Ok(q) => summary(ui, &q, saved.assumed_budget, pt),
                    Err(e) => {
                        ui.label(e.message);
                    }
                }
                if ui
                    .small_button(t(pt, "Load choices", "Carregar escolhas"))
                    .clicked()
                {
                    form.allocation = saved;
                }
            });
        }
    }
    ui.strong(t(
        pt,
        "Preview only. Purchase/apply is not available.",
        "Somente simulação. Compra/aplicação indisponível.",
    ));
}

fn summary(ui: &mut egui::Ui, q: &ScenarioPreview, assumed: bool, pt: bool) {
    ui.strong(format!(
        "{}: {} XP",
        t(pt, "Combined cost", "Custo combinado"),
        q.total_spend
    ));
    ui.label(format!(
        "{}: {} -> {} XP",
        if assumed {
            t(pt, "Hypothetical balance", "Saldo hipotético")
        } else {
            t(pt, "Ledger balance", "Saldo do livro")
        },
        q.available.as_deref().unwrap_or("?"),
        q.projected.as_deref().unwrap_or("?")
    ));
    if let Some(shortfall) = &q.shortfall
        && shortfall != "0"
    {
        ui.colored_label(
            egui::Color32::YELLOW,
            format!("{}: {shortfall} XP", t(pt, "Shortfall", "Faltam")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grapevine_workbench::purchase_preview::AnimalismPower;

    #[test]
    fn selections_are_bound_to_ids_and_unknown_selected_ids_fail_closed() {
        let mut a = Allocation {
            profile: true,
            assumed_budget: true,
            budget: "10".into(),
            ..Default::default()
        };
        for power in [
            AnimalismPower::HeightenedSenses,
            AnimalismPower::FeralWhispers,
        ] {
            a.choices.insert(
                power.stable_id().into(),
                AllocationChoice {
                    selected: true,
                    relation: Some(ClanRelation::OutOfClan),
                },
            );
        }
        let saved = a.clone();
        assert_eq!(a.evaluate("Vampire", None).unwrap().total_spend, "8");
        a.choices.insert(
            "unreviewed.power".into(),
            AllocationChoice {
                selected: false,
                relation: None,
            },
        );
        assert_eq!(a.evaluate("Vampire", None).unwrap().total_spend, "8");
        a.choices.get_mut("unreviewed.power").unwrap().selected = true;
        assert!(a.evaluate("Vampire", None).is_err());
        assert_eq!(saved.evaluate("Vampire", None).unwrap().total_spend, "8");
    }
}
