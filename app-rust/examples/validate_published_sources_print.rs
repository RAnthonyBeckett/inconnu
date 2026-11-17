//! Entirely synthetic layout/privacy stress input; never modifies a library or catalogue.
use grapevine_workbench::{
    model::RecordView,
    printing::{self, DocLanguage, Options, PrintScope, PublishedReference},
    published_powers::{Edition, Evidence, Kind, Power, Reference},
    safe_files,
};
use serde_json::Map;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&out)?;
    for (lang, language, label) in [
        ("en", DocLanguage::English, "Private Storyteller"),
        ("pt", DocLanguage::Portuguese, "Relatório privado"),
    ] {
        let view=RecordView{build:None,id:"SYNTHETIC_ONLY".into(),kind:"Vampire".into(),name:format!("{label} · synthetic source-reference layout"),status:"Synthetic".into(),player_name:String::new(),notes:"PRIVATE_CAST_NOTE PRIVATE_ASSOCIATION_NOTE INTERNAL_TARGET_ID INTERNAL_CHARACTER_ID".into(),fields:Map::new()};
        let mut document = printing::document(
            &view,
            &[],
            &Options {
                language,
                scope: PrintScope::PrivateStoryteller,
                large_text: true,
                include_notes: false,
                ..Default::default()
            },
        )?;
        for i in 1..=100 {
            let source = Reference {
                power: Power {
                    id: format!("synthetic:power:{i}"),
                    book_id: "synthetic".into(),
                    name: format!("Synthetic entry {i:03} <literal> Café"),
                    discipline: "Synthetic source family".into(),
                    path: Some("Synthetic branch".into()),
                    kind: Kind::Path,
                    level: 5,
                    printed_page: Some(129),
                    pdf_page: 138,
                    evidence: Evidence::PageText,
                },
                book_id: "synthetic".into(),
                book_title: "Synthetic source <literal> Café".into(),
                edition: Edition::V20,
                pdf_sha256: "a".repeat(64),
                facts_hash: "b".repeat(64),
            };
            document.published_references.push(PublishedReference {
                recorded_name: format!("Recorded trait {i:03}"),
                recorded_value: "advanced".into(),
                source,
            });
        }
        let html = printing::html(&document)?;
        assert!(html.contains("&lt;literal&gt;"));
        assert!(!html.contains("PRIVATE_CAST_NOTE"));
        safe_files::write_new(
            &out.join(format!("published-sources-{lang}.html")),
            html.as_bytes(),
        )?;
    }
    println!(
        "PASS synthetic100published-reference entries in EN/PT, literal/Unicode/privacy input"
    );
    Ok(())
}
