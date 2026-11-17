use grapevine_workbench::{
    model::{Record, Role},
    playtime::{self, AwardClass, AwardReview, AwardSnapshot, CapClass, Request},
    store::Library,
    xp::{Baseline, Entry, Ledger, XpCommand, XpKind},
};
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;
use tempfile::TempDir;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;
const PRIVATE_CHRONICLE: &str = "PRIVATE_CHRONICLE_NAME_SENTINEL";
const PRIVATE_NOTE: &str = "PRIVATE_ST_NOTE_SENTINEL";
const PRIVATE_REASON: &str = "PRIVATE_AWARD_REASON_SENTINEL";
const PRIVATE_EVIDENCE: &str = "PRIVATE_AWARD_EVIDENCE_SENTINEL";
const PRIVATE_REVIEW_REASON: &str = "PRIVATE_CLASSIFICATION_REASON_SENTINEL";
const PRIVATE_REVIEW_REFERENCE: &str = "PRIVATE_CLASSIFICATION_REFERENCE_SENTINEL";

fn baseline(day: &str, earned: &str, unspent: &str) -> Baseline {
    Baseline {
        day: day.into(),
        earned: earned.into(),
        unspent: unspent.into(),
        actor: "Synthetic opening staff".into(),
        reason: "Synthetic opening balance".into(),
        evidence: "Synthetic XP classification fixture".into(),
    }
}

fn entry(id: &str, kind: XpKind, amount: &str, day: &str, reversal_of: Option<&str>) -> Entry {
    Entry {
        command: XpCommand {
            id: id.into(),
            kind,
            amount: amount.into(),
            day: day.into(),
            actor: "Synthetic recorder".into(),
            reason: format!("Reason for {id}"),
            evidence: format!("Evidence for {id}"),
            reversal_of: reversal_of.map(str::to_owned),
            cost: None,
        },
        recorded_unix_seconds: 0,
        origin_library_id: "synthetic-library".into(),
    }
}

fn ledger(day: &str, earned: &str, unspent: &str, entries: Vec<Entry>) -> Ledger {
    Ledger {
        baseline: baseline(day, earned, unspent),
        entries,
    }
}

fn record(id: &str, kind: &str, xp_ledger: Option<Ledger>) -> Record {
    Record {
        id: id.into(),
        kind: kind.into(),
        name: format!("Synthetic {kind} {id}"),
        player_id: None,
        legacy_id: None,
        public_fields: Map::new(),
        private_fields: Map::new(),
        original: json!({"synthetic_fixture": true}),
        source_sha256: format!("synthetic-source-{id}"),
        st_markers: None,
        build: None,
        xp_ledger,
    }
}

fn confirmed_request(start: &str, end: &str) -> Request {
    Request {
        start_month: start.into(),
        end_month: end.into(),
        class: CapClass::Standard,
        policy_scope_confirmed: true,
        complete_regular_history_confirmed: true,
        full_month_eligible_confirmed: true,
        home_chronicle: PRIVATE_CHRONICLE.into(),
        review_evidence: "PRIVATE_MONTHLY_REVIEW_EVIDENCE_SENTINEL".into(),
        ..Request::default()
    }
}

fn review(award: &AwardSnapshot, class: AwardClass, reason: &str, reference: &str) -> AwardReview {
    AwardReview {
        entry_id: award.entry_id.clone(),
        entry_hash: award.entry_hash.clone(),
        class,
        reason: reason.into(),
        reference: reference.into(),
    }
}

fn error_code<T: std::fmt::Debug>(result: grapevine_workbench::model::Result<T>, code: &str) {
    let error = result.unwrap_err();
    assert_eq!(error.code, code, "{}", error.message);
}

fn awards_record(id: &str, entries: Vec<Entry>) -> Record {
    record(
        id,
        "Vampire",
        Some(ledger("2026-01-01", "200", "20", entries)),
    )
}

#[test]
fn old_requests_default_to_unclassified_and_classifications_keep_every_amount_visible() {
    let character = awards_record(
        "mixed-totals",
        vec![
            entry("regular-six", XpKind::Award, "6", "2026-01-01", None),
            entry("separate-four", XpKind::Award, "4", "2026-01-15", None),
            entry("pending-three", XpKind::Award, "3", "2026-01-20", None),
        ],
    );
    let old_request: Request = serde_json::from_value(json!({
        "start_month": "2026-01",
        "end_month": "2026-01",
        "class": "Standard",
        "policy_scope_confirmed": true,
        "complete_regular_history_confirmed": true,
        "full_month_eligible_confirmed": true,
        "home_chronicle": "Synthetic home evidence",
        "review_evidence": "Synthetic range evidence"
    }))
    .unwrap();
    assert!(old_request.award_reviews.is_empty());

    let default_report = playtime::assess(&character, &old_request).unwrap();
    let default_month = &default_report.months[0];
    assert_eq!(default_month.regular_awarded, "0");
    assert_eq!(default_month.separate_review_awarded, "0");
    assert_eq!(default_month.unclassified_awarded, "13");
    assert_eq!(default_month.source_awarded, "13");
    assert_eq!(default_month.status, "InsufficientEvidence");

    let snapshots = playtime::awards(&character, "2026-01", "2026-01").unwrap();
    assert_eq!(snapshots.len(), 3);
    assert_eq!(snapshots[0].entry_id, "regular-six");
    assert_eq!(snapshots[0].day, "2026-01-01");
    assert_eq!(snapshots[0].amount, "6");
    assert!(snapshots[0].entry_hash.len() == 64);

    let mut split = confirmed_request("2026-01", "2026-01");
    split.award_reviews = vec![
        review(
            &snapshots[0],
            AwardClass::Regular,
            "Synthetic regular decision",
            "Synthetic regular reference",
        ),
        review(
            &snapshots[1],
            AwardClass::SeparateReview,
            "Synthetic exception pending",
            "Synthetic exception reference",
        ),
    ];
    let split_report = playtime::assess(&character, &split).unwrap();
    let split_month = &split_report.months[0];
    assert_eq!(split_month.regular_awarded, "6");
    assert_eq!(split_month.separate_review_awarded, "4");
    assert_eq!(split_month.unclassified_awarded, "3");
    assert_eq!(split_month.source_awarded, "13");
    assert_eq!(split_month.ceiling.as_deref(), Some("12"));
    assert_eq!(split_month.status, "InsufficientEvidence");

    let mut all_regular = confirmed_request("2026-01", "2026-01");
    all_regular.award_reviews = snapshots
        .iter()
        .map(|snapshot| {
            review(
                snapshot,
                AwardClass::Regular,
                "Synthetic explicit regular classification",
                "Synthetic explicit regular evidence",
            )
        })
        .collect();
    let regular_report = playtime::assess(&character, &all_regular).unwrap();
    assert_eq!(regular_report.months[0].regular_awarded, "13");
    assert_eq!(regular_report.months[0].separate_review_awarded, "0");
    assert_eq!(regular_report.months[0].unclassified_awarded, "0");
    assert_eq!(regular_report.months[0].source_awarded, "13");
    assert_eq!(regular_report.months[0].status, "ExceedsDeclaredCeiling");

    let no_awards = record(
        "empty-history",
        "Vampire",
        Some(ledger("2026-01-01", "100", "40", vec![])),
    );
    let empty_report =
        playtime::assess(&no_awards, &confirmed_request("2026-01", "2026-01")).unwrap();
    assert_eq!(empty_report.months[0].status, "WithinDeclaredCeiling");
    assert_eq!(empty_report.months[0].source_awarded, "0");
    assert_eq!(empty_report.months[0].unclassified_awarded, "0");
}

#[test]
fn explicit_regular_math_preserves_opening_band_and_reversal_adjustment_qualifications() {
    let character = record(
        "opening-band",
        "Vampire",
        Some(ledger(
            "2025-12-01",
            "200",
            "0",
            vec![
                entry("prior-month", XpKind::Award, "1", "2025-12-15", None),
                entry("first-day", XpKind::Award, "10", "2026-01-01", None),
            ],
        )),
    );
    let january = playtime::awards(&character, "2026-01", "2026-01").unwrap();
    assert_eq!(january.len(), 1);
    let mut request = confirmed_request("2026-01", "2026-01");
    request.award_reviews = vec![review(
        &january[0],
        AwardClass::Regular,
        "Reviewed as regular",
        "Synthetic January evidence",
    )];
    let report = playtime::assess(&character, &request).unwrap();
    assert_eq!(report.months[0].opening_earned.as_deref(), Some("201"));
    assert_eq!(report.months[0].ceiling.as_deref(), Some("10"));
    assert_eq!(report.months[0].regular_awarded, "10");
    assert_eq!(report.months[0].source_awarded, "10");
    assert_eq!(report.months[0].status, "WithinDeclaredCeiling");

    let reversed = record(
        "reversed-award",
        "Vampire",
        Some(ledger(
            "2026-01-01",
            "100",
            "100",
            vec![
                entry("original-award", XpKind::Award, "5", "2026-01-05", None),
                entry(
                    "award-reversal",
                    XpKind::Reversal,
                    "5",
                    "2026-01-20",
                    Some("original-award"),
                ),
            ],
        )),
    );
    let reversed_awards = playtime::awards(&reversed, "2026-01", "2026-01").unwrap();
    let mut reversed_request = confirmed_request("2026-01", "2026-01");
    reversed_request.award_reviews = vec![review(
        &reversed_awards[0],
        AwardClass::Regular,
        "Reviewed original award",
        "Synthetic reversal review",
    )];
    let reversed_report = playtime::assess(&reversed, &reversed_request).unwrap();
    assert_eq!(reversed_report.months[0].regular_awarded, "5");
    assert_eq!(reversed_report.months[0].source_awarded, "5");
    assert_eq!(reversed_report.months[0].status, "InsufficientEvidence");

    let adjusted = record(
        "adjusted-history",
        "Vampire",
        Some(ledger(
            "2026-01-01",
            "100",
            "100",
            vec![entry(
                "manual-adjustment",
                XpKind::Adjustment,
                "1",
                "2026-01-05",
                None,
            )],
        )),
    );
    let adjusted_report =
        playtime::assess(&adjusted, &confirmed_request("2026-01", "2026-01")).unwrap();
    assert_eq!(adjusted_report.months[0].source_awarded, "0");
    assert_eq!(adjusted_report.months[0].status, "InsufficientEvidence");
}

#[test]
fn review_requires_current_in_range_hashes_unique_ids_and_reason_reference() {
    let character = record(
        "review-target",
        "Vampire",
        Some(ledger(
            "2026-01-01",
            "50",
            "50",
            vec![
                entry("january-entry", XpKind::Award, "2", "2026-01-05", None),
                entry("february-entry", XpKind::Award, "3", "2026-02-05", None),
            ],
        )),
    );
    let rows = playtime::awards(&character, "2026-01", "2026-02").unwrap();
    assert_eq!(rows.len(), 2);
    let valid_review = review(
        &rows[0],
        AwardClass::Regular,
        "Synthetic classification reason",
        "Synthetic review reference",
    );
    let mut valid = confirmed_request("2026-01", "2026-01");
    valid.award_reviews = vec![valid_review.clone()];
    assert_eq!(
        playtime::assess(&character, &valid).unwrap().months[0].regular_awarded,
        "2"
    );

    let mut duplicate = valid.clone();
    duplicate.award_reviews.push(valid_review.clone());
    error_code(
        playtime::assess(&character, &duplicate),
        "PLAYTIME_AWARD_REVIEW_STALE",
    );

    let mut unknown = valid.clone();
    unknown.award_reviews[0].entry_id = "unknown-award-entry".into();
    error_code(
        playtime::assess(&character, &unknown),
        "PLAYTIME_AWARD_REVIEW_STALE",
    );

    let mut stale_hash = valid.clone();
    stale_hash.award_reviews[0].entry_hash = "0".repeat(64);
    error_code(
        playtime::assess(&character, &stale_hash),
        "PLAYTIME_AWARD_REVIEW_STALE",
    );

    let mut out_of_range = confirmed_request("2026-01", "2026-01");
    out_of_range.award_reviews = vec![review(
        &rows[1],
        AwardClass::Regular,
        "Synthetic reason",
        "Synthetic reference",
    )];
    error_code(
        playtime::assess(&character, &out_of_range),
        "PLAYTIME_AWARD_REVIEW_STALE",
    );

    let mut missing_reason = valid.clone();
    missing_reason.award_reviews[0].reason.clear();
    error_code(
        playtime::assess(&character, &missing_reason),
        "PLAYTIME_AWARD_EVIDENCE",
    );
    let mut missing_reference = valid.clone();
    missing_reference.award_reviews[0].reference.clear();
    error_code(
        playtime::assess(&character, &missing_reference),
        "PLAYTIME_AWARD_EVIDENCE",
    );

    let wrong_record = record("different-record", "Vampire", character.xp_ledger.clone());
    error_code(
        playtime::assess(&wrong_record, &valid),
        "PLAYTIME_AWARD_REVIEW_STALE",
    );

    let mut changed_record = character.clone();
    changed_record.xp_ledger.as_mut().unwrap().entries[0]
        .command
        .evidence = "changed after review".into();
    error_code(
        playtime::assess(&changed_record, &valid),
        "PLAYTIME_AWARD_REVIEW_STALE",
    );

    let corrupt = record(
        "corrupt-ledger",
        "Vampire",
        Some(ledger(
            "2026-01-01",
            "50",
            "50",
            vec![entry(
                "bad-amount",
                XpKind::Award,
                "not-a-number",
                "2026-01-05",
                None,
            )],
        )),
    );
    error_code(
        playtime::awards(&corrupt, "2026-01", "2026-01"),
        "PLAYTIME_AWARDS_UNAVAILABLE",
    );
    error_code(
        playtime::awards(&character, "2026-02", "2026-01"),
        "PLAYTIME_RANGE",
    );
}

#[test]
fn row_review_and_aggregate_text_limits_are_bounded() {
    let ten_thousand = (0..10_000)
        .map(|index| {
            entry(
                &format!("award-{index:05}"),
                XpKind::Award,
                "1",
                "2026-01-02",
                None,
            )
        })
        .collect::<Vec<_>>();
    let at_limit = record(
        "at-limit",
        "Vampire",
        Some(ledger("2026-01-01", "0", "0", ten_thousand.clone())),
    );
    let snapshots = playtime::awards(&at_limit, "2026-01", "2026-01").unwrap();
    assert_eq!(snapshots.len(), 10_000);
    let mut at_limit_request = confirmed_request("2026-01", "2026-01");
    at_limit_request.award_reviews = snapshots
        .iter()
        .map(|snapshot| review(snapshot, AwardClass::Regular, "r", "e"))
        .collect();
    let at_limit_report = playtime::assess(&at_limit, &at_limit_request).unwrap();
    assert_eq!(at_limit_report.months[0].regular_awarded, "10000");
    assert_eq!(at_limit_report.months[0].source_awarded, "10000");

    let mut too_many_rows = ten_thousand;
    too_many_rows.push(entry("award-10000", XpKind::Award, "1", "2026-01-02", None));
    let too_many = record(
        "over-row-limit",
        "Vampire",
        Some(ledger("2026-01-01", "0", "0", too_many_rows)),
    );
    error_code(
        playtime::awards(&too_many, "2026-01", "2026-01"),
        "PLAYTIME_AWARD_LIMIT",
    );

    let mut too_many_reviews = at_limit_request.clone();
    too_many_reviews
        .award_reviews
        .push(too_many_reviews.award_reviews[0].clone());
    error_code(
        playtime::assess(&at_limit, &too_many_reviews),
        "PLAYTIME_AWARD_LIMIT",
    );

    let one_award = awards_record(
        "large-review-text",
        vec![entry(
            "single-award",
            XpKind::Award,
            "1",
            "2026-01-05",
            None,
        )],
    );
    let one_snapshot = playtime::awards(&one_award, "2026-01", "2026-01").unwrap();
    let mut large_review = review(
        &one_snapshot[0],
        AwardClass::Regular,
        &"R".repeat(4096),
        &"E".repeat(4096),
    );
    large_review.reason.shrink_to_fit();
    large_review.reference.shrink_to_fit();
    let mut too_much_text = confirmed_request("2026-01", "2026-01");
    too_much_text.award_reviews = vec![large_review; 300];
    error_code(
        playtime::assess(&one_award, &too_much_text),
        "PLAYTIME_AWARD_LIMIT",
    );
}

fn open_library() -> (TempDir, Library, String) {
    let dir = TempDir::new().unwrap();
    let mut library =
        Library::create(&dir.path().join("award-review.gvlib"), PRIVATE_CHRONICLE).unwrap();
    let character_id = library
        .create_character("Vampire", "Synthetic award review", 0, ST)
        .unwrap();
    library
        .initialize_xp(
            &character_id,
            &baseline("2026-01-01", "100", "100"),
            library.revision().unwrap(),
            ST,
        )
        .unwrap();
    let source_bytes = b"synthetic retained source bytes for read-only review";
    let source_hash = Sha256::digest(source_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Connection::open(&library.path)
        .unwrap()
        .execute(
            "INSERT INTO imports(hash,source_name,source_bytes,report) VALUES(?1,?2,?3,?4)",
            params![
                source_hash,
                "synthetic-retained-source.xml",
                source_bytes,
                json!({"kind": "synthetic-read-only-source"}).to_string()
            ],
        )
        .unwrap();
    (dir, library, character_id)
}

fn post(library: &mut Library, character_id: &str, request: &XpCommand) {
    library
        .post_xp(character_id, request, library.revision().unwrap(), ST)
        .unwrap();
}

fn record_data(path: &Path, id: &str) -> String {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
}

fn set_record_data(path: &Path, id: &str, data: &Value) {
    assert_eq!(
        Connection::open(path)
            .unwrap()
            .execute(
                "UPDATE records SET data=?1 WHERE id=?2",
                params![data.to_string(), id],
            )
            .unwrap(),
        1
    );
}

fn import_snapshot(path: &Path) -> Vec<(String, String, Vec<u8>, String)> {
    let connection = Connection::open(path).unwrap();
    let mut statement = connection
        .prepare("SELECT hash,source_name,source_bytes,report FROM imports ORDER BY hash")
        .unwrap();
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn xp_view_snapshot(library: &Library, id: &str) -> Value {
    serde_json::to_value(library.xp_view(id, ST).unwrap()).unwrap()
}

#[test]
fn store_award_review_is_storyteller_only_read_only_and_exports_private_v2_summary() {
    let (_dir, mut library, character_id) = open_library();
    post(
        &mut library,
        &character_id,
        &XpCommand {
            id: "PRIVATE_AWARD_ID_FIRST".into(),
            kind: XpKind::Award,
            amount: "5".into(),
            day: "2026-01-03".into(),
            actor: "PRIVATE_RECORDER_SENTINEL".into(),
            reason: PRIVATE_REASON.into(),
            evidence: PRIVATE_EVIDENCE.into(),
            reversal_of: None,
            cost: None,
        },
    );
    post(
        &mut library,
        &character_id,
        &XpCommand {
            id: "PRIVATE_AWARD_ID_SECOND".into(),
            kind: XpKind::Award,
            amount: "2".into(),
            day: "2026-01-20".into(),
            actor: "PRIVATE_RECORDER_SENTINEL".into(),
            reason: "PRIVATE_SECOND_AWARD_REASON".into(),
            evidence: "PRIVATE_SECOND_AWARD_EVIDENCE".into(),
            reversal_of: None,
            cost: None,
        },
    );
    post(
        &mut library,
        &character_id,
        &XpCommand {
            id: "PRIVATE_AWARD_ID_FEBRUARY".into(),
            kind: XpKind::Award,
            amount: "1".into(),
            day: "2026-02-02".into(),
            actor: "PRIVATE_RECORDER_SENTINEL".into(),
            reason: "PRIVATE_FEBRUARY_REASON".into(),
            evidence: "PRIVATE_FEBRUARY_EVIDENCE".into(),
            reversal_of: None,
            cost: None,
        },
    );

    let revision = library.revision().unwrap();
    let mut raw_record: Value =
        serde_json::from_str(&record_data(&library.path, &character_id)).unwrap();
    raw_record["private_fields"]["staff_note"] = json!(PRIVATE_NOTE);
    set_record_data(&library.path, &character_id, &raw_record);
    let record_before = record_data(&library.path, &character_id);
    let xp_before = xp_view_snapshot(&library, &character_id);
    let imports_before = import_snapshot(&library.path);
    assert_eq!(imports_before.len(), 1);
    assert_eq!(
        imports_before[0].2,
        b"synthetic retained source bytes for read-only review"
    );
    let snapshots = library
        .playtime_awards(&character_id, "2026-01", "2026-01", revision, ST)
        .unwrap();
    assert_eq!(snapshots.len(), 2);
    assert!(snapshots.iter().any(|row| row.reason == PRIVATE_REASON));
    assert!(snapshots.iter().any(|row| row.evidence == PRIVATE_EVIDENCE));
    assert!(snapshots.iter().all(|row| row.day.starts_with("2026-01")));
    assert!(
        library
            .playtime_awards(&character_id, "2026-01", "2026-01", revision, PLAYER)
            .is_err()
    );
    error_code(
        library.playtime_awards(
            &character_id,
            "2026-01",
            "2026-01",
            revision.saturating_sub(1),
            ST,
        ),
        "STALE_REVISION",
    );

    let mut request = confirmed_request("2026-01", "2026-02");
    request.award_reviews = vec![
        review(
            &snapshots[0],
            AwardClass::Regular,
            PRIVATE_REVIEW_REASON,
            PRIVATE_REVIEW_REFERENCE,
        ),
        review(
            &snapshots[1],
            AwardClass::SeparateReview,
            "PRIVATE_SEPARATE_REVIEW_REASON",
            "PRIVATE_SEPARATE_REVIEW_REFERENCE",
        ),
    ];
    let export = library
        .playtime_report(&character_id, &request, revision, ST)
        .unwrap();
    assert_eq!(export["format"], "grapevine-playtime-transfer-review-v2");
    assert_eq!(export["report"]["months"][0]["regular_awarded"], "5");
    assert_eq!(
        export["report"]["months"][0]["separate_review_awarded"],
        "2"
    );
    assert_eq!(export["report"]["months"][0]["unclassified_awarded"], "0");
    assert_eq!(export["report"]["months"][0]["source_awarded"], "7");
    assert_eq!(export["report"]["months"][1]["regular_awarded"], "0");
    assert_eq!(export["report"]["months"][1]["unclassified_awarded"], "1");
    assert_eq!(export["report"]["months"][1]["source_awarded"], "1");
    assert_eq!(export["report"]["status"], "InsufficientEvidence");
    assert!(
        export["report"]["award_classification_digest"]
            .as_str()
            .is_some_and(|digest| digest.len() == 64)
    );

    let exported = export.to_string();
    for private in [
        "PRIVATE_AWARD_ID_FIRST",
        "PRIVATE_AWARD_ID_SECOND",
        "PRIVATE_AWARD_ID_FEBRUARY",
        PRIVATE_REASON,
        PRIVATE_EVIDENCE,
        "PRIVATE_RECORDER_SENTINEL",
        PRIVATE_REVIEW_REASON,
        PRIVATE_REVIEW_REFERENCE,
        "PRIVATE_SEPARATE_REVIEW_REASON",
        "PRIVATE_SEPARATE_REVIEW_REFERENCE",
        PRIVATE_CHRONICLE,
        PRIVATE_NOTE,
    ] {
        assert!(!exported.contains(private), "v2 export leaked {private}");
    }

    let mut reverse_order = request.clone();
    reverse_order.award_reviews.reverse();
    let reversed_export = library
        .playtime_report(&character_id, &reverse_order, revision, ST)
        .unwrap();
    assert_eq!(
        export["report"]["award_classification_digest"],
        reversed_export["report"]["award_classification_digest"],
        "classification digest must not depend on request ordering"
    );
    assert!(
        library
            .playtime_report(&character_id, &request, revision, PLAYER)
            .is_err()
    );
    error_code(
        library.playtime_report(&character_id, &request, revision.saturating_sub(1), ST),
        "STALE_REVISION",
    );

    assert_eq!(library.revision().unwrap(), revision);
    assert_eq!(record_data(&library.path, &character_id), record_before);
    assert_eq!(xp_view_snapshot(&library, &character_id), xp_before);
    assert_eq!(import_snapshot(&library.path), imports_before);
}
