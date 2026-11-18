//! Explicit current-editor fields to legacy story data, never audit/approval evidence.
use crate::{
    actions,
    model::{Record, Result},
    plots, rumors,
};
use serde_json::{Map, Value, json};

pub(super) fn apply(record: &Record, fields: &mut Map<String, Value>) -> Result<()> {
    if record.kind == "Action" && record.private_fields.contains_key(actions::FIELD) {
        let (action, _, _) = actions::read(record)?;
        fields.insert("ActDate".into(), json!(action.day));
        fields.insert("CharName".into(), json!(action.character));
        fields.insert("Done".into(), json!(action.is_complete()));
        fields.insert("subaction".into(),json!(action.slots.iter().map(|s|json!({"Name":s.name,"Level":s.level,"Unused":s.unused,"Total":s.total,"Growth":s.growth,"act":s.action,"results":s.result})).collect::<Vec<_>>()));
    }
    if record.kind == "Plot" && record.private_fields.contains_key(plots::FIELD) {
        let (plot, _, _) = plots::read(record)?;
        fields.insert("Name".into(), json!(plot.name));
        fields.insert("Narrator".into(), json!(plot.narrator));
        fields.insert("Outline".into(), json!(plot.outline));
        fields.insert("StartDate".into(), json!(plot.start));
        fields.insert("EndDate".into(), json!(plot.end));
        fields.insert("CastList".into(),json!({"Name":"Key Characters","Traits":plot.cast.iter().map(|c|json!({"Name":c.label,"Value":c.value,"Note":c.note})).collect::<Vec<_>>()}));
        fields.insert(
            "development".into(),
            json!(
                plot.developments
                    .iter()
                    .map(|d| json!({"date":d.day,"event":d.event}))
                    .collect::<Vec<_>>()
            ),
        );
    }
    if record.kind == "Rumor" && record.private_fields.contains_key(rumors::FIELD) {
        let (rumor, _, _) = rumors::read(record)?;
        fields.insert("Title".into(), json!(rumor.title));
        fields.insert("RumorDate".into(), json!(rumor.day));
        fields.insert("Category".into(), json!(rumor.category));
        fields.insert("MultiKey".into(), json!(rumor.influence_list));
        fields.insert("MultiMatch".into(), json!(rumor.influence_trait));
        fields.insert("Done".into(), json!(rumor.reviewed));
        fields.insert(
            "subrumor".into(),
            json!(
                rumor
                    .entries
                    .iter()
                    .map(|e| json!({"level":e.level,"body":e.body}))
                    .collect::<Vec<_>>()
            ),
        );
    }
    Ok(())
}
