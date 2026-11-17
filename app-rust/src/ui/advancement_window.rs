use super::*;
use grapevine_workbench::advancement_draft::Review;

#[derive(Default)]
pub(super) struct AdvancementWindow {
    pub open: bool,
    pub target: Option<String>,
    pub review: Option<Review>,
    pub message: Option<String>,
}

impl InconnuApp {
    pub(super) fn open_advancement_draft(&mut self) {
        if self.route != WorkArea::Builder || self.has_unsaved_edit() {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        self.advancement = AdvancementWindow {
            open: true,
            target: Some(id),
            ..Default::default()
        };
    }

    pub(super) fn prepare_advancement_draft(&mut self) {
        self.advancement.review = None;
        if self.route != WorkArea::Builder || self.has_unsaved_edit() {
            return;
        }
        let (Some(lib), Some(id)) = (&self.library, &self.advancement.target) else {
            return;
        };
        match lib.review_advancement_draft(id, lib.revision().unwrap_or(-1), Role::Storyteller) {
            Ok(review) => {
                self.advancement.review = Some(review);
                self.advancement.message = None;
            }
            Err(error) => self.advancement.message = Some(error.message),
        }
    }

    pub(super) fn apply_advancement_draft(&mut self) {
        if self.route != WorkArea::Builder || self.has_unsaved_edit() {
            return;
        }
        let Some(review) = self.advancement.review.clone() else {
            return;
        };
        let Some(lib) = &mut self.library else {
            return;
        };
        match lib.apply_advancement_draft(&review, Role::Storyteller) {
            Ok(_) => {
                self.revision = lib.revision().ok();
                self.refresh_summaries(true);
                self.prepare_advancement_draft();
                self.advancement.message=Some(self.t("Planned purchases applied to the live character. Draft retained as an applied review record.","Compras planejadas aplicadas ao personagem ativo. Rascunho mantido como registro da revisão aplicada.").into());
            }
            Err(error) => self.advancement.message = Some(error.message),
        }
    }

    pub(super) fn render_advancement_draft(&mut self, ctx: &egui::Context) {
        if !self.advancement.open {
            return;
        }
        if self.route != WorkArea::Builder || self.advancement.target != self.selected_id {
            self.advancement = Default::default();
            return;
        }
        let mut open = true;
        let mut prepare = false;
        let mut apply = false;
        egui::Window::new(self.t("Saved advancement draft","Rascunho de evolução salvo"))
            .id(egui::Id::unique("advancement_draft_window")).default_size([690.0,550.0]).open(&mut open)
            .show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
                ui.label(self.t("Only the saved purchase plan can be applied. Draft sheet edits and its copied XP ledger never replace live data. A changed source or edited draft blocks application; create a fresh plan from the live character.","Somente o plano de compras salvo pode ser aplicado. Edições da ficha e o XP copiado do rascunho nunca substituem dados ativos. Fonte alterada ou rascunho editado bloqueiam a aplicação; crie um novo plano a partir do personagem ativo."));
                let b=ui.button(self.t("Review saved plan against live character","Revisar plano salvo com o personagem ativo"));
                remember(ui,&b,"advancement_draft_prepare");prepare=b.clicked();
                if let Some(message)=&self.advancement.message {ui.label(message);}
                if let Some(review)=&self.advancement.review {
                    ui.heading(&review.draft_name);ui.label(format!("{}: {}",self.t("Live character","Personagem ativo"),review.source_name));
                    for (index,step) in review.quote.steps.iter().enumerate() {
                        ui.strong(format!("{}. {} · {} + {} = {} XP · {} -> {}",index+1,step.price.name,step.price.price.base,step.price.price.affiliation_adjustment,step.price.price.total,step.before_unspent,step.after_unspent));
                        crate::purchase_ui::render_sources(ui,&step.price,self.language==Language::Portuguese);
                        egui::CollapsingHeader::new(format!("{} {}",self.t("Saved declarations","Declarações salvas"),index+1)).show(ui,|ui|{
                            crate::purchase_ui::render_declarations(ui,&step.request,self.language==Language::Portuguese);
                        });
                    }
                    ui.heading(format!("{}: {} XP · {} -> {} XP",self.t("Total plan cost","Custo total do plano"),review.quote.total_spend,review.quote.before_unspent,review.quote.after_unspent));
                    if review.applied {
                        ui.strong(self.t("Applied — no further XP will be spent","Aplicado — nenhum XP adicional será gasto"));
                    } else {
                        ui.label(self.t("Local declarations do not prove organizational approval. Automatic undo of a multi-purchase plan is not yet available.","Declarações locais não comprovam aprovação da organização. Desfazer automaticamente um plano com várias compras ainda não está disponível."));
                        let current=self.library.as_ref().and_then(|lib|lib.revision().ok())==Some(review.quote.revision);
                        if !current {ui.label(self.t("Library changed; review again.","Biblioteca alterada; revise novamente."));}
                        let b=ui.add_enabled(current && !self.has_unsaved_edit(),egui::Button::new(self.t("Apply saved purchases to live character","Aplicar compras salvas ao personagem ativo")));
                        remember(ui,&b,"advancement_draft_apply");apply=b.clicked();
                    }
                }
            });});
        self.advancement.open = open;
        if prepare {
            self.prepare_advancement_draft();
        }
        if apply {
            self.apply_advancement_draft();
        }
    }
}

fn remember(ui: &egui::Ui, response: &egui::Response, key: &str) {
    let rect = ui.is_rect_visible(response.rect).then_some(response.rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::unique(key), rect));
}
