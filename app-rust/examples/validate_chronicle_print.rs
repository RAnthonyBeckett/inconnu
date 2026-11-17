use grapevine_workbench::{
    chronicle::{Count, Handover, Profile, Source},
    chronicle_handover,
    model::Language,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let handover = Handover {
        revision: 26,
        profile: Profile {
            title: "Synthetic Chronicle Café <literal>".into(),
            description: "Local saved game information".into(),
            house_rules: "Synthetic staff-entered house rules\nSecond line".into(),
            ..Default::default()
        },
        include_contacts: false,
        include_notes: false,
        include_source_names: false,
        counts: vec![
            Count {
                key: "characters_total".into(),
                count: 100,
            },
            Count {
                key: "characters_without_local_xp_opening".into(),
                count: 4,
            },
        ],
        sources: (1..=100)
            .map(|i| Source {
                hash: format!("synthetic-evidence-{i:03}"),
                format: format!("Synthetic entry {i:03} Café <literal>"),
                name: String::new(),
            })
            .collect(),
    };
    for (language, suffix) in [(Language::English, "en"), (Language::Portuguese, "pt")] {
        let html = chronicle_handover::html(&handover, language)?;
        assert!(!html.contains("Saved drafts only"));
        std::fs::write(
            output.join(format!("chronicle-handover-{suffix}.html")),
            html,
        )?;
    }
    println!("PASS synthetic private Chronicle handover HTML");
    Ok(())
}
