use grapevine_workbench::{model::*, store::Library, xp::*};
use serde_json::json;
use std::fs;
use tempfile::TempDir;
fn baseline() -> Baseline {
    Baseline {
        day: "2026-01-01".into(),
        earned: "10".into(),
        unspent: "10".into(),
        actor: "Synthetic ST".into(),
        reason: "Declared opening".into(),
        evidence: "Synthetic record".into(),
    }
}
fn command(id: &str, kind: XpKind, amount: &str, day: &str) -> XpCommand {
    XpCommand {
        id: id.into(),
        kind,
        amount: amount.into(),
        day: day.into(),
        actor: "Synthetic ST".into(),
        reason: "Test event".into(),
        evidence: "Synthetic receipt".into(),
        reversal_of: None,
        cost: None,
    }
}
fn opened() -> (TempDir, Library, String) {
    let d = TempDir::new().unwrap();
    let mut l = Library::create(&d.path().join("xp.gvlib"), "XP").unwrap();
    let id = l
        .create_character("Wraith", "Synthetic", 0, Role::Storyteller)
        .unwrap();
    l.initialize_xp(&id, &baseline(), 1, Role::Storyteller)
        .unwrap();
    (d, l, id)
}
fn raw(l: &Library, id: &str) -> String {
    rusqlite::Connection::open(&l.path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .unwrap()
}
#[test]
fn exact_decimal_amounts_and_calendar_bounds() {
    assert_eq!(
        amount("0.1").unwrap() + amount("0.2").unwrap(),
        amount("0.3").unwrap()
    );
    assert_eq!(decimal(amount("-12.000001").unwrap()), "-12.000001");
    for bad in [
        "NaN",
        "inf",
        "1e3",
        "0.0000001",
        "1.",
        ".5",
        "--1",
        "99999999999",
        "",
        "٩",
    ] {
        assert!(amount(bad).is_err(), "{bad}");
    }
    assert!(day_valid("2024-02-29"));
    for d in [
        "2025-02-29",
        "2026-04-31",
        "0000-01-01",
        "2026-13-01",
        "26-01-01",
        "中文-01-01",
    ] {
        assert!(!day_valid(d));
    }
}
#[test]
fn exact_posting_idempotency_stale_edits_and_negative_rejection() {
    let (_d, mut l, id) = opened();
    let a = command("award", XpKind::Award, "0.1", "2026-01-02");
    l.post_xp(&id, &a, 2, Role::Storyteller).unwrap();
    let b = command("award2", XpKind::Award, "0.2", "2026-01-03");
    l.post_xp(&id, &b, 3, Role::Storyteller).unwrap();
    assert_eq!(
        l.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("10.3")
    );
    assert_eq!(l.post_xp(&id, &a, 2, Role::Storyteller).unwrap(), 4);
    let mut conflict = a.clone();
    conflict.amount = "1".into();
    assert_eq!(
        l.post_xp(&id, &conflict, 4, Role::Storyteller)
            .unwrap_err()
            .code,
        "XP_DUPLICATE_ID"
    );
    assert!(
        l.post_xp(
            &id,
            &command("stale", XpKind::Award, "1", "2026-01-04"),
            2,
            Role::Storyteller
        )
        .is_err()
    );
    let before = raw(&l, &id);
    assert!(
        l.post_xp(
            &id,
            &command("too-much", XpKind::Spend, "11", "2026-01-04"),
            4,
            Role::Storyteller
        )
        .is_err()
    );
    assert_eq!(raw(&l, &id), before);
    assert_eq!(l.revision().unwrap(), 4);
    assert!(
        l.post_xp(
            &id,
            &command("too-early", XpKind::Award, "1", "2025-12-31"),
            4,
            Role::Storyteller
        )
        .is_err()
    );
}
#[test]
fn backdated_spending_cannot_hide_a_negative_running_balance() {
    let (_d, mut l, id) = opened();
    l.post_xp(
        &id,
        &command("later-award", XpKind::Award, "10", "2026-02-01"),
        2,
        Role::Storyteller,
    )
    .unwrap();
    let before = raw(&l, &id);
    assert_eq!(
        l.post_xp(
            &id,
            &command("early-spend", XpKind::Spend, "15", "2026-01-02"),
            3,
            Role::Storyteller
        )
        .unwrap_err()
        .code,
        "XP_NEGATIVE_HISTORY"
    );
    assert_eq!(raw(&l, &id), before);
}
#[test]
fn append_only_reversals_preserve_originals_and_reject_double_or_unfunded_corrections() {
    let (_d, mut l, id) = opened();
    l.post_xp(
        &id,
        &command("purchase", XpKind::Spend, "8", "2026-01-02"),
        2,
        Role::Storyteller,
    )
    .unwrap();
    let mut reverse = command("reverse", XpKind::Reversal, "ignored", "2026-01-03");
    reverse.reversal_of = Some("purchase".into());
    l.post_xp(&id, &reverse, 3, Role::Storyteller).unwrap();
    let v = l.xp_view(&id, Role::Storyteller).unwrap();
    assert_eq!(v.unspent.as_deref(), Some("10"));
    assert_eq!(v.ledger.unwrap().entries.len(), 2);
    reverse.id = "double".into();
    assert_eq!(
        l.post_xp(&id, &reverse, 4, Role::Storyteller)
            .unwrap_err()
            .code,
        "XP_ALREADY_REVERSED"
    );
    let award = command("funding", XpKind::Award, "5", "2026-01-04");
    l.post_xp(&id, &award, 4, Role::Storyteller).unwrap();
    l.post_xp(
        &id,
        &command("spend-funding", XpKind::Spend, "14", "2026-01-05"),
        5,
        Role::Storyteller,
    )
    .unwrap();
    let mut r = command("unfunded", XpKind::Reversal, "", "2026-01-06");
    r.reversal_of = Some("funding".into());
    assert!(l.post_xp(&id, &r, 6, Role::Storyteller).is_err());
}
#[test]
fn cost_components_audit_missing_affiliation_surcharge_without_claiming_rules_authority() {
    let (_d, mut l, id) = opened();
    let mut c = command("declared-cost", XpKind::Spend, "3", "2026-01-02");
    c.cost = Some(CostEvidence {
        affiliation_name: "Synthetic affiliation".into(),
        power_family: "Synthetic power family".into(),
        approval_reference: "Synthetic approval fixture".into(),
        base: "3".into(),
        affiliation_adjustment: "1".into(),
        affiliation: "out".into(),
        source: "Fictional policy fixture".into(),
        edition: "Test".into(),
        page: "1".into(),
        policy_version: "test1".into(),
    });
    l.post_xp(&id, &c, 2, Role::Storyteller).unwrap();
    let v = l.xp_view(&id, Role::Storyteller).unwrap();
    assert!(v.findings.iter().any(|f| f.code == "COST_MISMATCH"));
    assert!(v.findings.iter().any(|f| f.code == "DECLARED_COST_POLICY"));
    let mut unknown = command("unknown-cost", XpKind::Spend, "1", "2026-01-03");
    unknown.cost = Some(CostEvidence {
        affiliation_name: "Synthetic affiliation".into(),
        power_family: "Synthetic power family".into(),
        approval_reference: "Synthetic approval fixture".into(),
        base: "1".into(),
        affiliation_adjustment: "0".into(),
        affiliation: "unknown".into(),
        source: "".into(),
        edition: "".into(),
        page: "".into(),
        policy_version: "".into(),
    });
    l.post_xp(&id, &unknown, 3, Role::Storyteller).unwrap();
    assert!(
        l.xp_view(&id, Role::Storyteller)
            .unwrap()
            .findings
            .iter()
            .any(|f| f.code == "AFFILIATION_COST_UNVERIFIED")
    );
}
#[test]
fn players_and_builds_cannot_post_or_read_private_audits() {
    let (_d, mut l, id) = opened();
    let before = raw(&l, &id);
    assert!(l.xp_view(&id, Role::Player).is_err());
    assert!(l.xp_roster_audit(Role::Player).is_err());
    assert!(
        l.post_xp(
            &id,
            &command("denied", XpKind::Award, "1", "2026-01-02"),
            2,
            Role::Player
        )
        .is_err()
    );
    let copy = l
        .create_build(&id, BuildKind::Test, 2, Role::Storyteller)
        .unwrap();
    assert!(
        l.initialize_xp(&copy, &baseline(), 3, Role::Storyteller)
            .is_err()
    );
    assert!(
        l.post_xp(
            &copy,
            &command("no-copy-spend", XpKind::Spend, "1", "2026-01-02"),
            3,
            Role::Storyteller
        )
        .is_err()
    );
    assert_eq!(raw(&l, &id), before);
    let report = l.xp_roster_audit(Role::Storyteller).unwrap();
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].record_id, id);
}
#[test]
fn failed_transaction_recovery_and_corrupt_ledger_are_not_silent() {
    let (d, mut l, id) = opened();
    let backup = d.path().join("xp.gvbackup");
    l.backup(&backup, Role::Storyteller).unwrap();
    let before = raw(&l, &id);
    let conn = rusqlite::Connection::open(&l.path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_xp BEFORE UPDATE ON records BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(
        l.post_xp(
            &id,
            &command("failed", XpKind::Award, "1", "2026-01-02"),
            2,
            Role::Storyteller
        )
        .is_err()
    );
    assert_eq!(raw(&l, &id), before);
    assert_eq!(l.revision().unwrap(), 2);
    conn.execute_batch("DROP TRIGGER fail_xp").unwrap();
    l.post_xp(
        &id,
        &command("valid", XpKind::Award, "1", "2026-01-02"),
        2,
        Role::Storyteller,
    )
    .unwrap();
    l.restore(&backup, 3, Role::Storyteller).unwrap();
    assert_eq!(raw(&l, &id), before);
    let mut record: Record = serde_json::from_str(&before).unwrap();
    let c = command("corrupt", XpKind::Spend, "11", "2026-01-02");
    record.xp_ledger.as_mut().unwrap().entries.push(Entry {
        command: c,
        recorded_unix_seconds: 0,
        origin_library_id: "fixture".into(),
    });
    let report = view(&record);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "XP_NEGATIVE_HISTORY")
    );
    assert!(report.unspent.is_none());
}
#[test]
fn legacy_replay_all_event_kinds_checks_negative_history_snapshots_and_container() {
    let (_d, l, id) = opened();
    let mut record: Record = serde_json::from_str(&raw(&l, &id)).unwrap();
    record.xp_ledger = None;
    let events = [
        (0, 10.0, 10.0, 10.0),
        (1, 1.0, 9.0, 9.0),
        (2, 12.0, 12.0, 9.0),
        (3, 3.0, 12.0, 6.0),
        (4, 1.0, 12.0, 7.0),
        (5, 2.0, 12.0, 2.0),
        (6, 0.0, 12.0, 2.0),
    ];
    let history=events.iter().enumerate().map(|(i,(k,c,e,u))|json!({"ChangeType":k,"Change":c,"Earned":e,"Unspent":u,"When":{"ole_automation_days":45000.0+i as f64},"Reason":"Synthetic"})).collect::<Vec<_>>();
    record.private_fields.insert(
        "Experience".into(),
        json!({"Earned":12.0,"Unspent":2.0,"History":history}),
    );
    let report = view(&record);
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == "LEGACY_SNAPSHOT_MISMATCH" || f.code == "LEGACY_CONTAINER_MISMATCH")
    );
    assert!(report.findings.iter().any(|f| f.code == "LEGACY_SET_TOTAL"));
    record.private_fields.get_mut("Experience").unwrap()["History"][3]["Unspent"] = json!(-1.0);
    assert!(
        view(&record)
            .findings
            .iter()
            .any(|f| f.code == "LEGACY_NEGATIVE_SNAPSHOT")
    );
    record.private_fields.get_mut("Experience").unwrap()["History"][0]["ChangeType"] = json!(99);
    let unknown = view(&record);
    assert!(
        unknown
            .findings
            .iter()
            .any(|f| f.code == "LEGACY_UNKNOWN_TYPE")
    );
    assert!(
        !unknown
            .findings
            .iter()
            .any(|f| f.code == "LEGACY_SNAPSHOT_MISMATCH")
    );
}
#[test]
fn schema2_migrates_with_backup_without_rewriting_records() {
    let (d, l, id) = opened();
    let path = l.path.clone();
    let record = raw(&l, &id);
    drop(l);
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch("DROP TRIGGER library_revision_guard_before_update; DROP TABLE purchase_write_guard; DROP TABLE purchase_receipts; DROP TABLE game_sessions; DROP TABLE scheduled_sessions;").unwrap();
    c.pragma_update(None, "user_version", 2).unwrap();
    drop(c);
    let l = Library::open(&path).unwrap();
    assert_eq!(l.revision().unwrap(), 9);
    assert_eq!(raw(&l, &id), record);
    assert!(
        fs::read_dir(d.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("pre-schema9-"))
    );
}

#[test]
fn summaries_omit_private_evidence_and_legacy_negative_final_totals_are_flagged() {
    let (_d, mut l, id) = opened();
    let mut c = command("secret", XpKind::Award, "1", "2026-01-02");
    c.actor = "CONTACT_SECRET".into();
    c.reason = "ST_NOTE_SECRET".into();
    c.evidence = "EVIDENCE_SECRET".into();
    l.post_xp(&id, &c, 2, Role::Storyteller).unwrap();
    assert!(l.xp_audit_export(Some(&id), Role::Player).is_err());
    let exported =
        serde_json::to_string(&l.xp_audit_export(Some(&id), Role::Storyteller).unwrap()).unwrap();
    for private in ["CONTACT_SECRET", "ST_NOTE_SECRET", "EVIDENCE_SECRET"] {
        assert!(!exported.contains(private));
    }
    let mut record: Record = serde_json::from_str(&raw(&l, &id)).unwrap();
    record.private_fields.insert(
        "Experience".into(),
        json!({"Earned":2,"Unspent":-1,"History":[]}),
    );
    assert!(
        view(&record)
            .findings
            .iter()
            .any(|f| f.code == "LEGACY_NEGATIVE_TOTALS")
    );
}

#[test]
fn truncated_legacy_opening_is_unverified_and_later_transitions_still_replay() {
    let (_d, l, id) = opened();
    let mut record: Record = serde_json::from_str(&raw(&l, &id)).unwrap();
    record.xp_ledger = None;
    record.private_fields.insert("Experience".into(),json!({"Earned":20,"Unspent":16,"History":[
        {"ChangeType":3,"Change":3,"Earned":20,"Unspent":17,"When":{"ole_automation_days":45000},"Reason":"Retained partial history"},
        {"ChangeType":3,"Change":1,"Earned":20,"Unspent":16,"When":{"ole_automation_days":45001},"Reason":"Later purchase"}
    ]}));
    let report = view(&record);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "LEGACY_OPENING_UNKNOWN")
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.code == "LEGACY_NEGATIVE_REPLAY" || f.code == "LEGACY_SNAPSHOT_MISMATCH")
    );
    assert_eq!(report.legacy_history[0].date, "2023-03-15 00:00:00");
    assert_eq!(legacy_date(25569.5), "1970-01-01 12:00:00");
    assert_eq!(legacy_date(0.0), "1899-12-30 00:00:00");
}
