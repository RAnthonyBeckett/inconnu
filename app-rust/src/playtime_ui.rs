use eframe::egui;
use grapevine_workbench::{
    model::Language,
    playtime::{AwardClass, AwardReview, AwardSnapshot, CapClass, Request},
};
use serde_json::Value;
#[derive(Default)]
pub struct Form {
    pub input: Request,
    pub report: Option<Value>,
    pub report_input: Option<Request>,
    pub report_revision: Option<i64>,
    pub message: Option<String>,
    pub awards: Vec<AwardSnapshot>,
    pub award_revision: Option<i64>,
    pub award_start: String,
    pub award_end: String,
    pub classification_reason: String,
    pub classification_reference: String,
    pub saved_reviews: Vec<grapevine_workbench::playtime_review::SavedReview>,
    pub selected_review: Option<String>,
    pub save_review_id: Option<String>,
    pub pending_load: Option<String>,
}
impl Form {
    pub fn load_awards(&mut self, rows: Vec<AwardSnapshot>, revision: i64) {
        let same = self.awards_current(revision) && self.awards == rows;
        let hashes = rows
            .iter()
            .map(|row| (row.entry_id.as_str(), row.entry_hash.as_str()))
            .collect::<std::collections::HashMap<_, _>>();
        let before = self.input.award_reviews.len();
        self.input.award_reviews.retain(|review| {
            hashes.get(review.entry_id.as_str()) == Some(&review.entry_hash.as_str())
        });
        let retained_unchanged = before == self.input.award_reviews.len();
        if !retained_unchanged {
            self.save_review_id = None;
        }
        self.awards = rows;
        self.award_revision = Some(revision);
        self.award_start = self.input.start_month.clone();
        self.award_end = self.input.end_month.clone();
        if !same || !retained_unchanged {
            self.invalidate();
        } else {
            self.message = None;
        }
    }
    pub fn awards_current(&self, revision: i64) -> bool {
        self.award_revision == Some(revision)
            && self.award_start == self.input.start_month
            && self.award_end == self.input.end_month
    }
    pub fn invalidate(&mut self) {
        self.report = None;
        self.report_input = None;
        self.report_revision = None;
        self.message = None;
    }
    pub fn mark_all_regular(&mut self) {
        self.input.award_reviews = self
            .awards
            .iter()
            .map(|row| AwardReview {
                entry_id: row.entry_id.clone(),
                entry_hash: row.entry_hash.clone(),
                class: AwardClass::Regular,
                reason: self.classification_reason.clone(),
                reference: self.classification_reference.clone(),
            })
            .collect();
        self.invalidate();
    }
}
pub enum Action {
    Calculate,
    Export,
    LoadAwards,
    SavePrivate,
    LoadPrivate(String),
}
fn t<'a>(pt: bool, en: &'a str, br: &'a str) -> &'a str {
    if pt { br } else { en }
}
fn status(pt: bool, value: &str) -> &str {
    match value {
        "WithinDeclaredCeiling" => t(pt, "Within declared ceiling", "Dentro do limite declarado"),
        "ExceedsDeclaredCeiling" => t(
            pt,
            "Exceeds declared ceiling — review required",
            "Acima do limite declarado — revisão necessária",
        ),
        "InsufficientEvidence" => t(pt, "Insufficient evidence", "Evidência insuficiente"),
        _ => value,
    }
}
fn mark(ui: &egui::Ui, key: &str, rect: egui::Rect) {
    let visible = ui.is_rect_visible(rect).then_some(rect);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(key), visible));
}
pub fn render(
    ui: &mut egui::Ui,
    form: &mut Form,
    revision: i64,
    language: Language,
) -> Option<Action> {
    let pt = language == Language::Portuguese;
    let before = form.input.clone();
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        let b = ui.button(t(pt, "Calculate monthly ceiling", "Calcular limite mensal"));
        mark(ui, "playtime_calculate", b.interact_rect);
        if b.clicked() {
            action = Some(Action::Calculate);
        }
        let b = ui.button(t(
            pt,
            "Load / reload awards",
            "Carregar / recarregar concessões",
        ));
        mark(ui, "playtime_load_awards", b.interact_rect);
        if b.clicked() {
            action = Some(Action::LoadAwards);
        }
        let enabled = !form.awards.is_empty()
            && form.awards_current(revision)
            && !form.classification_reason.trim().is_empty()
            && !form.classification_reference.trim().is_empty();
        let b = ui.add_enabled(
            enabled,
            egui::Button::new(t(
                pt,
                "Mark shown awards regular",
                "Marcar concessões exibidas como regulares",
            )),
        );
        mark(ui, "playtime_mark_regular", b.interact_rect);
        if b.clicked() {
            form.mark_all_regular();
        }
    });
    if let Some(private_action) = render_private_controls(ui, form, revision, pt) {
        action = Some(private_action);
    }
    egui::ScrollArea::vertical().id_salt("playtime_scroll").show(ui,|ui|{
  ui.label(t(pt,"Compare explicitly reviewed regular XP with calendar-month limits. Separate-review and unclassified awards stay unresolved. This does not approve XP or a transfer.","Compare XP regular explicitamente revisado com limites mensais. Concessões de revisão separada e não classificadas permanecem pendentes. Isto não aprova XP ou transferência."));
  ui.hyperlink_to(t(pt,"OWbN Character Bylaws — Experience Points","Estatuto de personagens da OWbN — Experiência"),grapevine_workbench::playtime::SOURCE_URL);
  for (en,br,value) in [("Start month YYYY-MM","Mês inicial AAAA-MM",&mut form.input.start_month),("End month YYYY-MM","Mês final AAAA-MM",&mut form.input.end_month),("Home chronicle","Crônica de origem",&mut form.input.home_chronicle),("Policy applicability / complete-history review reference","Referência da regra aplicável / histórico completo",&mut form.input.review_evidence)]{ui.label(t(pt,en,br));ui.text_edit_singleline(value);}
  ui.horizontal_wrapped(|ui|{ui.selectable_value(&mut form.input.class,CapClass::Unknown,t(pt,"Classification unresolved","Classificação pendente"));ui.selectable_value(&mut form.input.class,CapClass::Standard,t(pt,"Standard monthly bands","Faixas mensais padrão"));ui.selectable_value(&mut form.input.class,CapClass::Wraith,t(pt,"Wraith monthly limit","Limite mensal de Wraith"));});
  ui.checkbox(&mut form.input.policy_scope_confirmed,t(pt,"Reviewed policy applicability throughout this range; historical applicability is not assumed.","Revisada a validade da regra em todo o período; não é presumida a aplicação histórica."));
  ui.checkbox(&mut form.input.complete_regular_history_confirmed,t(pt,"Reviewed complete regular history and unresolved creation, rollover or special-award issues.","Revisados o histórico regular completo e os pontos pendentes sobre concessões iniciais, transferidas ou especiais."));
  ui.checkbox(&mut form.input.full_month_eligible_confirmed,t(pt,"Reviewed home-chronicle authority and full-month eligibility.","Revisadas a autoridade da crônica de origem e a elegibilidade para o mês completo."));
  ui.separator();ui.heading(t(pt,"Award classifications for this review","Classificações das concessões nesta revisão"));
  ui.label(t(pt,"No default is regular. These local declarations do not change XP, establish an exemption or approve an award. Enter a common reason/reference before using Mark shown awards regular, or review rows individually.","Nenhuma concessão é regular por padrão. Estas declarações locais não alteram XP, estabelecem exceção ou aprovam concessão. Insira motivo/referência comuns antes de marcar todas como regulares, ou revise as linhas individualmente."));
  ui.label(t(pt,"Common classification reason","Motivo comum da classificação"));ui.text_edit_singleline(&mut form.classification_reason);
  ui.label(t(pt,"Common classification reference","Referência comum da classificação"));ui.text_edit_singleline(&mut form.classification_reference);
  if !form.awards_current(revision){ui.colored_label(egui::Color32::YELLOW,t(pt,"Load awards for the current range and library revision before classifying.","Carregue concessões para o período e a revisão atuais antes de classificar."));}
  else{
   let indices=form.input.award_reviews.iter().enumerate().map(|(i,r)|(r.entry_id.clone(),i)).collect::<std::collections::HashMap<_,_>>();
   let mut remove=vec![];let mut add=vec![];
   if form.awards.is_empty(){ui.label(t(pt,"No Award entries in this loaded range.","Nenhuma entrada de concessão neste período carregado."));}
   for row in &form.awards{
    ui.push_id(&row.entry_id,|ui|{
     let existing=indices.get(&row.entry_id).copied();let mut selected=existing.map(|i|form.input.award_reviews[i].class);let previous=selected;
     ui.horizontal_wrapped(|ui|{ui.label(format!("{} · {} XP",row.day,row.amount));egui::ComboBox::from_id_salt("class").selected_text(match selected{None=>t(pt,"Unclassified","Não classificada"),Some(AwardClass::Regular)=>t(pt,"Regular for this review","Regular nesta revisão"),Some(AwardClass::SeparateReview)=>t(pt,"Separate review required","Revisão separada necessária")}).show_ui(ui,|ui|{ui.selectable_value(&mut selected,None,t(pt,"Unclassified","Não classificada"));ui.selectable_value(&mut selected,Some(AwardClass::Regular),t(pt,"Regular for this review","Regular nesta revisão"));ui.selectable_value(&mut selected,Some(AwardClass::SeparateReview),t(pt,"Separate review required","Revisão separada necessária"));});});
     if previous!=selected{match (existing,selected){(Some(_),None)=>remove.push(row.entry_id.clone()),(Some(i),Some(class))=>form.input.award_reviews[i].class=class,(None,Some(class))=>add.push(AwardReview{entry_id:row.entry_id.clone(),entry_hash:row.entry_hash.clone(),class,reason:String::new(),reference:String::new()}),_=>{}}}
     if let Some(i)=existing{let review=&mut form.input.award_reviews[i];ui.label(t(pt,"Classification reason","Motivo da classificação"));ui.text_edit_singleline(&mut review.reason);ui.label(t(pt,"Classification reference","Referência da classificação"));ui.text_edit_singleline(&mut review.reference);}
     ui.collapsing(t(pt,"Private original reason / evidence","Motivo / evidência originais privados"),|ui|{ui.label(&row.reason);ui.label(&row.evidence);});
    });
   }
   form.input.award_reviews.retain(|r|!remove.contains(&r.entry_id));form.input.award_reviews.extend(add);
  }
  if before.start_month!=form.input.start_month||before.end_month!=form.input.end_month||before.award_reviews!=form.input.award_reviews{form.save_review_id=None;}
  if before!=form.input{form.invalidate();}
  if let Some(message)=&form.message{ui.label(message);}
  if let Some(value)=&form.report{
   let current=form.report_revision==Some(revision)&&form.report_input.as_ref()==Some(&form.input);
   if !current{ui.colored_label(egui::Color32::YELLOW,t(pt,"Library changed; recalculate before exporting.","Biblioteca alterada; recalcule antes de exportar."));}
   let report=&value["report"];ui.small(format!("{}: {}",t(pt,"Policy version","Versão da regra"),report["policy_version"].as_str().unwrap_or("?")));ui.heading(status(pt,report["status"].as_str().unwrap_or("InsufficientEvidence")));
   if let Some(rows)=report["months"].as_array(){egui::Grid::new("playtime_months").striped(true).show(ui,|ui|{
    for label in [t(pt,"Month","Mês"),t(pt,"Opening XP","XP inicial"),t(pt,"Awards by classification","Concessões por classificação"),t(pt,"Ceiling","Limite"),t(pt,"Result","Resultado")]{ui.strong(label);}ui.end_row();
    for row in rows{
     ui.label(row["month"].as_str().unwrap_or("?"));ui.label(row["opening_earned"].as_str().unwrap_or("?"));
     ui.label(format!("{}: {}\n{}: {}\n{}: {}\n{}: {}",t(pt,"Regular","Regular"),row["regular_awarded"].as_str().unwrap_or("?"),t(pt,"Separate review","Revisão separada"),row["separate_review_awarded"].as_str().unwrap_or("?"),t(pt,"Unclassified","Não classificada"),row["unclassified_awarded"].as_str().unwrap_or("?"),t(pt,"All awards","Todas as concessões"),row["source_awarded"].as_str().unwrap_or("?")));
     ui.label(row["ceiling"].as_str().unwrap_or("?"));ui.label(status(pt,row["status"].as_str().unwrap_or("InsufficientEvidence")));ui.end_row();
    }
   });for row in rows{if let Some(issues)=row["issues"].as_array(){for issue in issues{ui.small(format!("{}: {}",row["month"].as_str().unwrap_or("?"),issue.as_str().unwrap_or("")));}}}}
   if let Some(limits)=report["limitations"].as_array(){for limit in limits{ui.small(limit.as_str().unwrap_or(""));}}
   if ui.add_enabled(current,egui::Button::new(t(pt,"Export transfer review summary…","Exportar resumo para revisão de transferência…"))).clicked(){action=Some(Action::Export);}
   ui.small(t(pt,"Summary includes a classification digest and subtotals, omitting raw award IDs/history, ST notes, chronicle name and private evidence text. Local declarations do not prove authority.","O resumo inclui um resumo criptográfico das classificações e subtotais, omitindo IDs/histórico bruto, notas do Narrador, nome da crônica e texto de evidência privada. Declarações locais não comprovam autoridade."));
  }
 });
    action
}

fn render_private_controls(
    ui: &mut egui::Ui,
    form: &mut Form,
    revision: i64,
    pt: bool,
) -> Option<Action> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        let save = ui.add_enabled(
            !form.input.award_reviews.is_empty() && form.awards_current(revision),
            egui::Button::new(t(pt, "Save private choices", "Salvar escolhas privadas")),
        );
        mark(ui, "playtime_save_private", save.interact_rect);
        if save.clicked() {
            action = Some(Action::SavePrivate);
        }
        egui::ComboBox::from_id_salt("private_review_selection")
            .selected_text(
                form.selected_review
                    .as_ref()
                    .and_then(|id| form.saved_reviews.iter().find(|r| &r.id == id))
                    .map(|r| {
                        format!(
                            "{} / {} · {} {}",
                            r.start_month,
                            r.end_month,
                            t(pt, "revision", "revisão"),
                            r.saved_revision
                        )
                    })
                    .unwrap_or_else(|| {
                        t(pt, "Saved private reviews", "Revisões privadas salvas").into()
                    }),
            )
            .show_ui(ui, |ui| {
                for review in form.saved_reviews.iter().rev() {
                    ui.selectable_value(
                        &mut form.selected_review,
                        Some(review.id.clone()),
                        format!(
                            "{} / {} · {} {}",
                            review.start_month,
                            review.end_month,
                            t(pt, "revision", "revisão"),
                            review.saved_revision
                        ),
                    );
                }
            });
        let load = ui.add_enabled(
            form.selected_review.is_some(),
            egui::Button::new(t(pt, "Load saved choices…", "Carregar escolhas salvas…")),
        );
        mark(ui, "playtime_load_private", load.interact_rect);
        if load.clicked() {
            form.pending_load = form.selected_review.clone();
        }
    });
    if let Some(id) = form.pending_load.clone() {
        ui.label(t(pt,"Replace current inputs with saved choices? Policy and eligibility confirmations will be reset.","Substituir os dados atuais pelas escolhas salvas? As confirmações de regra e elegibilidade serão redefinidas."));
        ui.horizontal_wrapped(|ui| {
            let load = ui.button(t(pt, "Load saved choices", "Carregar escolhas salvas"));
            mark(ui, "playtime_confirm_load", load.interact_rect);
            if load.clicked() {
                form.pending_load = None;
                action = Some(Action::LoadPrivate(id));
            }
            if ui
                .button(t(pt, "Keep current review", "Manter revisão atual"))
                .clicked()
            {
                form.pending_load = None;
            }
        });
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use grapevine_workbench::{
        model::Role,
        store::Library,
        xp::{Baseline, XpCommand, XpKind},
    };
    #[test]
    fn bilingual_review_reload_keeps_exact_choices_and_new_awards_remain_pending() {
        let dir = tempfile::tempdir().unwrap();
        let mut lib =
            Library::create(&dir.path().join("review.gvlib"), "Synthetic review").unwrap();
        let id = lib
            .create_character("Vampire", "Synthetic review", 0, Role::Storyteller)
            .unwrap();
        lib.initialize_xp(
            &id,
            &Baseline {
                day: "2026-01-01".into(),
                earned: "10".into(),
                unspent: "10".into(),
                actor: "PRIVATE_UI_ACTOR".into(),
                reason: "Synthetic opening".into(),
                evidence: "PRIVATE_UI_SOURCE".into(),
            },
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
        let award = |lib: &mut Library| {
            lib.post_xp(
                &id,
                &XpCommand {
                    id: uuid::Uuid::new_v4().to_string(),
                    kind: XpKind::Award,
                    amount: "1".into(),
                    day: "2026-01-10".into(),
                    actor: "PRIVATE_UI_ACTOR".into(),
                    reason: "PRIVATE_UI_SOURCE_REASON".into(),
                    evidence: "PRIVATE_UI_SOURCE_EVIDENCE".into(),
                    reversal_of: None,
                    cost: None,
                },
                lib.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap();
        };
        award(&mut lib);
        let revision = lib.revision().unwrap();
        let mut form = Form {
            input: Request {
                start_month: "2026-01".into(),
                end_month: "2026-01".into(),
                class: CapClass::Standard,
                policy_scope_confirmed: true,
                complete_regular_history_confirmed: true,
                full_month_eligible_confirmed: true,
                home_chronicle: "PRIVATE_UI_CHRONICLE".into(),
                review_evidence: "PRIVATE_UI_REVIEW".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let rows = lib
            .playtime_awards(&id, "2026-01", "2026-01", revision, Role::Storyteller)
            .unwrap();
        form.load_awards(rows.clone(), revision);
        assert_eq!(
            lib.playtime_report(&id, &form.input, revision, Role::Storyteller)
                .unwrap()["report"]["status"],
            "InsufficientEvidence"
        );
        form.classification_reason = "PRIVATE_UI_CLASSIFICATION_REASON".into();
        form.classification_reference = "PRIVATE_UI_CLASSIFICATION_REF".into();
        form.mark_all_regular();
        let report = lib
            .playtime_report(&id, &form.input, revision, Role::Storyteller)
            .unwrap();
        assert_eq!(report["report"]["status"], "WithinDeclaredCeiling");
        assert!(!report.to_string().contains("PRIVATE_UI"));
        form.report = Some(report);
        form.report_input = Some(form.input.clone());
        form.report_revision = Some(revision);
        let expected = form.input.clone();
        let ctx = egui::Context::default();
        for language in [Language::English, Language::Portuguese] {
            for _ in 0..2 {
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    render(ui, &mut form, revision, language);
                });
                out.textures_delta.clear();
            }
            assert_eq!(form.input, expected);
            assert!(form.report.is_some());
        }
        form.load_awards(rows, revision);
        assert_eq!(form.input, expected);
        assert!(form.report.is_some());
        assert_eq!(lib.revision().unwrap(), revision);
        assert_eq!(
            lib.xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("11")
        );
        award(&mut lib);
        let next = lib.revision().unwrap();
        assert!(!form.awards_current(next));
        form.load_awards(
            lib.playtime_awards(&id, "2026-01", "2026-01", next, Role::Storyteller)
                .unwrap(),
            next,
        );
        assert_eq!(form.input.award_reviews.len(), 1);
        assert_eq!(form.awards.len(), 2);
        assert!(form.report.is_none());
        let next_report = lib
            .playtime_report(&id, &form.input, next, Role::Storyteller)
            .unwrap();
        assert_eq!(next_report["report"]["status"], "InsufficientEvidence");
        assert_eq!(
            next_report["report"]["months"][0]["unclassified_awarded"],
            "1"
        );
        let saved = lib
            .save_playtime_award_review(
                &id,
                &uuid::Uuid::new_v4().to_string(),
                &form.input,
                next,
                Role::Storyteller,
            )
            .unwrap();
        let saved_revision = lib.revision().unwrap();
        assert_eq!(
            lib.playtime_report(&id, &form.input, saved_revision, Role::Storyteller)
                .unwrap()["record_hash"],
            next_report["record_hash"]
        );
        form.saved_reviews = vec![saved.clone()];
        form.selected_review = Some(saved.id.clone());
        form.pending_load = Some(saved.id.clone());
        let before_load = form.input.clone();
        for language in [Language::English, Language::Portuguese] {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                assert!(render(ui, &mut form, saved_revision, language).is_none());
            });
            out.textures_delta.clear();
            assert_eq!(form.input, before_load);
            assert_eq!(form.pending_load.as_deref(), Some(saved.id.as_str()));
        }
        let loaded = lib
            .load_playtime_award_review(&id, &saved.id, saved_revision, Role::Storyteller)
            .unwrap();
        assert_eq!(loaded.unclassified_awards, 1);
        assert_eq!(loaded.stale_reviews, 0);
        form.pending_load = None;
        form.input = loaded.input;
        form.load_awards(
            lib.playtime_awards(&id, "2026-01", "2026-01", saved_revision, Role::Storyteller)
                .unwrap(),
            saved_revision,
        );
        assert_eq!(form.input.class, CapClass::Unknown);
        assert!(
            !form.input.policy_scope_confirmed
                && !form.input.full_month_eligible_confirmed
                && !form.input.complete_regular_history_confirmed
        );
        assert!(form.input.home_chronicle.is_empty() && form.input.review_evidence.is_empty());
        assert_eq!(form.input.award_reviews.len(), 1);
        assert_eq!(
            lib.xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("12")
        );
    }
}
