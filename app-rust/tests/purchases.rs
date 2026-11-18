use grapevine_workbench::{
    model::{BuildKind, Edit, Role},
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::{ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID},
    purchases::{PurchaseQuote, PurchaseRequest, TeacherRule},
    store::Library,
    xp::{Baseline, XpCommand, XpKind},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::TempDir;
use uuid::Uuid;

fn baseline(unspent: &str) -> Baseline {
    Baseline {
        day: "2026-01-01".into(),
        earned: "10.25".into(),
        unspent: unspent.into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic opening balance".into(),
        evidence: "Synthetic fixture".into(),
    }
}

fn request(power_id: &str) -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: Uuid::new_v4().to_string(),
        power_id: power_id.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(ClanRelation::OutOfClan),
        affiliation_name: "Synthetic clan".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic next session 2026-01-03".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic purchase test".into(),
        policy_version: "synthetic-policy-v1".into(),
        policy_reference: "Synthetic local policy reference".into(),
        affiliation_evidence: "Synthetic affiliation evidence".into(),
        approval_reference: "Synthetic approval reference".into(),
        eligibility_evidence: "Synthetic eligibility evidence".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Synthetic qualified teacher evidence".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    }
}

fn fixture(unspent: &str) -> (TempDir, Library, String) {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("purchases.gvlib");
    let mut library = Library::create(&path, "Synthetic purchase tests").unwrap();
    let id = library
        .create_character("Vampire", "Synthetic Vampire", 0, Role::Storyteller)
        .unwrap();
    library
        .initialize_xp(&id, &baseline(unspent), 1, Role::Storyteller)
        .unwrap();
    (directory, library, id)
}

fn quote(library: &Library, id: &str, request: &PurchaseRequest) -> PurchaseQuote {
    library
        .quote_purchase(id, request, library.revision().unwrap(), Role::Storyteller)
        .unwrap()
}

fn raw(path: &Path, id: &str) -> String {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .unwrap()
}

fn audit_count(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM audit", [], |r| r.get(0))
        .unwrap()
}

fn file_sha256(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn strip_schema4(connection: &Connection, version: i64) {
    connection
        .execute_batch(
            "DROP TRIGGER library_revision_guard_before_update;
             DROP TABLE purchase_write_guard;
             DROP TABLE purchase_receipts; DROP TABLE game_sessions; DROP TABLE scheduled_sessions;",
        )
        .unwrap();
    connection
        .pragma_update(None, "user_version", version)
        .unwrap();
}

#[test]
fn purchase_commit_is_atomic_retryable_and_receipts_survive_reopen() {
    let (_directory, mut library, id) = fixture("10.25");
    let request = request(ANIMALISM_FERAL_WHISPERS_ID);
    let quote = quote(&library, &id, &request);
    assert_eq!(quote.before_unspent, "10.25");
    assert_eq!(quote.after_unspent, "6.25");
    let before_data = raw(&library.path, &id);
    let before_audit = audit_count(&library.path);

    for (trigger, sql) in [
        (
            "fail_purchase_record",
            "CREATE TRIGGER fail_purchase_record BEFORE UPDATE ON records
             BEGIN SELECT RAISE(ABORT,'injected record failure'); END;",
        ),
        (
            "fail_purchase_receipt",
            "CREATE TRIGGER fail_purchase_receipt BEFORE INSERT ON purchase_receipts
             BEGIN SELECT RAISE(ABORT,'injected receipt failure'); END;",
        ),
        (
            "fail_purchase_audit",
            "CREATE TRIGGER fail_purchase_audit
             BEFORE INSERT ON audit WHEN NEW.operation='purchase_commit'
             BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;",
        ),
    ] {
        Connection::open(&library.path)
            .unwrap()
            .execute_batch(sql)
            .unwrap();
        assert!(library.commit_purchase(&quote, Role::Storyteller).is_err());
        assert_eq!(library.revision().unwrap(), 2, "{trigger}");
        assert_eq!(raw(&library.path, &id), before_data, "{trigger}");
        assert_eq!(audit_count(&library.path), before_audit, "{trigger}");
        assert!(
            library
                .purchase_receipts(&id, Role::Storyteller)
                .unwrap()
                .is_empty(),
            "{trigger}"
        );
        Connection::open(&library.path)
            .unwrap()
            .execute_batch(&format!("DROP TRIGGER {trigger};"))
            .unwrap();
    }
    let receipt = library.commit_purchase(&quote, Role::Storyteller).unwrap();
    assert_eq!(receipt.committed_revision, 3);
    assert_eq!(library.revision().unwrap(), 3);
    assert_eq!(
        library
            .xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("6.25")
    );
    let stored: Value = serde_json::from_str(&raw(&library.path, &id)).unwrap();
    assert_eq!(
        stored["private_fields"]["DisciplineList"]["Traits"][0]["AppPurchaseId"],
        json!(request.id.clone())
    );
    assert_eq!(
        library.purchase_receipts(&id, Role::Storyteller).unwrap(),
        vec![receipt.clone()]
    );

    // An exact retry returns the committed receipt after unrelated revisions move.
    library
        .create_character("Mortal", "Unrelated", 3, Role::Storyteller)
        .unwrap();
    assert_eq!(library.revision().unwrap(), 4);
    assert_eq!(
        library.commit_purchase(&quote, Role::Storyteller).unwrap(),
        receipt
    );
    let mut conflicting = quote.clone();
    conflicting.request.reason = "Different request data".into();
    assert_eq!(
        library
            .commit_purchase(&conflicting, Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_ID_CONFLICT"
    );

    let path = library.path.clone();
    drop(library);
    let reopened = Library::open(&path).unwrap();
    assert_eq!(reopened.revision().unwrap(), 4);
    assert_eq!(
        reopened.purchase_receipts(&id, Role::Storyteller).unwrap(),
        vec![receipt]
    );
}

#[test]
fn purchase_quote_rejects_player_build_and_insufficient_balance() {
    let (_directory, mut library, id) = fixture("1");
    let request = request(ANIMALISM_FERAL_WHISPERS_ID);
    let revision = library.revision().unwrap();
    assert_eq!(
        library
            .quote_purchase(&id, &request, revision, Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert_eq!(
        library
            .purchase_receipts(&id, Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );

    let build_id = library
        .create_build(&id, BuildKind::Draft, revision, Role::Storyteller)
        .unwrap();
    let current = library.revision().unwrap();
    assert_eq!(
        library
            .quote_purchase(&build_id, &request, current, Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_SCOPE"
    );
    assert_eq!(
        library
            .quote_purchase(&id, &request, current, Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_INSUFFICIENT_XP"
    );
}

#[test]
fn stale_and_forged_purchase_quotes_do_not_write() {
    let (_directory, mut library, id) = fixture("10.25");
    let request = request(ANIMALISM_FERAL_WHISPERS_ID);
    let quote = quote(&library, &id, &request);
    let original = raw(&library.path, &id);

    let mut forged = quote.clone();
    forged.after_unspent = "999".into();
    assert_eq!(
        library
            .commit_purchase(&forged, Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_QUOTE_CHANGED"
    );
    assert_eq!(library.revision().unwrap(), quote.revision);
    assert_eq!(raw(&library.path, &id), original);
    assert!(
        library
            .purchase_receipts(&id, Role::Storyteller)
            .unwrap()
            .is_empty()
    );

    library
        .save_edit(
            &id,
            &Edit {
                name: "Edited after quote".into(),
                ..Default::default()
            },
            quote.revision,
            Role::Storyteller,
        )
        .unwrap();
    let edited = raw(&library.path, &id);
    assert_eq!(
        library
            .commit_purchase(&quote, Role::Storyteller)
            .unwrap_err()
            .code,
        "STALE_PURCHASE"
    );
    assert_eq!(raw(&library.path, &id), edited);
    assert_eq!(library.revision().unwrap(), quote.revision + 1);
}

#[test]
fn purchase_drift_appears_in_character_roster_and_export_audits() {
    let (_directory, mut library, id) = fixture("10.25");
    let request = request(ANIMALISM_FERAL_WHISPERS_ID);
    let quote = quote(&library, &id, &request);
    library.commit_purchase(&quote, Role::Storyteller).unwrap();

    let mut sheet = library.sheet_edit(&id, Role::Storyteller).unwrap();
    sheet.lists.get_mut("DisciplineList").unwrap()["Traits"][0]["Note"] =
        json!("Edited after the receipt");
    let revision = library.revision().unwrap();
    library
        .save_sheet(&id, &sheet, revision, Role::Storyteller)
        .unwrap();

    let view = library.xp_view(&id, Role::Storyteller).unwrap();
    assert!(
        view.findings.iter().any(|finding| {
            finding.code == "PURCHASE_TRAIT_DRIFT" && finding.severity == "review"
        })
    );
    assert!(
        library
            .xp_roster_audit(Role::Storyteller)
            .unwrap()
            .into_iter()
            .find(|view| view.record_id == id)
            .unwrap()
            .findings
            .iter()
            .any(|finding| finding.code == "PURCHASE_TRAIT_DRIFT")
    );
    let export = library
        .xp_audit_export(Some(&id), Role::Storyteller)
        .unwrap();
    assert!(
        export["records"][0]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "PURCHASE_TRAIT_DRIFT")
    );

    let saved = raw(&library.path, &id);
    let revision = library.revision().unwrap();
    let manual_reuse = XpCommand {
        id: request.id.clone(),
        kind: XpKind::Award,
        amount: "1".into(),
        day: "2026-01-04".into(),
        actor: "Synthetic ST".into(),
        reason: "Manual ID reuse attempt".into(),
        evidence: "Synthetic test".into(),
        reversal_of: None,
        cost: None,
    };
    assert_eq!(
        library
            .post_xp(&id, &manual_reuse, revision, Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_ID_RESERVED"
    );
    let reversal = XpCommand {
        id: Uuid::new_v4().to_string(),
        kind: XpKind::Reversal,
        amount: "0".into(),
        day: "2026-01-04".into(),
        actor: "Synthetic ST".into(),
        reason: "Manual purchase reversal attempt".into(),
        evidence: "Synthetic test".into(),
        reversal_of: Some(request.id),
        cost: None,
    };
    assert_eq!(
        library
            .post_xp(&id, &reversal, revision, Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_SPEND_IMMUTABLE"
    );
    assert_eq!(raw(&library.path, &id), saved);
    assert_eq!(library.revision().unwrap(), revision);
}

#[test]
fn backup_restore_copies_receipts_and_old_schema_restore_clears_them() {
    let (directory, mut library, id) = fixture("10.25");
    let base_backup = directory.path().join("base.gvbackup");
    library.backup(&base_backup, Role::Storyteller).unwrap();
    let high_target_path = directory.path().join("high-target.gvlib");
    fs::copy(&base_backup, &high_target_path).unwrap();

    let old_backup = directory.path().join("old-schema3.gvbackup");
    library.backup(&old_backup, Role::Storyteller).unwrap();
    let old_conn = Connection::open(&old_backup).unwrap();
    strip_schema4(&old_conn, 3);
    drop(old_conn);
    let old_backup_hash = file_sha256(&old_backup);

    let first_request = request(ANIMALISM_FERAL_WHISPERS_ID);
    let first_quote = quote(&library, &id, &first_request);
    let first = library
        .commit_purchase(&first_quote, Role::Storyteller)
        .unwrap();
    let paid_backup = directory.path().join("paid.gvbackup");
    library.backup(&paid_backup, Role::Storyteller).unwrap();
    let paid_backup_hash = file_sha256(&paid_backup);

    let second_request = request(ANIMALISM_BECKONING_ID);
    let second_quote = quote(&library, &id, &second_request);
    library
        .commit_purchase(&second_quote, Role::Storyteller)
        .unwrap();
    assert_eq!(
        library
            .purchase_receipts(&id, Role::Storyteller)
            .unwrap()
            .len(),
        2
    );
    let future_backup = directory.path().join("future.gvbackup");
    library.backup(&future_backup, Role::Storyteller).unwrap();
    let future_backup_hash = file_sha256(&future_backup);

    // Restore a later snapshot into an older clone. The final revision exceeds
    // expected+1, so the guard must be updated for both restore and later writes.
    let mut high_target = Library::open(&high_target_path).unwrap();
    assert_eq!(high_target.revision().unwrap(), 2);
    let old_writer = Connection::open(&high_target_path).unwrap();
    assert_eq!(
        high_target
            .restore(&future_backup, 2, Role::Storyteller)
            .unwrap(),
        5
    );
    assert_eq!(
        high_target
            .purchase_receipts(&id, Role::Storyteller)
            .unwrap()
            .len(),
        2
    );
    assert!(
        old_writer
            .execute("UPDATE library SET revision=revision+1", [])
            .is_err()
    );
    high_target
        .create_character("Mortal", "Guard check", 5, Role::Storyteller)
        .unwrap();
    assert_eq!(high_target.revision().unwrap(), 6);
    assert_eq!(file_sha256(&future_backup), future_backup_hash);

    let current = library.revision().unwrap();
    let restored = library
        .restore(&paid_backup, current, Role::Storyteller)
        .unwrap();
    assert_eq!(restored, current + 1);
    assert_eq!(
        library.purchase_receipts(&id, Role::Storyteller).unwrap(),
        vec![first]
    );
    assert_eq!(file_sha256(&paid_backup), paid_backup_hash);

    let restored = library
        .restore(&old_backup, restored, Role::Storyteller)
        .unwrap();
    assert_eq!(restored, current + 2);
    assert!(
        library
            .purchase_receipts(&id, Role::Storyteller)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        library
            .xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("10.25")
    );
    assert_eq!(file_sha256(&old_backup), old_backup_hash);
}

#[test]
fn schema3_upgrade_keeps_backup_and_blocks_an_already_open_old_writer() {
    let (directory, library, id) = fixture("10.25");
    let path = library.path.clone();
    let before = raw(&path, &id);
    drop(library);

    let old_writer = Connection::open(&path).unwrap();
    strip_schema4(&old_writer, 3);
    drop(old_writer);
    let old_writer = Connection::open(&path).unwrap();
    let migrated = Library::open(&path).unwrap();
    assert_eq!(migrated.revision().unwrap(), 8);
    assert_eq!(raw(&path, &id), before);
    let version: i64 = Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 9);
    assert!(![1, 2, 3, 4, 5].contains(&version));

    let backup = fs::read_dir(directory.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("pre-schema9-")
        })
        .unwrap();
    let backup_hash = file_sha256(&backup);
    let backup_conn = Connection::open(&backup).unwrap();
    assert_eq!(
        backup_conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
    let had_receipt_table: bool = backup_conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='purchase_receipts')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!had_receipt_table);

    // This connection was opened before schema4 existed and models the 0.6 writer.
    assert!(
        old_writer
            .execute("UPDATE library SET revision=revision+1", [])
            .is_err()
    );
    assert_eq!(migrated.revision().unwrap(), 8);
    assert_eq!(raw(&path, &id), before);
    assert_eq!(file_sha256(&backup), backup_hash);
}

#[test]
fn schema1_to4_failure_rolls_back_both_migration_steps_and_keeps_backup() {
    let (directory, library, id) = fixture("10.25");
    let path = library.path.clone();
    let before = raw(&path, &id);
    let prior_revision = library.revision().unwrap();
    drop(library);

    let connection = Connection::open(&path).unwrap();
    strip_schema4(&connection, 1);
    let schema3_revision = prior_revision + 1;
    let trigger = format!(
        "CREATE TRIGGER fail_schema4 BEFORE UPDATE OF revision ON library
         WHEN OLD.revision={schema3_revision}
         BEGIN SELECT RAISE(ABORT,'injected schema4 failure'); END;"
    );
    connection.execute_batch(&trigger).unwrap();
    drop(connection);

    assert!(Library::open(&path).is_err());
    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT revision FROM library", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        prior_revision
    );
    drop(connection);
    assert_eq!(raw(&path, &id), before);

    let backup = fs::read_dir(directory.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("pre-schema9-")
        })
        .unwrap();
    let backup_hash = file_sha256(&backup);
    let backup_conn = Connection::open(&backup).unwrap();
    assert_eq!(
        backup_conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        backup_conn
            .query_row("SELECT revision FROM library", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        prior_revision
    );
    assert_eq!(file_sha256(&backup), backup_hash);
}
