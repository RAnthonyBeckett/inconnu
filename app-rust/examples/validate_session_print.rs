//! Synthetic end-to-end session output, never reads a user library.
use grapevine_workbench::{
    game_sessions::*,
    model::{Language, Role},
    store::Library,
    xp::Baseline,
};
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output folder"));
    std::fs::create_dir_all(&out).unwrap();
    let dir = tempfile::TempDir::new().unwrap();
    let mut lib = Library::create(
        &dir.path().join("synthetic.gvlib"),
        "Synthetic session print",
    )
    .unwrap();
    let mut attendance = Vec::new();
    for i in 1..=100 {
        let id = lib
            .create_character(
                if i % 2 == 0 { "Risen" } else { "Vampire" },
                &format!("Érica — Synthetic attendee {i:03} & <literal>"),
                lib.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap();
        let present = i % 3 != 0;
        if present {
            lib.initialize_xp(
                &id,
                &Baseline {
                    day: "2026-01-01".into(),
                    earned: "10".into(),
                    unspent: "10".into(),
                    actor: "PRIVATE_ACTOR".into(),
                    reason: "PRIVATE_REASON".into(),
                    evidence: "PRIVATE_EVIDENCE".into(),
                },
                lib.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap();
        }
        attendance.push(Attendance {
            record_id: id,
            present,
            award: present.then(|| "2".into()),
        });
    }
    let request = SessionRequest {
        id: uuid::Uuid::new_v4().to_string(),
        title: "Synthetic 100-row attendance".into(),
        day: "2026-01-02".into(),
        actor: "PRIVATE_ACTOR".into(),
        reason: "PRIVATE_REASON".into(),
        evidence: "PRIVATE_EVIDENCE".into(),
        attendance,
    };
    let quote = lib
        .quote_session(&request, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    lib.commit_session(&quote, Role::Storyteller).unwrap();
    for (name, language) in [
        ("session-en", Language::English),
        ("session-pt", Language::Portuguese),
    ] {
        let html = lib
            .session_report(&request.id, language, Role::Storyteller)
            .unwrap();
        assert!(!html.contains("PRIVATE_"));
        grapevine_workbench::safe_files::write_new(
            &out.join(format!("{name}.html")),
            html.as_bytes(),
        )
        .unwrap();
    }
    println!(
        "Synthetic 100-row session reports saved; 67 awards/33 absent, private evidence omitted."
    );
}
