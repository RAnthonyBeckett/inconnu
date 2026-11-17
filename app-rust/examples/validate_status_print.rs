use grapevine_workbench::{
    model::Language,
    searches::{Mode, Query, Row},
    vampire_status::{self, Boon, Character, Snapshot, Trait},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let rows: Vec<_> = (1..=100)
        .map(|i| Row {
            id: format!("INTERNAL_CHARACTER_ID_{i}"),
            name: format!("Synthetic entry {i:03} Café <literal>"),
            family: "Vampire".into(),
            status: "Active".into(),
            group: String::new(),
            subgroup: String::new(),
            player: "PRIVATE_PLAYER".into(),
            npc: None,
        })
        .collect();
    let characters = rows
        .iter()
        .map(|row| Character {
            name: row.name.clone(),
            title: String::new(),
            group: String::new(),
            statuses: vec![Trait {
                name: "Synthetic status".into(),
                value: "Recorded <literal>".into(),
            }],
            unreadable_status: false,
            boons: vec![Boon {
                kind: "Synthetic minor".into(),
                partner: "Synthetic partner".into(),
                owed_by_holder: Some(true),
                day: Some("2026-01-01".into()),
                description: "PRIVATE_CAST_NOTE".into(),
            }],
        })
        .collect();
    let snapshot = Snapshot {
        search: grapevine_workbench::searches::Snapshot {
            title: "Synthetic Café <literal>".into(),
            revision: 24,
            query: Query {
                mode: Mode::All,
                clauses: Vec::new(),
            },
            include_players: false,
            rows,
        },
        include_boons: true,
        include_descriptions: false,
        characters,
    };
    for (language, suffix) in [(Language::English, "en"), (Language::Portuguese, "pt")] {
        let html = vampire_status::html(&snapshot, language)?;
        for secret in [
            "PRIVATE_CAST_NOTE",
            "INTERNAL_CHARACTER_ID",
            "PRIVATE_PLAYER",
        ] {
            assert!(!html.contains(secret));
        }
        std::fs::write(output.join(format!("vampire-status-{suffix}.html")), html)?;
    }
    println!("PASS synthetic Vampire Status HTML");
    Ok(())
}
