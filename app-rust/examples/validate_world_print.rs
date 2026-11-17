use grapevine_workbench::{model::Language, world, world_card};
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Supply new synthetic output directory")?,
    );
    std::fs::create_dir(&folder)?;
    for kind in ["Item", "Location", "Rote"] {
        let mut edit = world::Edit {
            name: format!("Synthetic {kind} Café <literal>"),
            ..Default::default()
        };
        for field in world::text_fields(kind) {
            edit.fields.insert(
                (*field).into(),
                json!(if *field == "Notes" {
                    "PRIVATE_SECRET"
                } else {
                    "Synthetic descriptive text <literal>"
                }),
            );
        }
        let rows:Vec<_>=(1..=100).map(|i|json!({"Name":format!("Synthetic trait {i:03}"),"Value":"2","Note":"PRIVATE_ROW_SECRET"})).collect();
        edit.fields
            .insert(world::list_fields(kind)[0].into(), json!({"Traits":rows}));
        for (lang, language) in [("en", Language::English), ("pt", Language::Portuguese)] {
            std::fs::write(
                folder.join(format!("{kind}-{lang}.html")),
                world_card::html(kind, &edit, language, false)?,
            )?;
        }
    }
    println!("Six synthetic EN/PT cards written; no real game data.");
    Ok(())
}
