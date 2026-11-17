use grapevine_workbench::{ledger_reports, model::Language, player_points as pp, xp};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide new output directory")?,
    );
    std::fs::create_dir(&output)?;
    let mut xp_ledger = xp::Ledger {
        baseline: xp::Baseline {
            day: "2026-01-01".into(),
            earned: "0".into(),
            unspent: "0".into(),
            actor: "SECRET_ACTOR".into(),
            reason: "Synthetic opening".into(),
            evidence: "SECRET_EVIDENCE".into(),
        },
        entries: vec![],
    };
    let mut points = pp::open(pp::Baseline {
        day: "2026-01-01".into(),
        balance: "0".into(),
        actor: "SECRET_ACTOR".into(),
        reason: "Synthetic opening".into(),
        evidence: "SECRET_EVIDENCE".into(),
    })?;
    for i in 1..=100 {
        let reason = format!("Synthetic entry {i:03} Café <literal>");
        let c = xp::XpCommand {
            id: format!("award-{i}"),
            kind: xp::XpKind::Award,
            amount: "0.000001".into(),
            day: "2026-01-02".into(),
            actor: "SECRET_ACTOR".into(),
            reason: reason.clone(),
            evidence: "SECRET_EVIDENCE".into(),
            reversal_of: None,
            cost: None,
        };
        xp_ledger.entries.push(xp::Entry {
            command: c,
            recorded_unix_seconds: 1,
            origin_library_id: "INTERNAL_ORIGIN".into(),
        });
        points = pp::apply(
            &points,
            &pp::Command {
                id: uuid::Uuid::new_v4().to_string(),
                kind: pp::Kind::Award,
                amount: "0.000002".into(),
                day: "2026-01-02".into(),
                actor: "SECRET_ACTOR".into(),
                reason,
                evidence: "SECRET_EVIDENCE".into(),
                reversal_of: None,
            },
        )?;
    }
    let view = xp::XpView {
        record_id: "INTERNAL_RECORD".into(),
        record_name: "Character Café <literal>".into(),
        is_build: false,
        ledger: Some(xp_ledger),
        earned: Some("0.0001".into()),
        unspent: Some("0.0001".into()),
        legacy_history_count: 0,
        legacy_history: vec![],
        legacy_earned: None,
        legacy_unspent: None,
        findings: vec![],
        coverage: String::new(),
    };
    let xp = ledger_reports::character(&view, 1, "2026-01-01", "2026-01-31", true)?;
    let player = ledger_reports::player(
        "Player Café <literal>",
        Some(&points),
        &xp::XpView {
            ledger: None,
            ..view
        },
        1,
        "2026-01-01",
        "2026-01-31",
        true,
    )?;
    for (family, h) in [("xp-history", xp), ("points-history", player)] {
        for (tag, language) in [("en", Language::English), ("pt", Language::Portuguese)] {
            let html = ledger_reports::html(&h, language)?;
            assert!(!html.contains("SECRET_") && !html.contains("INTERNAL_"));
            std::fs::write(output.join(format!("{family}-{tag}.html")), html)?;
        }
    }
    let roster = grapevine_workbench::operations::Snapshot {
        title: "Synthetic roster Café <literal>".into(),
        revision: 1,
        content: grapevine_workbench::operations::Content::PlayerRoster {
            status: String::new(),
            include_contacts: false,
            include_legacy_ids: false,
            rows: (1..=100)
                .map(|i| grapevine_workbench::operations::PlayerLine {
                    name: format!("Synthetic entry {i:03} Café <literal>"),
                    position: "Volunteer".into(),
                    status: "Active".into(),
                    legacy_id: "INTERNAL_RECORD".into(),
                    email: "PRIVATE_ASSOCIATION_NOTE".into(),
                    phone: "PRIVATE_ASSOCIATION_NOTE".into(),
                })
                .collect(),
        },
    };
    for (tag, language) in [("en", Language::English), ("pt", Language::Portuguese)] {
        let html = grapevine_workbench::operations::html(&roster, language)?;
        assert!(!html.contains("PRIVATE_ASSOCIATION_NOTE") && !html.contains("INTERNAL_RECORD"));
        std::fs::write(output.join(format!("player-roster-{tag}.html")), html)?;
    }
    Ok(())
}
