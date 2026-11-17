use grapevine_workbench::{
    model::{BuildInfo, Record},
    playtime::{self, CapClass, Request},
    xp::{Baseline, Entry, Ledger, XpCommand, XpKind},
};
use serde_json::{Map, json};

fn baseline(day: &str, earned: &str, unspent: &str) -> Baseline {
    Baseline {
        day: day.into(),
        earned: earned.into(),
        unspent: unspent.into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic opening".into(),
        evidence: "Synthetic fixture".into(),
    }
}

fn entry(id: &str, kind: XpKind, amount: &str, day: &str, reversal_of: Option<&str>) -> Entry {
    Entry {
        command: XpCommand {
            id: id.into(),
            kind,
            amount: amount.into(),
            day: day.into(),
            actor: "Synthetic ST".into(),
            reason: "Synthetic XP test entry".into(),
            evidence: "Synthetic fixture".into(),
            reversal_of: reversal_of.map(str::to_owned),
            cost: None,
        },
        recorded_unix_seconds: 0,
        origin_library_id: "synthetic-library".into(),
    }
}

fn record(kind: &str, ledger: Option<Ledger>) -> Record {
    Record {
        id: "synthetic-record".into(),
        kind: kind.into(),
        name: "Synthetic character".into(),
        player_id: None,
        legacy_id: None,
        public_fields: Map::new(),
        private_fields: Map::new(),
        original: json!({"synthetic":true}),
        source_sha256: String::new(),
        st_markers: None,
        build: None,
        xp_ledger: ledger,
    }
}

fn confirmed_request(start: &str, end: &str, class: CapClass) -> Request {
    Request {
        start_month: start.into(),
        end_month: end.into(),
        class,
        policy_scope_confirmed: true,
        complete_regular_history_confirmed: true,
        full_month_eligible_confirmed: true,
        home_chronicle: "Synthetic home chronicle evidence".into(),
        review_evidence: "Synthetic review evidence".into(),
        ..Default::default()
    }
}

fn simple_ledger(day: &str, earned: &str, unspent: &str, entries: Vec<Entry>) -> Ledger {
    Ledger {
        baseline: baseline(day, earned, unspent),
        entries,
    }
}

#[test]
fn first_day_opening_excludes_same_day_awards_across_year_boundary() {
    let character = record(
        "Vampire",
        Some(simple_ledger(
            "2025-12-01",
            "200",
            "200",
            vec![entry("jan-award", XpKind::Award, "1", "2026-01-01", None)],
        )),
    );
    let before = serde_json::to_value(&character).unwrap();
    let report = assess_explicitly_reviewed_fixture(
        &character,
        &confirmed_request("2025-12", "2026-02", CapClass::Standard),
    )
    .unwrap();

    assert_eq!(report.source_url, playtime::SOURCE_URL);
    assert_eq!(report.policy_version, "owbn-character-2026-11-07");
    assert_eq!(report.status, "WithinDeclaredCeiling");
    assert_eq!(report.months[0].opening_earned.as_deref(), Some("200"));
    assert_eq!(report.months[0].ceiling.as_deref(), Some("12"));
    assert_eq!(report.months[1].opening_earned.as_deref(), Some("200"));
    assert_eq!(report.months[1].regular_awarded, "1");
    assert_eq!(report.months[1].ceiling.as_deref(), Some("12"));
    assert_eq!(report.months[2].opening_earned.as_deref(), Some("201"));
    assert_eq!(report.months[2].ceiling.as_deref(), Some("10"));
    assert_eq!(serde_json::to_value(&character).unwrap(), before);
}

#[test]
fn standard_cap_boundaries_are_exact_and_fractional_gaps_are_insufficient() {
    for (earned, expected_cap, expected_status) in [
        ("200", Some("12"), "WithinDeclaredCeiling"),
        ("200.000001", None, "InsufficientEvidence"),
        ("201", Some("10"), "WithinDeclaredCeiling"),
        ("400", Some("10"), "WithinDeclaredCeiling"),
        ("400.000001", None, "InsufficientEvidence"),
        ("401", Some("8"), "WithinDeclaredCeiling"),
    ] {
        let character = record(
            "Vampire",
            Some(simple_ledger("2026-03-01", earned, "0", vec![])),
        );
        let report = assess_explicitly_reviewed_fixture(
            &character,
            &confirmed_request("2026-03", "2026-03", CapClass::Standard),
        )
        .unwrap();
        assert_eq!(
            report.months[0].ceiling.as_deref(),
            expected_cap,
            "{earned}"
        );
        assert_eq!(report.months[0].status, expected_status, "{earned}");
    }
}

#[test]
fn wraith_uses_fixed_eight_point_ceiling() {
    let character = record(
        "Wraith",
        Some(simple_ledger(
            "2026-03-01",
            "200.5",
            "0",
            vec![
                entry("regular-1", XpKind::Award, "8", "2026-03-05", None),
                entry("regular-2", XpKind::Award, "1", "2026-03-20", None),
            ],
        )),
    );
    let report = assess_explicitly_reviewed_fixture(
        &character,
        &confirmed_request("2026-03", "2026-03", CapClass::Wraith),
    )
    .unwrap();
    assert_eq!(report.months[0].opening_earned.as_deref(), Some("200.5"));
    assert_eq!(report.months[0].regular_awarded, "9");
    assert_eq!(report.months[0].ceiling.as_deref(), Some("8"));
    assert_eq!(report.months[0].status, "ExceedsDeclaredCeiling");

    let mismatch = assess_explicitly_reviewed_fixture(
        &character,
        &confirmed_request("2026-03", "2026-03", CapClass::Standard),
    )
    .unwrap_err();
    assert_eq!(mismatch.code, "PLAYTIME_CLASS");

    let no_ledger = assess_explicitly_reviewed_fixture(
        &record("Wraith", None),
        &confirmed_request("2026-03", "2026-03", CapClass::Wraith),
    )
    .unwrap();
    assert_eq!(no_ledger.months[0].ceiling.as_deref(), Some("8"));
    assert_eq!(no_ledger.months[0].status, "InsufficientEvidence");

    let invalid_ledger = record(
        "Wraith",
        Some(simple_ledger(
            "2026-03-01",
            "10",
            "10",
            vec![entry(
                "bad-award",
                XpKind::Award,
                "invalid",
                "2026-03-02",
                None,
            )],
        )),
    );
    let invalid = assess_explicitly_reviewed_fixture(
        &invalid_ledger,
        &confirmed_request("2026-03", "2026-03", CapClass::Wraith),
    )
    .unwrap();
    assert_eq!(invalid.months[0].ceiling.as_deref(), Some("8"));
    assert_eq!(invalid.months[0].status, "InsufficientEvidence");
}

#[test]
fn spending_does_not_lower_earned_xp_opening_band() {
    let character = record(
        "Mage",
        Some(simple_ledger(
            "2026-01-01",
            "201",
            "10",
            vec![entry("spent", XpKind::Spend, "5", "2026-01-28", None)],
        )),
    );
    let report = assess_explicitly_reviewed_fixture(
        &character,
        &confirmed_request("2026-02", "2026-02", CapClass::Standard),
    )
    .unwrap();
    assert_eq!(report.months[0].opening_earned.as_deref(), Some("201"));
    assert_eq!(report.months[0].regular_awarded, "0");
    assert_eq!(report.months[0].ceiling.as_deref(), Some("10"));
    assert_eq!(report.months[0].status, "WithinDeclaredCeiling");
}

#[test]
fn adjustments_and_award_reversals_make_affected_month_insufficient() {
    let award_reversed = record(
        "Vampire",
        Some(simple_ledger(
            "2026-01-01",
            "100",
            "10",
            vec![
                entry("award", XpKind::Award, "1", "2026-01-05", None),
                entry(
                    "reverse-award",
                    XpKind::Reversal,
                    "1",
                    "2026-01-25",
                    Some("award"),
                ),
            ],
        )),
    );
    let reversed_report = assess_explicitly_reviewed_fixture(
        &award_reversed,
        &confirmed_request("2026-01", "2026-01", CapClass::Standard),
    )
    .unwrap();
    assert_eq!(reversed_report.months[0].regular_awarded, "1");
    assert_eq!(reversed_report.months[0].status, "InsufficientEvidence");

    let adjusted = record(
        "Vampire",
        Some(simple_ledger(
            "2026-01-01",
            "100",
            "10",
            vec![entry("adjust", XpKind::Adjustment, "1", "2026-01-05", None)],
        )),
    );
    let adjusted_report = assess_explicitly_reviewed_fixture(
        &adjusted,
        &confirmed_request("2026-01", "2026-01", CapClass::Standard),
    )
    .unwrap();
    assert_eq!(adjusted_report.months[0].status, "InsufficientEvidence");
}

#[test]
fn standard_award_total_above_declared_cap_is_reported() {
    let character = record(
        "Vampire",
        Some(simple_ledger(
            "2026-01-01",
            "200",
            "0",
            vec![entry(
                "many-awards",
                XpKind::Award,
                "13",
                "2026-01-15",
                None,
            )],
        )),
    );
    let report = assess_explicitly_reviewed_fixture(
        &character,
        &confirmed_request("2026-01", "2026-01", CapClass::Standard),
    )
    .unwrap();
    assert_eq!(report.months[0].ceiling.as_deref(), Some("12"));
    assert_eq!(report.months[0].regular_awarded, "13");
    assert_eq!(report.status, "ExceedsDeclaredCeiling");
}

#[test]
fn missing_ledger_partial_opening_and_unconfirmed_evidence_are_not_clean() {
    let no_ledger = record("Vampire", None);
    let missing = assess_explicitly_reviewed_fixture(
        &no_ledger,
        &confirmed_request("2026-01", "2026-01", CapClass::Standard),
    )
    .unwrap();
    assert_eq!(missing.status, "InsufficientEvidence");
    assert_eq!(missing.months[0].opening_earned, None);
    assert_eq!(missing.months[0].regular_awarded, "unknown");

    let partial = record(
        "Vampire",
        Some(simple_ledger("2026-01-15", "10", "10", vec![])),
    );
    let partial_report = assess_explicitly_reviewed_fixture(
        &partial,
        &confirmed_request("2026-01", "2026-02", CapClass::Standard),
    )
    .unwrap();
    assert_eq!(partial_report.months[0].opening_earned, None);
    assert_eq!(partial_report.months[0].status, "InsufficientEvidence");
    assert_eq!(
        partial_report.months[1].opening_earned.as_deref(),
        Some("10")
    );
    assert_eq!(partial_report.months[1].status, "WithinDeclaredCeiling");

    let mut unconfirmed = confirmed_request("2026-01", "2026-01", CapClass::Standard);
    unconfirmed.policy_scope_confirmed = false;
    unconfirmed.complete_regular_history_confirmed = false;
    unconfirmed.full_month_eligible_confirmed = false;
    unconfirmed.home_chronicle.clear();
    unconfirmed.review_evidence.clear();
    let report = assess_explicitly_reviewed_fixture(
        &record(
            "Vampire",
            Some(simple_ledger("2026-01-01", "10", "10", vec![])),
        ),
        &unconfirmed,
    )
    .unwrap();
    assert_eq!(report.months[0].status, "InsufficientEvidence");
    assert!(report.months[0].issues.len() >= 5);
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("Synthetic")
    );
}

#[test]
fn invalid_ledger_range_and_build_records_are_rejected_or_insufficient() {
    let broken = record(
        "Vampire",
        Some(simple_ledger(
            "2026-01-01",
            "10",
            "10",
            vec![entry(
                "bad-award",
                XpKind::Award,
                "not-a-number",
                "2026-01-02",
                None,
            )],
        )),
    );
    let report = assess_explicitly_reviewed_fixture(
        &broken,
        &confirmed_request("2026-01", "2026-01", CapClass::Standard),
    )
    .unwrap();
    assert_eq!(report.months[0].status, "InsufficientEvidence");
    assert_eq!(report.months[0].opening_earned, None);
    assert_eq!(report.months[0].regular_awarded, "unknown");

    let mut build = record(
        "Vampire",
        Some(simple_ledger("2026-01-01", "10", "10", vec![])),
    );
    build.build = Some(BuildInfo {
        source_id: "synthetic-source".into(),
        source_revision: 1,
        kind: grapevine_workbench::model::BuildKind::Draft,
    });
    assert_eq!(
        assess_explicitly_reviewed_fixture(
            &build,
            &confirmed_request("2026-01", "2026-01", CapClass::Standard)
        )
        .unwrap_err()
        .code,
        "PLAYTIME_SCOPE"
    );

    let mut too_long = confirmed_request("0001-01", "9999-12", CapClass::Standard);
    too_long.policy_scope_confirmed = true;
    assert_eq!(
        assess_explicitly_reviewed_fixture(&record("Vampire", None), &too_long)
            .unwrap_err()
            .code,
        "PLAYTIME_RANGE_LIMIT"
    );

    let entries = (0..2001)
        .map(|index| {
            entry(
                &format!("entry-{index}"),
                XpKind::Award,
                "1",
                "2000-01-01",
                None,
            )
        })
        .collect();
    let work_limited = record(
        "Vampire",
        Some(simple_ledger("2000-01-01", "10", "10", entries)),
    );
    assert_eq!(
        assess_explicitly_reviewed_fixture(
            &work_limited,
            &confirmed_request("2000-01", "2083-04", CapClass::Standard)
        )
        .unwrap_err()
        .code,
        "PLAYTIME_WORK_LIMIT"
    );
}

#[test]
fn reversal_target_lookup_work_is_bounded_before_replay() {
    let mut entries = Vec::new();
    for i in 0..1001 {
        let id = format!("award-{i}");
        entries.push(entry(&id, XpKind::Award, "1", "2026-01-01", None));
        entries.push(entry(
            &format!("reverse-{i}"),
            XpKind::Reversal,
            "1",
            "2026-01-02",
            Some(&id),
        ));
    }
    let character = record(
        "Vampire",
        Some(simple_ledger("2026-01-01", "10", "10", entries)),
    );
    assert_eq!(
        assess_explicitly_reviewed_fixture(
            &character,
            &confirmed_request("2026-01", "2026-01", CapClass::Standard)
        )
        .unwrap_err()
        .code,
        "PLAYTIME_WORK_LIMIT"
    );
}

#[test]
fn risen_and_mixed_templates_do_not_inherit_a_cap_by_assumption() {
    for kind in ["Risen", "Various"] {
        for class in [CapClass::Standard, CapClass::Wraith] {
            let character = record(kind, Some(simple_ledger("2026-01-01", "10", "10", vec![])));
            let report = assess_explicitly_reviewed_fixture(
                &character,
                &confirmed_request("2026-01", "2026-01", class),
            )
            .unwrap();
            assert_eq!(report.status, "InsufficientEvidence");
            assert!(
                report.months[0]
                    .issues
                    .iter()
                    .any(|s| s.contains("mixed-template"))
            );
        }
    }
}

// Existing date/band/reversal cases now declare their synthetic regular awards explicitly.
// New default/unclassified behavior is tested without this helper in xp_award_classification.
fn assess_explicitly_reviewed_fixture(
    record: &Record,
    input: &Request,
) -> grapevine_workbench::model::Result<playtime::Report> {
    let mut reviewed = input.clone();
    if let Ok(rows) = playtime::awards(record, &input.start_month, &input.end_month) {
        reviewed.award_reviews = rows
            .into_iter()
            .map(|r| playtime::AwardReview {
                entry_id: r.entry_id,
                entry_hash: r.entry_hash,
                class: playtime::AwardClass::Regular,
                reason: "Synthetic explicit regular classification".into(),
                reference: "Synthetic classification evidence".into(),
            })
            .collect();
    }
    playtime::assess(record, &reviewed)
}
