//! Synthetic, isolated browser-print proof input. Never reads a user's library.
use grapevine_workbench::{model::*, printing::*, purchase_preview::AnimalismPower, safe_files};
use serde_json::json;
fn main() {
    let output = std::path::PathBuf::from(std::env::args().nth(1).expect("output folder"));
    std::fs::create_dir_all(&output).unwrap();
    let rows:Vec<_>=(1..=90).map(|i|json!({"Name":format!("Synthetic ability {i:02} — ação & <literal>"),"Value":"Descriptive trait","Note":"SYNTHETIC_PRIVATE_NOTE"})).collect();
    let view=RecordView {id:"synthetic-print".into(),kind:"Vampire".into(),name:"Érica — Synthetic print proof".into(),status:"Not approved".into(),player_name:"Synthetic owner".into(),notes:"SYNTHETIC_PRIVATE_NOTE".into(),build:None,
        fields:json!({"Name":"Érica — Synthetic print proof","Status":"Not approved","AbilityList":{"Traits":rows},"DisciplineList":{"Traits":[{"Name":"Heightened Senses","Value":"Basic"}]},"Willpower":7,"TempWillpower":2}).as_object().unwrap().clone()};
    for (name, options) in [
        ("normal-en", Options::default()),
        (
            "large-pt",
            Options {
                language: DocLanguage::Portuguese,
                large_text: true,
                ..Options::default()
            },
        ),
    ] {
        let doc = document(
            &view,
            &[AnimalismPower::HeightenedSenses.stable_id().into()],
            &options,
        )
        .unwrap();
        let html = html(&doc).unwrap();
        assert!(!html.contains("SYNTHETIC_PRIVATE_NOTE"));
        safe_files::write_new(&output.join(format!("{name}.html")), html.as_bytes()).unwrap();
    }
    println!("Synthetic print proof inputs saved.");
}
