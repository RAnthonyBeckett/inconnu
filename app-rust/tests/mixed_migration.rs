use grapevine_workbench::{
    mixed_advancement as mixed,
    model::Role,
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{PurchaseRequest, TeacherRule},
    store::Library,
    xp::Baseline,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const ST: Role = Role::Storyteller;

fn fixture(directory: &Path, filename: &str) -> (Library, String) {
    let path = directory.join(filename);
    let mut library = Library::create(&path, "Synthetic schema migration").unwrap();
    let id = library
        .create_character("Vampire", "Synthetic migration subject", 0, ST)
        .unwrap();
    library
        .initialize_xp(
            &id,
            &Baseline {
                day: "2026-01-01".into(),
                earned: "20.25".into(),
                unspent: "20.25".into(),
                actor: "Synthetic ST".into(),
                reason: "Synthetic opening balance".into(),
                evidence: "Synthetic fixture".into(),
            },
            library.revision().unwrap(),
            ST,
        )
        .unwrap();

    // Keep an opaque existing field in the pre-upgrade record to check byte-preserving migration.
    let mut record: Value = serde_json::from_str(&record_data(&path, &id)).unwrap();
    record["private_fields"]["MigrationSentinel"] = json!({"opaque": ["keep", 9]});
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![record.to_string(), id],
        )
        .unwrap();
    (library, id)
}

fn record_data(path: &Path, id: &str) -> String {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
}

fn revision(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT revision FROM library", [], |row| row.get(0))
        .unwrap()
}

fn user_version(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn table_exists(path: &Path, table: &str) -> bool {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )
        .unwrap()
}

fn audit_count(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
        .unwrap()
}

fn schema9_backup(directory: &Path) -> PathBuf {
    let paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("pre-schema9-")
        })
        .collect();
    assert_eq!(paths.len(), 1, "expected one pre-schema9 backup");
    paths.into_iter().next().unwrap()
}

/// Reconstruct a schema8 file from the current schema9 layout, replacing its newer token
/// guard with the schema8 complementary-token guard before exercising the real upgrader.
fn downgrade_current_fixture_to_schema8(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "DROP TRIGGER library_revision_guard_before_update;
             DROP TABLE mixed_advancement_receipts;
             PRAGMA user_version=8;",
        )
        .unwrap();
    let current_revision: i64 = connection
        .query_row("SELECT revision FROM library", [], |row| row.get(0))
        .unwrap();
    let next_revision = current_revision + 1;
    connection
        .execute(
            "UPDATE purchase_write_guard SET next_revision=?1,schema_revision=?2 WHERE singleton=1",
            params![next_revision, !next_revision],
        )
        .unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER library_revision_guard_before_update
             BEFORE UPDATE OF revision ON library
             WHEN NEW.revision != COALESCE((SELECT next_revision FROM purchase_write_guard WHERE singleton=1), -1)
               OR ~NEW.revision != COALESCE((SELECT schema_revision FROM purchase_write_guard WHERE singleton=1), 1)
             BEGIN
                 SELECT RAISE(ABORT, 'schema8 revision guard');
             END;",
        )
        .unwrap();
}

fn power_request() -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: Uuid::new_v4().to_string(),
        power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(ClanRelation::InClan),
        affiliation_name: "Synthetic clan".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic next session".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic purchase".into(),
        policy_version: "synthetic-policy-v1".into(),
        policy_reference: "Synthetic local policy reference".into(),
        affiliation_evidence: "Synthetic affiliation evidence".into(),
        approval_reference: "Synthetic approval reference".into(),
        eligibility_evidence: "Synthetic eligibility evidence".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Synthetic qualified teacher evidence".into(),
        standard_cost_confirmed: true,
    }
}

#[test]
fn schema8_upgrade_keeps_record_and_xp_and_backs_up_the_original_schema8_file() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("upgrade.gvlib");
    let (library, id) = fixture(directory.path(), "upgrade.gvlib");
    drop(library);
    let original_record = record_data(&path, &id);
    let original_revision = revision(&path);
    let original_audits = audit_count(&path);
    downgrade_current_fixture_to_schema8(&path);
    assert_eq!(user_version(&path), 8);
    assert!(!table_exists(&path, "mixed_advancement_receipts"));

    let upgraded = Library::open(&path).unwrap();
    assert_eq!(user_version(&path), 9);
    assert!(table_exists(&path, "mixed_advancement_receipts"));
    assert_eq!(upgraded.revision().unwrap(), original_revision + 1);
    assert_eq!(record_data(&path, &id), original_record);
    assert_eq!(
        upgraded.xp_view(&id, ST).unwrap().unspent.as_deref(),
        Some("20.25")
    );
    assert_eq!(audit_count(&path), original_audits + 1);
    assert_eq!(
        Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM audit WHERE operation='schema9_migration'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );

    let backup_path = schema9_backup(directory.path());
    assert_eq!(user_version(&backup_path), 8);
    assert!(!table_exists(&backup_path, "mixed_advancement_receipts"));
    assert_eq!(revision(&backup_path), original_revision);
    assert_eq!(record_data(&backup_path, &id), original_record);
}

#[test]
fn an_already_open_schema8_writer_cannot_use_the_complement_only_token_after_upgrade() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("old-writer.gvlib");
    let (library, id) = fixture(directory.path(), "old-writer.gvlib");
    drop(library);
    downgrade_current_fixture_to_schema8(&path);
    let mut old_schema8 = Connection::open(&path).unwrap();

    let mut current = Library::open(&path).unwrap();
    let current_revision = current.revision().unwrap();
    let next_revision = current_revision + 1;
    {
        let transaction = old_schema8.transaction().unwrap();
        // This is the schema8 writer's complete token update; schema9 requires the extra +9.
        transaction
            .execute(
                "UPDATE purchase_write_guard SET next_revision=?1,schema_revision=?2 WHERE singleton=1",
                params![next_revision, !next_revision],
            )
            .unwrap();
        assert!(
            transaction
                .execute(
                    "UPDATE library SET revision=?1 WHERE revision=?2",
                    params![next_revision, current_revision],
                )
                .is_err()
        );
    }
    assert_eq!(current.revision().unwrap(), current_revision);
    current
        .create_character("Risen", "Current schema9 writer", current_revision, ST)
        .unwrap();
    assert_eq!(current.revision().unwrap(), current_revision + 1);
    assert_eq!(
        current.record(&id, ST).unwrap().name,
        "Synthetic migration subject"
    );
}

#[test]
fn failed_schema9_audit_rolls_back_migration_and_keeps_the_schema8_backup() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("failed-upgrade.gvlib");
    let (library, id) = fixture(directory.path(), "failed-upgrade.gvlib");
    drop(library);
    let original_record = record_data(&path, &id);
    let original_revision = revision(&path);
    let original_audits = audit_count(&path);
    downgrade_current_fixture_to_schema8(&path);
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_schema9_audit BEFORE INSERT ON audit
             WHEN NEW.operation='schema9_migration'
             BEGIN SELECT RAISE(ABORT, 'synthetic schema9 audit failure'); END;",
        )
        .unwrap();

    assert!(Library::open(&path).is_err());
    assert_eq!(user_version(&path), 8);
    assert!(!table_exists(&path, "mixed_advancement_receipts"));
    assert_eq!(revision(&path), original_revision);
    assert_eq!(audit_count(&path), original_audits);
    assert_eq!(record_data(&path, &id), original_record);
    let backup_path = schema9_backup(directory.path());
    assert_eq!(user_version(&backup_path), 8);
    assert!(!table_exists(&backup_path, "mixed_advancement_receipts"));
    assert_eq!(revision(&backup_path), original_revision);
    assert_eq!(record_data(&backup_path, &id), original_record);
}

#[test]
fn restore_from_schema8_clears_mixed_receipts_and_corrupt_schema9_restore_is_atomic() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("restore.gvlib");
    let (library, id) = fixture(directory.path(), "restore.gvlib");
    drop(library);
    let original_record = record_data(&path, &id);
    let original_revision = revision(&path);
    downgrade_current_fixture_to_schema8(&path);

    let mut current = Library::open(&path).unwrap();
    let schema8_backup = schema9_backup(directory.path());
    let request = power_request();
    let quote = current
        .quote_mixed_advancement(
            &id,
            &[mixed::Request::Power(Box::new(request))],
            current.revision().unwrap(),
            ST,
        )
        .unwrap();
    let saved = current.commit_mixed_advancement(&quote, ST).unwrap();
    assert_eq!(
        current.mixed_advancement_history(&id, ST).unwrap(),
        vec![saved.clone()]
    );
    let migrated_revision = current.revision().unwrap();
    let mixed_record = record_data(&path, &id);
    let mixed_audits = audit_count(&path);
    let mixed_purchases = current.purchase_receipts(&id, ST).unwrap();

    let schema9_backup = directory.path().join("schema9-with-mixed.gvbackup");
    current.backup(&schema9_backup, ST).unwrap();
    assert_eq!(user_version(&schema9_backup), 9);
    assert_eq!(
        Connection::open(&schema9_backup)
            .unwrap()
            .execute("UPDATE mixed_advancement_receipts SET data='{}'", [])
            .unwrap(),
        1
    );

    assert!(
        current
            .restore(&schema9_backup, migrated_revision, ST)
            .is_err()
    );
    assert_eq!(current.revision().unwrap(), migrated_revision);
    assert_eq!(record_data(&path, &id), mixed_record);
    assert_eq!(audit_count(&path), mixed_audits);
    assert_eq!(current.purchase_receipts(&id, ST).unwrap(), mixed_purchases);
    assert_eq!(
        current.mixed_advancement_history(&id, ST).unwrap(),
        vec![saved.clone()]
    );
    assert_eq!(
        Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM mixed_advancement_receipts",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );

    current
        .restore(&schema8_backup, current.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(user_version(&path), 9);
    assert!(table_exists(&path, "mixed_advancement_receipts"));
    assert_eq!(
        Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM mixed_advancement_receipts",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert!(
        current
            .mixed_advancement_history(&id, ST)
            .unwrap()
            .is_empty()
    );
    assert!(current.purchase_receipts(&id, ST).unwrap().is_empty());
    assert_eq!(record_data(&path, &id), original_record);
    assert_eq!(revision(&schema8_backup), original_revision);
    assert_eq!(
        current.xp_view(&id, ST).unwrap().unspent.as_deref(),
        Some("20.25")
    );
}
