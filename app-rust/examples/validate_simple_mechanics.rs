//! Source-qualified layout/privacy stress fixture, not a character grant or XP approval.
use grapevine_workbench::{
    model::Role,
    power_catalogue,
    printing::{self, DocLanguage, Options},
    safe_files, sheet,
    store::Library,
};
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("fresh output directory required")?,
    );
    std::fs::create_dir(&out)?;
    let mut library = Library::create(
        &out.join("synthetic-mechanics-layout.gvlib"),
        "Synthetic source-mapped print fixture",
    )?;
    let id =
        library.create_character("Vampire", "Mechanics <literal> Café", 0, Role::Storyteller)?;
    let mut edit = library.sheet_edit(&id, Role::Storyteller)?;
    let mut list = sheet::empty_list("DisciplineList");
    list["Traits"]=json!(power_catalogue::entries().iter().map(|power|json!({"Name":power.name,"Value":power.tier.sheet_value(),"Note":"SYNTHETIC_PRIVATE_MECHANICS_NOTE","AppSourcePowerId":power.stable_id,"AppAvailableFrom":"2031-12-31"})).collect::<Vec<_>>());
    edit.lists.insert("DisciplineList".into(), list);
    edit.visibility.insert("DisciplineList".into(), true);
    library.save_sheet(&id, &edit, library.revision()?, Role::Storyteller)?;
    let mut results = vec![];
    for (label, language) in [
        ("en", DocLanguage::English),
        ("pt", DocLanguage::Portuguese),
    ] {
        let enabled = Options {
            language,
            include_simple_mechanics: true,
            include_references: false,
            large_text: label == "pt",
            ..Default::default()
        };
        let doc = library.print_document(&id, &enabled, library.revision()?, Role::Player)?;
        let prefix = if label == "en" {
            "Simple mechanics:"
        } else {
            "Regras resumidas:"
        };
        assert_eq!(
            doc.sections
                .iter()
                .filter(|s| s.title.starts_with(prefix))
                .count(),
            40
        );
        let html = printing::html(&doc)?;
        assert!(html.contains("2031-12-31"));
        assert!(html.contains("&lt;literal&gt;"));
        assert!(!html.contains("SYNTHETIC_PRIVATE_MECHANICS_NOTE"));
        for entry in power_catalogue::entries() {
            assert!(doc.sections.iter().any(|s| s.title.ends_with(&entry.name)));
        }
        safe_files::write_new(
            &out.join(format!("mechanics-{label}.html")),
            html.as_bytes(),
        )?;
        let disabled = library.print_document(
            &id,
            &Options {
                language,
                ..Default::default()
            },
            library.revision()?,
            Role::Player,
        )?;
        assert!(
            !disabled
                .sections
                .iter()
                .any(|s| s.title.starts_with(prefix))
        );
        results.push(json!({"language":label,"cards":40,"separate_references_disabled":true,"private_notes_omitted":true,"availability_preserved":true,"literal_html_escaped":true,"old_default_print_unchanged":true}));
    }
    safe_files::write_new(
        &out.join("mechanics-layout-results.json"),
        &serde_json::to_vec_pretty(&results)?,
    )?;
    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}
