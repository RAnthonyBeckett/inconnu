use grapevine_workbench::{
    model::Language,
    plots::{self, Cast, Development, Plot, Snapshot},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide a new synthetic output directory")?,
    );
    std::fs::create_dir(&output)?;
    let p = Snapshot {
        revision: 1,
        legacy: false,
        retained: false,
        plot: Plot {
            name: "Synthetic plot Café <literal>".into(),
            start: "2026-10-01".into(),
            end: "2026-11-01".into(),
            outline: "Synthetic outline & context.".into(),
            cast: vec![Cast {
                label: "Witness".into(),
                value: "Guest".into(),
                note: "PRIVATE_CAST_NOTE".into(),
            }],
            developments: (1..=100)
                .rev()
                .map(|i| Development {
                    day: format!("2026-10-{:02}", (i - 1) % 31 + 1),
                    event: format!("Synthetic event {i:03} — Café <literal>"),
                })
                .collect(),
            ..Default::default()
        },
    };
    for (lang, language) in [("en", Language::English), ("pt", Language::Portuguese)] {
        let html = plots::html(&p, "2026-10-28", language, false)?;
        std::fs::write(output.join(format!("plot-{lang}.html")), html)?;
    }
    Ok(())
}
