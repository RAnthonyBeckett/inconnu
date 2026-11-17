use grapevine_workbench::{
    action_links::{Timeline, TimelineRow},
    actions::{Action, ActivitySlot},
    combined_story::{Request, RumorRow, Snapshot},
    combined_story_report,
    model::Language,
    rumors::{Entry, Rumor},
    story_work::{Draft, Snapshot as StorySnapshot},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let mut snapshot = Snapshot {
        request: Request {
            character_id: "INTERNAL_CHARACTER_ID".into(),
            from: "2026-11-01".into(),
            through: "2026-11-30".into(),
        },
        actions: Timeline {
            revision: 30,
            character_id: "INTERNAL_CHARACTER_ID".into(),
            character_name: "Synthetic Café <literal>".into(),
            from: "2026-11-01".into(),
            through: "2026-11-30".into(),
            rows: Vec::new(),
            undated_omitted: 1,
            unlinked_in_library: 2,
        },
        rumors: Vec::new(),
        undated_rumors: 3,
        unassigned_rumors_in_range: 4,
    };
    for i in 1..=100 {
        let day = format!("2026-11-{:02}", (i % 28) + 1);
        let name = format!("Synthetic entry {i:03} Café <literal>");
        if i % 2 == 1 {
            snapshot.actions.rows.push(TimelineRow {
                action_id: format!("INTERNAL_TARGET_ID_{i}"),
                snapshot: StorySnapshot {
                    revision: 30,
                    legacy: false,
                    retained: false,
                    draft: Draft::Action(Action {
                        character: snapshot.actions.character_name.clone(),
                        day,
                        slots: vec![ActivitySlot {
                            name,
                            level: "1".into(),
                            unused: "0".into(),
                            total: "1".into(),
                            growth: "0".into(),
                            action: "Saved request".into(),
                            result: "Saved result".into(),
                        }],
                    }),
                },
            });
        } else {
            snapshot.rumors.push(RumorRow {
                record_id: format!("INTERNAL_TARGET_ID_{i}"),
                snapshot: StorySnapshot {
                    revision: 30,
                    legacy: false,
                    retained: false,
                    draft: Draft::Rumor(Rumor {
                        title: name,
                        day,
                        entries: vec![Entry {
                            level: "0".into(),
                            body: "Saved staff-review rumor".into(),
                        }],
                        ..Default::default()
                    }),
                },
            });
        }
    }
    snapshot.actions.rows.reverse();
    snapshot.rumors.reverse();
    for (language, suffix) in [(Language::English, "en"), (Language::Portuguese, "pt")] {
        let html = combined_story_report::html(&snapshot, language)?;
        assert!(!html.contains("INTERNAL_CHARACTER_ID") && !html.contains("INTERNAL_TARGET_ID"));
        std::fs::write(output.join(format!("combined-story-{suffix}.html")), html)?;
    }
    println!("PASS combined private timeline with100 synthetic entries");
    Ok(())
}
