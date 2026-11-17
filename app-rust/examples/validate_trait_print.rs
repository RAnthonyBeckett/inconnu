use grapevine_workbench::{
    model::Language,
    searches::{Mode, Query, Row},
    trait_reports::{self, Snapshot},
    trait_statistics::{self, Character, Entry, Kind},
};
use std::collections::BTreeMap;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let characters: Vec<_> = (1..=100)
        .map(|i| Character {
            name: format!("Synthetic entry {i:03} Café <literal>"),
            family: "Vampire".into(),
            lists: BTreeMap::from([
                (
                    "InfluenceList".into(),
                    vec![Entry {
                        name: "Police".into(),
                        value: if i == 50 {
                            "descriptive".into()
                        } else {
                            "2".into()
                        },
                    }],
                ),
                (
                    "MeritList".into(),
                    vec![Entry {
                        name: "Synthetic Merit".into(),
                        value: "2".into(),
                    }],
                ),
                (
                    "FlawList".into(),
                    vec![Entry {
                        name: "Synthetic Flaw".into(),
                        value: "descriptive".into(),
                    }],
                ),
            ]),
        })
        .collect();
    for (kind, family) in [
        (Kind::Influence, "influence"),
        (Kind::MeritsFlaws, "merits-flaws"),
    ] {
        let search = grapevine_workbench::searches::Snapshot {
            title: "Synthetic Café <literal>".into(),
            revision: 15,
            query: Query {
                mode: Mode::All,
                clauses: Vec::new(),
            },
            include_players: false,
            rows: characters
                .iter()
                .map(|c| Row {
                    id: "INTERNAL_CHARACTER_ID".into(),
                    name: c.name.clone(),
                    family: c.family.clone(),
                    status: String::new(),
                    group: String::new(),
                    subgroup: String::new(),
                    player: "PRIVATE_CAST_NOTE".into(),
                    npc: None,
                })
                .collect(),
        };
        let snapshot = Snapshot {
            search,
            kind,
            report: trait_statistics::build(&characters, kind)?,
            omitted_lists: 0,
            unnamed_entries: 0,
        };
        for (language, suffix) in [(Language::English, "en"), (Language::Portuguese, "pt")] {
            let html = trait_reports::html(&snapshot, language)?;
            for secret in ["INTERNAL_CHARACTER_ID", "PRIVATE_CAST_NOTE"] {
                assert!(!html.contains(secret));
            }
            std::fs::write(output.join(format!("{family}-{suffix}.html")), html)?;
        }
    }
    println!("PASS synthetic trait reports");
    Ok(())
}
