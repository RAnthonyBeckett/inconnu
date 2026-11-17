use grapevine_workbench::{
    model::Language,
    operations::{self, CalendarLine, CalendarState, CharacterLine, Content, SignInLine, Snapshot},
};
fn label(i: usize) -> String {
    format!("Synthetic entry {i:03} <literal>")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new synthetic output directory")?,
    );
    std::fs::create_dir(&output)?;
    let variants = [
        (
            "roster",
            Content::Roster {
                family: String::new(),
                status: "Active".into(),
                include_players: false,
                rows: (1..=100)
                    .map(|i| CharacterLine {
                        name: label(i),
                        family: "Vampire".into(),
                        group: "Group".into(),
                        subgroup: "Circle".into(),
                        status: "Active".into(),
                        npc: false,
                        player: "PRIVATE_ASSOCIATION_NOTE".into(),
                    })
                    .collect(),
            },
        ),
        (
            "sign-in",
            Content::SignIn {
                day: "2026-11-10".into(),
                title: "Synthetic session".into(),
                include_players: false,
                missing: 0,
                rows: (1..=100)
                    .map(|i| SignInLine {
                        name: label(i),
                        player: "PRIVATE_ASSOCIATION_NOTE".into(),
                    })
                    .collect(),
            },
        ),
        (
            "calendar",
            Content::Calendar {
                from: "2026-11-01".into(),
                through: "2026-11-30".into(),
                include_cancelled: true,
                rows: (1..=100)
                    .map(|i| CalendarLine {
                        day: "2026-11-10".into(),
                        title: label(i),
                        state: CalendarState::Planned,
                        selected: 0,
                        attended: None,
                    })
                    .collect(),
            },
        ),
    ];
    for (family, content) in variants {
        let s = Snapshot {
            title: "Synthetic game Café <literal>".into(),
            revision: 1,
            content,
        };
        for (tag, lang) in [("en", Language::English), ("pt", Language::Portuguese)] {
            let html = operations::html(&s, lang)?;
            assert!(!html.contains("PRIVATE_ASSOCIATION_NOTE"));
            std::fs::write(output.join(format!("{family}-{tag}.html")), html)?;
        }
    }
    Ok(())
}
