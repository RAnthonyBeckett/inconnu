use grapevine_workbench::{
    action_links::{Timeline, TimelineRow},
    action_timeline_report,
    actions::{Action, ActivitySlot},
    model::Language,
    story_work::{Draft, Snapshot},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let timeline = Timeline {
        revision: 25,
        character_id: "INTERNAL_CHARACTER_ID".into(),
        character_name: "Synthetic Actor Café <literal>".into(),
        from: "2026-01-01".into(),
        through: "2026-12-31".into(),
        undated_omitted: 2,
        unlinked_in_library: 3,
        rows: (1..=100)
            .map(|i| TimelineRow {
                action_id: format!("INTERNAL_TARGET_ID_{i}"),
                snapshot: Snapshot {
                    revision: 25,
                    legacy: false,
                    retained: false,
                    draft: Draft::Action(Action {
                        character: "Synthetic Actor Café <literal>".into(),
                        day: "2026-01-01".into(),
                        slots: vec![ActivitySlot {
                            name: format!("Synthetic entry {i:03} Café <literal>"),
                            level: "2".into(),
                            unused: "0".into(),
                            total: "4".into(),
                            growth: "0".into(),
                            action: "Recorded request".into(),
                            result: "Recorded result".into(),
                        }],
                    }),
                },
            })
            .collect(),
    };
    for (language, suffix) in [(Language::English, "en"), (Language::Portuguese, "pt")] {
        let html = action_timeline_report::html(&timeline, language)?;
        for secret in ["INTERNAL_CHARACTER_ID", "INTERNAL_TARGET_ID"] {
            assert!(!html.contains(secret));
        }
        std::fs::write(output.join(format!("action-timeline-{suffix}.html")), html)?;
    }
    println!("PASS synthetic character Action timeline HTML");
    Ok(())
}
