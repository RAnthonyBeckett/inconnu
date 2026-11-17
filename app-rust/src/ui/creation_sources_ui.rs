use super::*;
use grapevine_workbench::{
    creation_sources as sources, published_powers as powers, published_reference as source,
};
use serde_json::{Value, json};
#[derive(Default)]
pub(super) struct Picker {
    row: usize,
    pub(super) query: String,
    book_id: Option<String>,
}
fn t(pt: bool, en: &str, pt_text: &str) -> String {
    if pt { pt_text.into() } else { en.into() }
}
pub(super) fn render(
    ui: &mut egui::Ui,
    edit: &mut SheetEdit,
    state: &mut Picker,
    language: Language,
) -> bool {
    let pt = language == Language::Portuguese;
    let mut changed = false;
    egui::CollapsingHeader::new(t(pt,"Published names for initial powers","Nomes publicados para poderes iniciais"))
        .id_salt("creation_source_picker").default_open(!state.query.is_empty()).show(ui,|ui| {
        ui.label(t(pt,"This catalogue is partial. A published level never assigns a MET tier. Add or choose an initial row, select its published name, then review its MET eligibility separately.","Este catálogo é parcial. Um nível publicado nunca define o grau MET. Adicione ou escolha uma linha inicial, selecione o nome publicado e revise separadamente sua elegibilidade MET."));
        let Some(list)=edit.lists.get_mut("DisciplineList") else {return;};
        let Some(rows)=list.get_mut("Traits").and_then(Value::as_array_mut) else {return;};
        if rows.len()<4096 && ui.button(t(pt,"Add a MET Basic candidate · review required","Adicionar candidato MET Básico · revisão obrigatória")).clicked() {
            rows.push(json!({"Name":"","Value":"basic","Note":""}));state.row=rows.len()-1;changed=true;
        }
        if rows.is_empty() {return;}
        state.row=state.row.min(rows.len()-1);
        egui::ComboBox::from_id_salt("creation_source_row").selected_text(format!("{} · {}",state.row+1,rows[state.row]["Name"].as_str().unwrap_or(""))).show_ui(ui,|ui| {
            for (i,row) in rows.iter().enumerate() {ui.selectable_value(&mut state.row,i,format!("{} · {}",i+1,row["Name"].as_str().unwrap_or("")));}
        });
        ui.horizontal(|ui| {
            ui.label(t(pt,"Search published names","Buscar nomes publicados"));ui.text_edit_singleline(&mut state.query);
            egui::ComboBox::from_id_salt("creation_source_book").selected_text(match state.book_id.as_deref() {Some("v20-core")=>"V20",Some("da20-core")=>"DA20",_=>"V20 + DA20"}).show_ui(ui,|ui| {
                ui.selectable_value(&mut state.book_id,None,"V20 + DA20");
                ui.selectable_value(&mut state.book_id,Some("v20-core".into()),"V20");
                ui.selectable_value(&mut state.book_id,Some("da20-core".into()),"DA20");
            });
        });
        while state.query.len()>1000 {state.query.pop();}
        let Ok(catalogue)=powers::catalogue() else {ui.label(t(pt,"Catalogue needs inspection.","O catálogo precisa de revisão."));return;};
        let results=catalogue.search(state.book_id.as_deref(),&state.query);
        ui.small(t(pt,"Showing at most 30 results; narrow the search to see others.","Exibindo até 30 resultados; refine a busca para ver os demais."));
        egui::ScrollArea::vertical().id_salt("creation_source_results").max_height(160.0).show(ui,|ui| {
            for (index,power) in results.into_iter().take(30).enumerate() {
                ui.push_id(&power.id,|ui| {
                    let label=format!("{} · {} · {:?} {} · p.{} / PDF{}",power.name,power.discipline,catalogue.book(&power.book_id).map(|b|b.edition).unwrap_or(powers::Edition::V20),power.level,power.printed_page.map_or_else(||"?".into(),|p|p.to_string()),power.pdf_page);
                    let response=ui.button(label);
                    if index==0 {
                        let rect=ui.clip_rect().contains_rect(response.rect).then_some(response.rect);
                        ui.ctx().data_mut(|data|data.insert_temp(egui::Id::unique("creation_source_result"),rect));
                    }
                    if response.clicked() && sources::choose(&mut rows[state.row],&power.id).is_ok() {changed=true;}
                });
            }
        });
        let row=&mut rows[state.row];
        let original_met=row[sources::PROPOSED]["met_power_id"].as_str().or_else(||row[sources::MET_FIELD]["power_id"].as_str()).map(str::to_owned);
        let mut met=original_met.clone();
        let selected=met.as_deref().and_then(grapevine_workbench::power_catalogue::lookup).map(|p|p.name).unwrap_or_else(||t(pt,"No reviewed MET reminder mapping","Sem vínculo MET revisado para lembrete"));
        ui.label(t(pt,"Optional MET mechanics reminder: choose the actual MET Basic power separately from its published level.","Lembrete opcional de regras MET: escolha o poder MET Básico separadamente do nível publicado."));
        egui::ComboBox::from_id_salt("creation_met_mapping").selected_text(selected).show_ui(ui,|ui| {
            ui.selectable_value(&mut met,None,t(pt,"No mapping","Sem vínculo"));
            for p in grapevine_workbench::power_catalogue::entries().into_iter().filter(|p|p.tier==grapevine_workbench::pricing::MetTier::Basic) {
                ui.selectable_value(&mut met,Some(p.stable_id),format!("{} · {}",p.family,p.name));
            }
        });
        if met!=original_met {
            match sources::choose_met(row,met.as_deref()) {Ok(())=>changed=true,Err(error)=>{ui.colored_label(egui::Color32::YELLOW,error.message);}}
        }
        if let Some(choice)=row.get(sources::PROPOSED).cloned() {
            let source_id=choice["source_id"].as_str().unwrap_or("");
            if let Ok(reference)=catalogue.reference(source_id) {
                ui.label(format!("{} · {} · {:?} · p.{} / PDF{}",reference.power.name,reference.book_title,reference.edition,reference.power.printed_page.map_or_else(||"?".into(),|p|p.to_string()),reference.power.pdf_page));
                if reference.power.evidence==powers::Evidence::IndexOnly {
                    ui.colored_label(egui::Color32::YELLOW,t(pt,"Index-only evidence: check your library before confirming this source.","Evidência apenas do índice: consulte sua biblioteca antes de confirmar esta fonte."));
                }
            }
            if let Ok(signature)=sources::review_signature(row) {
                let mut reviewed=choice["reviewed_signature"].as_str()==Some(signature.as_str());
                if ui.checkbox(&mut reviewed,t(pt,"I reviewed this source, any alias, MET Basic eligibility, prerequisites and starting progression; the published level is separate.","Revisei esta fonte, eventuais nomes alternativos, elegibilidade MET Básica, pré-requisitos e progressão inicial; o nível publicado é separado.")).changed() {
                    if reviewed {row[sources::PROPOSED]["reviewed_signature"]=json!(signature);}
                    else if let Some(object)=row[sources::PROPOSED].as_object_mut() {object.remove("reviewed_signature");}
                    changed=true;
                }
            }
            if ui.button(t(pt,"Clear proposed source · keep the trait name","Remover fonte proposta · manter nome da característica")).clicked() {
                if let Some(object)=row.as_object_mut() {object.remove(sources::PROPOSED);}changed=true;
            }
        } else if let Some(reference)=source::verified_reference(row) {
            ui.label(format!("{}: {} · {:?} · PDF{}",t(pt,"Saved source","Fonte salva"),reference.power.name,reference.edition,reference.power.pdf_page));
        }
    });
    changed
}
