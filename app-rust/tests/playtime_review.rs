use grapevine_workbench::{
    cancellations::{CancellationQuote, CancellationRequest},
    legacy_calendar,
    model::{Result, Role},
    playtime::{AwardClass, AwardReview, CapClass, Request},
    playtime_review::FIELD,
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{PurchaseRequest, TeacherRule},
    store::Library,
    xp::{Baseline, XpCommand, XpKind},
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

fn baseline(earned: &str, unspent: &str) -> Baseline {
    Baseline {
        day: "2026-01-01".into(),
        earned: earned.into(),
        unspent: unspent.into(),
        actor: "Synthetic opening staff".into(),
        reason: "Synthetic opening balance".into(),
        evidence: "Synthetic opening evidence".into(),
    }
}

fn fixture() -> (TempDir, Library, String) {
    let dir = TempDir::new().unwrap();
    let mut library =
        Library::create(&dir.path().join("review.gvlib"), "Synthetic Chronicle").unwrap();
    let id = library
        .create_character("Vampire", "Synthetic Vampire", 0, ST)
        .unwrap();
    library
        .initialize_xp(&id, &baseline("100", "100"), 1, ST)
        .unwrap();
    add_award(&mut library, &id, "award-first", "2026-01-03");
    (dir, library, id)
}

fn add_award(library: &mut Library, character_id: &str, id: &str, day: &str) {
    library
        .post_xp(
            character_id,
            &XpCommand {
                id: id.into(),
                kind: XpKind::Award,
                amount: "1".into(),
                day: day.into(),
                actor: "PRIVATE_ST_ACTOR".into(),
                reason: format!("PRIVATE_REASON_{id}"),
                evidence: format!("PRIVATE_EVIDENCE_{id}"),
                reversal_of: None,
                cost: None,
            },
            library.revision().unwrap(),
            ST,
        )
        .unwrap();
}

fn request(library: &Library, record_id: &str) -> Request {
    let rows = library
        .playtime_awards(
            record_id,
            "2026-01",
            "2026-01",
            library.revision().unwrap(),
            ST,
        )
        .unwrap();
    Request {
        start_month: "2026-01".into(),
        end_month: "2026-01".into(),
        class: CapClass::Standard,
        policy_scope_confirmed: true,
        complete_regular_history_confirmed: true,
        full_month_eligible_confirmed: true,
        home_chronicle: "PRIVATE_HOME_CHRONICLE".into(),
        review_evidence: "PRIVATE_RANGE_REFERENCE".into(),
        award_reviews: rows
            .iter()
            .map(|row| AwardReview {
                entry_id: row.entry_id.clone(),
                entry_hash: row.entry_hash.clone(),
                class: AwardClass::Regular,
                reason: format!("PRIVATE_CLASS_REASON_{}", row.entry_id),
                reference: format!("PRIVATE_CLASS_REFERENCE_{}", row.entry_id),
            })
            .collect(),
    }
}

fn record_data(path: &Path, id: &str) -> String {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
}

fn profile_data(path: &Path) -> Value {
    let value: String = Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT data FROM chronicle_profile WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_str(&value).unwrap()
}

fn set_profile_data(path: &Path, value: &Value) {
    assert_eq!(
        Connection::open(path)
            .unwrap()
            .execute(
                "UPDATE chronicle_profile SET data=?1 WHERE singleton=1",
                [value.to_string()],
            )
            .unwrap(),
        1
    );
}

fn audit_count(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT count(*) FROM audit", [], |row| row.get(0))
        .unwrap()
}

fn insert_retained_source(path: &Path) {
    let bytes = b"synthetic retained source bytes for private review";
    let hash = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Connection::open(path)
        .unwrap()
        .execute(
            "INSERT INTO imports(hash,source_name,source_bytes,report) VALUES(?1,?2,?3,?4)",
            params![
                hash,
                "synthetic-source.xml",
                bytes,
                json!({"kind":"synthetic-source"}).to_string()
            ],
        )
        .unwrap();
}

fn import_snapshot(path: &Path) -> Vec<(String, Vec<u8>, String)> {
    let conn = Connection::open(path).unwrap();
    let mut stmt = conn
        .prepare("SELECT hash,source_bytes,report FROM imports ORDER BY hash")
        .unwrap();
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn error_code<T: std::fmt::Debug>(result: Result<T>, expected: &str) {
    let error = result.unwrap_err();
    assert_eq!(error.code, expected, "{}", error.message);
}

fn purchase_request() -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: Uuid::new_v4().to_string(),
        power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(ClanRelation::OutOfClan),
        affiliation_name: "Synthetic clan".into(),
        day: "2026-01-04".into(),
        available_from: "2026-01-05".into(),
        next_session_reference: "Synthetic next session".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic purchase".into(),
        policy_version: "synthetic-policy-v1".into(),
        policy_reference: "Synthetic policy reference".into(),
        affiliation_evidence: "Synthetic affiliation".into(),
        approval_reference: "Synthetic approval".into(),
        eligibility_evidence: "Synthetic eligibility".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Synthetic teacher evidence".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    }
}

fn cancel_request() -> CancellationRequest {
    CancellationRequest {
        id: Uuid::new_v4().to_string(),
        day: "2026-01-06".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic correction".into(),
        evidence: "Synthetic cancellation evidence".into(),
    }
}

#[test]
fn save_is_storyteller_only_idempotent_and_atomic_without_record_writes() {
    let (_dir, mut library, id) = fixture();
    let profile = library.chronicle_profile(ST).unwrap().profile;
    library
        .save_chronicle_profile(&profile, library.revision().unwrap(), ST)
        .unwrap();
    let input = request(&library, &id);
    let expected = library.revision().unwrap();
    let record_before = record_data(&library.path, &id);
    let audit_before = audit_count(&library.path);
    let failing = Connection::open(&library.path).unwrap();
    failing
        .execute_batch(
            "CREATE TRIGGER fail_review BEFORE UPDATE ON chronicle_profile
             BEGIN SELECT RAISE(ABORT,'synthetic review failure'); END;",
        )
        .unwrap();
    assert!(
        library
            .save_playtime_award_review(&id, "review-atomic", &input, expected, ST)
            .is_err()
    );
    assert_eq!(library.revision().unwrap(), expected);
    assert_eq!(audit_count(&library.path), audit_before);
    assert_eq!(record_data(&library.path, &id), record_before);
    failing.execute_batch("DROP TRIGGER fail_review").unwrap();

    error_code(
        library.save_playtime_award_review(&id, "review-player", &input, expected, PLAYER),
        "ACCESS_DENIED",
    );
    let saved = library
        .save_playtime_award_review(&id, "review-atomic", &input, expected, ST)
        .unwrap();
    let committed_revision = library.revision().unwrap();
    assert_eq!(committed_revision, expected + 1);
    assert_eq!(audit_count(&library.path), audit_before + 1);
    assert_eq!(record_data(&library.path, &id), record_before);
    assert_eq!(
        saved,
        library
            .save_playtime_award_review(&id, "review-atomic", &input, expected, ST)
            .unwrap(),
        "an exact historic retry must not write again"
    );
    assert_eq!(library.revision().unwrap(), committed_revision);
    assert_eq!(audit_count(&library.path), audit_before + 1);
    error_code(
        library.load_playtime_award_review(&id, "review-atomic", committed_revision, PLAYER),
        "ACCESS_DENIED",
    );

    let mut conflict = input.clone();
    conflict.award_reviews[0].reason.push_str(" changed");
    error_code(
        library.save_playtime_award_review(&id, "review-atomic", &conflict, committed_revision, ST),
        "PLAYTIME_REVIEW_ID_CONFLICT",
    );
    error_code(
        library.save_playtime_award_review(&id, "review-new", &input, expected, ST),
        "STALE_REVISION",
    );
    assert!(library.playtime_award_reviews(&id, PLAYER).is_err());
    assert_eq!(library.revision().unwrap(), committed_revision);
    assert_eq!(record_data(&library.path, &id), record_before);
}

#[test]
fn load_restores_only_current_hashes_and_resets_every_policy_confirmation() {
    let (_dir, mut library, id) = fixture();
    let input = request(&library, &id);
    library
        .save_playtime_award_review(
            &id,
            "review-reload",
            &input,
            library.revision().unwrap(),
            ST,
        )
        .unwrap();
    add_award(&mut library, &id, "award-new", "2026-01-20");
    let loaded = library
        .load_playtime_award_review(&id, "review-reload", library.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(loaded.input.start_month, "2026-01");
    assert_eq!(loaded.input.end_month, "2026-01");
    assert_eq!(loaded.input.class, CapClass::Unknown);
    assert!(!loaded.input.policy_scope_confirmed);
    assert!(!loaded.input.complete_regular_history_confirmed);
    assert!(!loaded.input.full_month_eligible_confirmed);
    assert!(loaded.input.home_chronicle.is_empty());
    assert!(loaded.input.review_evidence.is_empty());
    assert_eq!(loaded.input.award_reviews.len(), 1);
    assert_eq!(loaded.input.award_reviews[0].entry_id, "award-first");
    assert_eq!(loaded.stale_reviews, 0);
    assert_eq!(loaded.unclassified_awards, 1);

    let mut changed: Value = serde_json::from_str(&record_data(&library.path, &id)).unwrap();
    changed["xp_ledger"]["entries"][0]["command"]["reason"] =
        json!("synthetic changed immutable entry");
    Connection::open(&library.path)
        .unwrap()
        .execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![changed.to_string(), id],
        )
        .unwrap();
    let stale = library
        .load_playtime_award_review(&id, "review-reload", library.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(stale.input.award_reviews.len(), 0);
    assert_eq!(stale.stale_reviews, 1);
    assert_eq!(stale.unclassified_awards, 2);
}

#[test]
fn raw_profile_extensions_backup_restore_and_public_outputs_preserve_privacy() {
    let (dir, mut library, id) = fixture();
    let profile = library.chronicle_profile(ST).unwrap().profile;
    library
        .save_chronicle_profile(&profile, library.revision().unwrap(), ST)
        .unwrap();
    let mut raw = profile_data(&library.path);
    raw["future_profile_key"] = json!({"synthetic":"preserve-me"});
    raw[legacy_calendar::FIELD] = json!([]);
    set_profile_data(&library.path, &raw);
    insert_retained_source(&library.path);

    let input = request(&library, &id);
    let revision = library.revision().unwrap();
    let record_before = record_data(&library.path, &id);
    let imports_before = import_snapshot(&library.path);
    let saved = library
        .save_playtime_award_review(&id, "review-private", &input, revision, ST)
        .unwrap();
    assert_eq!(record_data(&library.path, &id), record_before);
    assert_eq!(import_snapshot(&library.path), imports_before);
    let raw_after_save = profile_data(&library.path);
    assert_eq!(
        raw_after_save["future_profile_key"]["synthetic"],
        "preserve-me"
    );
    assert_eq!(raw_after_save[legacy_calendar::FIELD], json!([]));
    assert_eq!(raw_after_save[FIELD][0]["id"], "review-private");

    let public = serde_json::to_string(&library.public_chronicle_profile().unwrap()).unwrap();
    for private in [
        "review-private",
        "PRIVATE_CLASS_REASON",
        "PRIVATE_CLASS_REFERENCE",
    ] {
        assert!(!public.contains(private));
    }
    let export = library
        .playtime_report(&id, &input, library.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(export["format"], "grapevine-playtime-transfer-review-v2");
    let export = export.to_string();
    for private in [
        "review-private",
        "award-first",
        "PRIVATE_REASON_award-first",
        "PRIVATE_EVIDENCE_award-first",
        "PRIVATE_CLASS_REASON_award-first",
        "PRIVATE_CLASS_REFERENCE_award-first",
        "PRIVATE_HOME_CHRONICLE",
        "PRIVATE_RANGE_REFERENCE",
    ] {
        assert!(!export.contains(private), "v2 export leaked {private}");
    }

    let mut edited = library.chronicle_profile(ST).unwrap().profile;
    edited.website = "https://synthetic.invalid/profile-edit".into();
    library
        .save_chronicle_profile(&edited, library.revision().unwrap(), ST)
        .unwrap();
    let after_edit = profile_data(&library.path);
    assert_eq!(after_edit[FIELD][0]["content_hash"], saved.content_hash);
    assert_eq!(after_edit["future_profile_key"]["synthetic"], "preserve-me");
    assert_eq!(after_edit[legacy_calendar::FIELD], json!([]));

    let backup = dir.path().join("review-backup.gvlib");
    library.backup(&backup, ST).unwrap();
    let mut second = input.clone();
    second.award_reviews[0].reason = "PRIVATE_SECOND_SAVED_REASON".into();
    library
        .save_playtime_award_review(
            &id,
            "review-after-backup",
            &second,
            library.revision().unwrap(),
            ST,
        )
        .unwrap();
    library
        .restore(&backup, library.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(library.playtime_award_reviews(&id, ST).unwrap().len(), 1);
    assert_eq!(record_data(&library.path, &id), record_before);
    assert_eq!(import_snapshot(&library.path), imports_before);
    let path = library.path.clone();
    drop(library);
    let reopened = Library::open(&path).unwrap();
    assert_eq!(reopened.playtime_award_reviews(&id, ST).unwrap()[0], saved);
    assert_eq!(profile_data(&path)[legacy_calendar::FIELD], json!([]));
}

#[test]
fn bounded_history_and_tampered_content_fail_without_partial_writes() {
    let (_dir, mut library, id) = fixture();
    let input = request(&library, &id);
    for index in 0..128 {
        let next = library.revision().unwrap();
        library
            .save_playtime_award_review(&id, &format!("review-{index:03}"), &input, next, ST)
            .unwrap();
    }
    assert_eq!(library.playtime_award_reviews(&id, ST).unwrap().len(), 128);
    let revision = library.revision().unwrap();
    let audit = audit_count(&library.path);
    let profile_before = profile_data(&library.path);
    error_code(
        library.save_playtime_award_review(&id, "review-128", &input, revision, ST),
        "PLAYTIME_REVIEW_LIMIT",
    );
    assert_eq!(library.revision().unwrap(), revision);
    assert_eq!(audit_count(&library.path), audit);
    assert_eq!(profile_data(&library.path), profile_before);

    let mut tampered = profile_data(&library.path);
    tampered[FIELD][0]["content_hash"] = json!("0".repeat(64));
    set_profile_data(&library.path, &tampered);
    error_code(
        library.playtime_award_reviews(&id, ST),
        "PLAYTIME_REVIEW_INVALID",
    );
    assert_eq!(library.revision().unwrap(), revision);

    tampered[FIELD] = json!("x".repeat(2 * 1024 * 1024 + 1));
    set_profile_data(&library.path, &tampered);
    error_code(
        library.playtime_award_reviews(&id, ST),
        "PLAYTIME_REVIEW_LIMIT",
    );
    assert_eq!(library.revision().unwrap(), revision);
}

#[test]
fn saved_review_only_stales_cancellation_quote_by_library_revision() {
    let dir = TempDir::new().unwrap();
    let mut library =
        Library::create(&dir.path().join("cancel.gvlib"), "Synthetic Cancel").unwrap();
    let id = library
        .create_character("Vampire", "Synthetic Vampire", 0, ST)
        .unwrap();
    library
        .initialize_xp(&id, &baseline("10.25", "10.25"), 1, ST)
        .unwrap();
    add_award(&mut library, &id, "award-cancel", "2026-01-03");
    let purchase = library
        .quote_purchase(&id, &purchase_request(), library.revision().unwrap(), ST)
        .unwrap();
    library.commit_purchase(&purchase, ST).unwrap();
    let cancellation = cancel_request();
    let old_quote: CancellationQuote = library
        .quote_cancellation(
            &purchase.request.id,
            &cancellation,
            library.revision().unwrap(),
            ST,
        )
        .unwrap();
    let receipt_before: String = Connection::open(&library.path)
        .unwrap()
        .query_row("SELECT data FROM purchase_receipts", [], |row| row.get(0))
        .unwrap();
    let input = request(&library, &id);
    library
        .save_playtime_award_review(
            &id,
            "review-cancel",
            &input,
            library.revision().unwrap(),
            ST,
        )
        .unwrap();
    let receipt_after: String = Connection::open(&library.path)
        .unwrap()
        .query_row("SELECT data FROM purchase_receipts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(receipt_before, receipt_after);
    error_code(
        library.commit_cancellation(&old_quote, ST),
        "STALE_CANCELLATION",
    );
    let fresh_quote = library
        .quote_cancellation(
            &purchase.request.id,
            &cancellation,
            library.revision().unwrap(),
            ST,
        )
        .unwrap();
    library.commit_cancellation(&fresh_quote, ST).unwrap();
    assert!(
        library.purchase_receipts(&id, ST).unwrap()[0]
            .cancellation
            .is_some()
    );
    let loaded = library
        .load_playtime_award_review(&id, "review-cancel", library.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(loaded.stale_reviews, 0);
    assert_eq!(loaded.input.award_reviews.len(), 1);
}
