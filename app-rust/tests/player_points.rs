use grapevine_workbench::player_points::*;

fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn baseline(balance: &str) -> Baseline {
    Baseline {
        day: "2026-01-01".into(),
        balance: balance.into(),
        actor: "Synthetic operator".into(),
        reason: "Declared opening".into(),
        evidence: "Synthetic ledger opening".into(),
    }
}

fn command(n: u64, kind: Kind, amount: &str, day: &str) -> Command {
    Command {
        id: id(n),
        kind,
        amount: amount.into(),
        day: day.into(),
        actor: "Synthetic operator".into(),
        reason: "Test entry".into(),
        evidence: "Synthetic receipt".into(),
        reversal_of: None,
    }
}

#[test]
fn exact_fractional_award_spend_and_adjustment() {
    let ledger = open(baseline("1.25")).unwrap();
    let ledger = apply(&ledger, &command(1, Kind::Award, "0.1", "2026-01-02")).unwrap();
    let ledger = apply(&ledger, &command(2, Kind::Spend, "0.15", "2026-01-03")).unwrap();
    let ledger = apply(&ledger, &command(3, Kind::Adjustment, "-0.2", "2026-01-04")).unwrap();
    assert_eq!(replay(&ledger).unwrap(), "1");
}

#[test]
fn overspend_and_backdated_negative_history_are_rejected_without_mutating_input() {
    let ledger = open(baseline("1")).unwrap();
    let with_later_award = apply(&ledger, &command(1, Kind::Award, "5", "2026-01-10")).unwrap();
    let before = with_later_award.clone();
    assert!(
        apply(
            &with_later_award,
            &command(2, Kind::Spend, "2", "2026-01-02"),
        )
        .is_err()
    );
    assert_eq!(with_later_award, before);
    assert!(
        apply(
            &with_later_award,
            &command(3, Kind::Spend, "7", "2026-01-11"),
        )
        .is_err()
    );
    assert_eq!(replay(&with_later_award).unwrap(), "6");
}

#[test]
fn same_id_retry_is_idempotent_and_conflicting_reuse_is_rejected() {
    let ledger = open(baseline("0")).unwrap();
    let award = command(1, Kind::Award, "0.25", "2026-01-02");
    let posted = apply(&ledger, &award).unwrap();
    assert_eq!(apply(&posted, &award).unwrap(), posted);
    let mut conflict = award;
    conflict.amount = "0.5".into();
    assert_eq!(
        apply(&posted, &conflict).unwrap_err().code,
        "PLAYER_POINTS_COMMAND_CONFLICT"
    );
    assert_eq!(replay(&posted).unwrap(), "0.25");
}

#[test]
fn reversal_stores_signed_delta_and_blocks_duplicate_or_reversal_targets() {
    let ledger = open(baseline("10")).unwrap();
    let spend = command(1, Kind::Spend, "2", "2026-01-02");
    let posted = apply(&ledger, &spend).unwrap();
    let mut reversal = command(2, Kind::Reversal, "", "2026-01-03");
    reversal.reversal_of = Some(spend.id.clone());
    let reversed = apply(&posted, &reversal).unwrap();
    assert_eq!(reversed.entries[1].amount, "2");
    assert_eq!(replay(&reversed).unwrap(), "10");
    assert_eq!(apply(&reversed, &reversal).unwrap(), reversed);

    let mut duplicate = command(3, Kind::Reversal, "", "2026-01-04");
    duplicate.reversal_of = Some(spend.id);
    assert_eq!(
        apply(&reversed, &duplicate).unwrap_err().code,
        "PLAYER_POINTS_ALREADY_REVERSED"
    );
    let mut reversal_of_reversal = command(4, Kind::Reversal, "", "2026-01-04");
    reversal_of_reversal.reversal_of = Some(reversed.entries[1].id.clone());
    assert!(apply(&reversed, &reversal_of_reversal).is_err());
}

#[test]
fn reversal_of_award_records_negative_balance_delta() {
    let ledger = open(baseline("1")).unwrap();
    let award = command(1, Kind::Award, "0.25", "2026-01-02");
    let posted = apply(&ledger, &award).unwrap();
    let mut reversal = command(2, Kind::Reversal, "", "2026-01-03");
    reversal.reversal_of = Some(award.id);
    let reversed = apply(&posted, &reversal).unwrap();
    assert_eq!(reversed.entries[1].amount, "-0.25");
    assert_eq!(replay(&reversed).unwrap(), "1");
}

#[test]
fn opening_and_command_fields_are_bounded_and_validated() {
    assert!(
        open(Baseline {
            evidence: String::new(),
            ..baseline("1")
        })
        .is_err()
    );
    let ledger = open(baseline("1")).unwrap();
    for (kind, amount, day) in [
        (Kind::Award, "0", "2026-01-02"),
        (Kind::Award, "-1", "2026-01-02"),
        (Kind::Spend, "1", "2025-12-31"),
        (Kind::Adjustment, "0", "2026-01-02"),
    ] {
        assert!(apply(&ledger, &command(1, kind, amount, day)).is_err());
    }
    let mut malformed = command(2, Kind::Award, "1", "2026-01-02");
    malformed.id = "not-a-canonical-uuid".into();
    assert!(apply(&ledger, &malformed).is_err());
    assert_eq!(replay(&ledger).unwrap(), "1");
}
