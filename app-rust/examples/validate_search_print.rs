use grapevine_workbench::{
    model::Language,
    searches::{self, Mode, Query, Row, Snapshot},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let snapshot = Snapshot {
        title: "Synthetic Café <literal>".into(),
        revision: 12,
        query: Query {
            mode: Mode::All,
            clauses: Vec::new(),
        },
        include_players: false,
        rows: (1..=100)
            .map(|i| Row {
                id: format!("INTERNAL_CHARACTER_ID_{i}"),
                name: format!("Synthetic entry {i:03} Café <literal>"),
                family: if i % 2 == 0 {
                    "Wraith".into()
                } else {
                    "Vampire".into()
                },
                status: "Active".into(),
                group: format!("Synthetic entry {i:03} Café <literal>"),
                subgroup: String::new(),
                player: "PRIVATE_CAST_NOTE".into(),
                npc: if i % 3 == 0 { None } else { Some(i % 2 == 0) },
            })
            .collect(),
    };
    for (language, suffix) in [(Language::English, "en"), (Language::Portuguese, "pt")] {
        for (statistics, family) in [(false, "search"), (true, "statistics")] {
            let html = searches::html(&snapshot, language, statistics)?;
            for secret in ["INTERNAL_CHARACTER_ID", "PRIVATE_CAST_NOTE"] {
                assert!(!html.contains(secret));
            }
            std::fs::write(output.join(format!("{family}-{suffix}.html")), html)?;
        }
    }
    println!("PASS synthetic search/statistics HTML");
    Ok(())
}
