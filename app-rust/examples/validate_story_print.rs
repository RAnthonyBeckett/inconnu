use grapevine_workbench::{
    actions::{Action, ActivitySlot},
    model::Language,
    rumors::{Entry, Rumor},
    story_work::{self, Draft, Kind, Master, Snapshot},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide a new synthetic output directory")?,
    );
    std::fs::create_dir(&folder)?;
    for kind in [Kind::Action, Kind::Rumor] {
        let records = (0..25)
            .map(|i| {
                let day = format!("2026-11-{:02}", i + 1);
                let draft = match kind {
                    Kind::Action => Draft::Action(Action {
                        character: format!("Synthetic actor {i:02} Café <literal>"),
                        day,
                        slots: (1..=4)
                            .map(|j| ActivitySlot {
                                name: "Research".into(),
                                action: format!("Synthetic entry {:03} <literal>", i * 4 + j),
                                result: "A synthetic result.".into(),
                                ..Default::default()
                            })
                            .collect(),
                    }),
                    Kind::Rumor => Draft::Rumor(Rumor {
                        title: format!("Synthetic rumor {i:02} Café <literal>"),
                        day,
                        category: 1,
                        audience_label: "Declared synthetic audience".into(),
                        entries: (1..=4)
                            .map(|j| Entry {
                                level: j.to_string(),
                                body: format!("Synthetic entry {:03} <literal>", i * 4 + j),
                            })
                            .collect(),
                        ..Default::default()
                    }),
                };
                Snapshot {
                    revision: 1,
                    draft,
                    legacy: false,
                    retained: false,
                }
            })
            .collect();
        let master = Master {
            revision: 1,
            kind,
            from: "2026-11-01".into(),
            through: "2026-11-30".into(),
            incomplete_only: false,
            undated_omitted: 0,
            records,
        };
        for (lang, language) in [("en", Language::English), ("pt", Language::Portuguese)] {
            std::fs::write(
                folder.join(format!(
                    "{}-{lang}.html",
                    kind.record_kind().to_ascii_lowercase()
                )),
                story_work::master_html(&master, language)?,
            )?;
        }
    }
    Ok(())
}
