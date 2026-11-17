use grapevine_workbench::{
    model::{Edit, Language, Role},
    rumor_packet,
    rumors::{Entry, Rumor},
    store::Library,
    story_work::Draft,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide a new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let mut library = Library::create(
        &output.join("synthetic-packet.gvlib"),
        "Synthetic audience proof",
    )?;
    let mut ids = Vec::new();
    let mut revision = 0;
    for i in 1..=100 {
        ids.push(library.create_character(
            "Vampire",
            &format!("Synthetic entry {i:03} Café <literal>"),
            revision,
            Role::Storyteller,
        )?);
        revision += 1;
    }
    library.save_edit(
        &ids[0],
        &Edit {
            name: "Synthetic entry 001 Café <literal>".into(),
            status: "Active".into(),
            notes: "PRIVATE_CAST_NOTE".into(),
        },
        revision,
        Role::Storyteller,
    )?;
    revision += 1;
    let rumor = library.create_story(
        &Draft::Rumor(Rumor {
            title: "Synthetic audience Café <literal>".into(),
            day: "2026-11-11".into(),
            audience_label: "Staff-selected synthetic list".into(),
            reviewed: true,
            entries: vec![Entry {
                level: "0".into(),
                body: "Synthetic saved rumor text <literal>".into(),
            }],
            ..Default::default()
        }),
        revision,
        Role::Storyteller,
    )?;
    revision += 1;
    let preview = library.preview_rumor_audience(&rumor, &ids, revision, Role::Storyteller)?;
    assert_eq!(library.revision()?, revision);
    library.commit_rumor_audience(&preview, Role::Storyteller)?;
    let snapshot = library.rumor_audience_snapshot(&rumor, Role::Storyteller)?;
    for (language, suffix) in [(Language::English, "en"), (Language::Portuguese, "pt")] {
        let html = rumor_packet::html(&snapshot, language)?;
        assert!(!html.contains("PRIVATE_CAST_NOTE"));
        assert!(ids.iter().all(|id| !html.contains(id)));
        std::fs::write(output.join(format!("rumor-audience-{suffix}.html")), html)?;
    }
    println!("PASS private saved Rumor packet with100 synthetic recipients");
    Ok(())
}
