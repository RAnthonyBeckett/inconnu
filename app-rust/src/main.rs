#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod playtime_ui;
mod points_ui;
mod print_ui;
mod purchase_ui;
mod scenario_ui;
mod session_ui;
mod sheet_ui;
mod ui;
mod xp_ui;

use std::{env, error::Error, path::PathBuf};

use grapevine_workbench::{diagnostics, store::Library};
use uuid::Uuid;

const WINDOW_TITLE: &str = "Inconnu — Beta Edition";

struct Arguments {
    library: Option<PathBuf>,
    ui_smoke_dir: Option<PathBuf>,
}

fn main() {
    diagnostics::install_panic_hook();
    if let Err(error) = run() {
        diagnostics::record("startup", "START_FAILED");
        eprintln!("Inconnu could not start: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments = parse_arguments()?;
    let mut native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!(
                "../../assets/branding/inconnu-icon.png"
            ))?)
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([760.0, 540.0]),
        ..Default::default()
    };

    if let Some(output_root) = arguments.ui_smoke_dir {
        native_options.viewport = native_options.viewport.with_active(false);
        if arguments.library.is_some() {
            return Err("--ui-smoke cannot be combined with --library".into());
        }
        let run_dir = output_root.join(format!("ui-smoke-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&run_dir)?;
        let library_path = run_dir.join("smoke-library.gvlib");
        let library = Library::create(&library_path, "UI Smoke Library")?;
        std::fs::write(run_dir.join("smoke-source.gex"), br#"<?xml version="1.0"?><grapevine version="3"><vampire name="Smoke Character" id="smoke1" status="Draft" clan="Original clan" willpower="5" tempwillpower="3"><notes>SMOKE_ONLY_SECRET</notes><traitlist name="Disciplines"><trait name="Smoke Power" val="Original Level" note="SMOKE_TRAIT_SECRET"/></traitlist></vampire></grapevine>"#)?;
        drop(library);

        let result_path = run_dir.join("ui-smoke.json");
        eframe::run_native(
            WINDOW_TITLE,
            native_options,
            Box::new(move |creation_context| {
                if let Some(scale) = env::var("GRAPEVINE_SMOKE_SCALE")
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|s| s.is_finite() && *s >= 0.75 && *s <= 2.0)
                {
                    creation_context.egui_ctx.set_zoom_factor(scale);
                }
                Ok(Box::new(ui::InconnuApp::new_smoke(library_path, run_dir)))
            }),
        )?;
        let result: serde_json::Value = serde_json::from_slice(&std::fs::read(result_path)?)?;
        if result["ok"] != true {
            return Err("Native UI verification failed; see ui-smoke.json".into());
        }
        return Ok(());
    }

    let initial_library = arguments.library;
    eframe::run_native(
        WINDOW_TITLE,
        native_options,
        Box::new(move |_creation_context| {
            let history = env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join("Inconnu").join("recent-libraries.json"));
            Ok(Box::new(ui::InconnuApp::new_with_recent_history(
                initial_library,
                history,
            )))
        }),
    )?;
    Ok(())
}

fn parse_arguments() -> Result<Arguments, Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let mut library = None;
    let mut ui_smoke_dir = None;
    while let Some(argument) = args.next() {
        if argument == "--library" {
            let Some(path) = args.next() else {
                return Err("--library requires a path".into());
            };
            library = Some(PathBuf::from(path));
        } else if argument == "--ui-smoke" {
            let Some(path) = args.next() else {
                return Err("--ui-smoke requires an output directory".into());
            };
            ui_smoke_dir = Some(PathBuf::from(path));
        } else if argument == "--verify-import" {
            return Err(
                "--verify-import is reserved and is not implemented by the UI executable".into(),
            );
        } else {
            return Err(format!("unknown argument: {}", argument.to_string_lossy()).into());
        }
    }
    Ok(Arguments {
        library,
        ui_smoke_dir,
    })
}
