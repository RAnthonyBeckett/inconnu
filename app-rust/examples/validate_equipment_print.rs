use grapevine_workbench::{
    associations::{self, Kind, Legacy, Link, Resolved, Snapshot},
    model::Language,
    world::Edit,
};
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new synthetic output directory")?,
    );
    std::fs::create_dir(&output)?;
    let snapshot = Snapshot {
        character_id: "INTERNAL_CHARACTER_ID".into(),
        character_name: "Synthetic character Café <literal>".into(),
        revision: 1,
        linked: (1..=100)
            .map(|i| {
                let title = format!("Synthetic entry {i:03} <literal>");
                Resolved {
                    link: Link {
                        target_id: format!("INTERNAL_TARGET_ID_{i}"),
                        kind: Kind::Item,
                        label_at_binding: title.clone(),
                        quantity: 2,
                        note: "PRIVATE_ASSOCIATION_NOTE".into(),
                    },
                    current: Some(Edit {
                        name: title,
                        fields: std::collections::BTreeMap::from([
                            ("Appearance".into(), json!("Sample visible description")),
                            ("Notes".into(), json!("PRIVATE_ASSOCIATION_NOTE")),
                            ("RAW_DATA".into(), json!("PRIVATE_ASSOCIATION_NOTE")),
                        ]),
                    }),
                }
            })
            .collect(),
        legacy: vec![Legacy {
            kind: Kind::Item,
            label: "Unresolved example".into(),
            value: "3".into(),
            note: "PRIVATE_ASSOCIATION_NOTE".into(),
        }],
        retained: 0,
    };
    for (tag, lang) in [("en", Language::English), ("pt", Language::Portuguese)] {
        let html = associations::html(&snapshot, lang, false)?;
        assert!(
            html.contains("Unresolved example")
                && !html.contains("INTERNAL_")
                && !html.contains("PRIVATE_ASSOCIATION_NOTE")
        );
        std::fs::write(output.join(format!("equipment-{tag}.html")), html)?;
    }
    Ok(())
}
