use grapevine_workbench::{
    model::Language,
    searches::{Mode, Query, Row, Snapshot as SearchSnapshot},
    trait_distribution::{self, Request},
    trait_distribution_report::{self, Snapshot},
    trait_statistics::{Character, Entry},
};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let request = Request {
        list: "AbilityList".into(),
        name: "Synthetic trait Café <literal>".into(),
    };
    let mut rows = Vec::new();
    let mut characters = Vec::new();
    for i in 1..=100 {
        let name = format!("Synthetic entry {i:03} Café <literal>");
        rows.push(Row {
            id: format!("INTERNAL_CHARACTER_ID_{i}"),
            name: name.clone(),
            family: "Vampire".into(),
            status: "Active".into(),
            group: String::new(),
            subgroup: String::new(),
            player: "PRIVATE_PLAYER_SECRET".into(),
            npc: Some(false),
        });
        let mut entries = vec![Entry {
            name: request.name.clone(),
            value: if i % 5 == 0 {
                "descriptive".into()
            } else {
                (i % 4).to_string()
            },
        }];
        if i == 1 {
            entries.push(Entry {
                name: request.name.clone(),
                value: "02".into(),
            });
            entries.push(Entry {
                name: request.name.clone(),
                value: "2".into(),
            });
        }
        characters.push(Character {
            name,
            family: "Vampire".into(),
            lists: BTreeMap::from([("AbilityList".into(), entries)]),
        });
    }
    let snapshot = Snapshot {
        search: SearchSnapshot {
            title: "Synthetic distribution".into(),
            revision: 28,
            query: Query {
                mode: Mode::All,
                clauses: Vec::new(),
            },
            include_players: false,
            rows,
        },
        report: trait_distribution::build(&characters, &request)?,
        request,
        omitted_lists: 0,
        unnamed_entries: 0,
    };
    for (language, suffix) in [(Language::English, "en"), (Language::Portuguese, "pt")] {
        let html = trait_distribution_report::html(&snapshot, language)?;
        assert!(!html.contains("PRIVATE_PLAYER_SECRET"));
        assert!(!html.contains("INTERNAL_CHARACTER_ID"));
        std::fs::write(output.join(format!("distribution-{suffix}.html")), html)?;
    }
    println!("PASS synthetic named-trait distribution with100characters");
    Ok(())
}
