use grapevine_workbench::{
    model::{BuildKind, Role},
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::{ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID},
    purchases::{PurchaseRequest, TeacherRule},
    store::Library,
    xp::{Baseline, XpCommand, XpKind},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

fn baseline(unspent: &str) -> Baseline {
    Baseline {
        day: "2026-01-01".into(),
        earned: unspent.into(),
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

fn requests() -> Vec<PurchaseRequest> {
    let mut first = request(ANIMALISM_FERAL_WHISPERS_ID);
    first.relation = Some(ClanRelation::InClan);
    vec![first, request(ANIMALISM_BECKONING_ID)]
}

fn save(lib: &mut Library, id: &str) -> String {
    lib.save_advancement_draft(id, &requests(), lib.revision().unwrap(), Role::Storyteller)
        .unwrap()
}
#[test]
fn save_reopen_review_apply_and_retry_preserve_live_identity_and_draft_ledger() {
    let (dir, mut lib, id) = fixture("20.25");
    let before = raw(&lib.path, &id);
    let draft_id = save(&mut lib, &id);
    assert_eq!(raw(&lib.path, &id), before);
    let draft_before: Value = serde_json::from_str(&raw(&lib.path, &draft_id)).unwrap();
    drop(lib);
    let mut lib = Library::open(&dir.path().join("purchases.gvlib")).unwrap();
    let review = lib
        .review_advancement_draft(&draft_id, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert!(!review.applied);
    assert_eq!(review.quote.total_spend, "7");
    let saved = lib
        .apply_advancement_draft(&review, Role::Storyteller)
        .unwrap();
    let live: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    let old: Value = serde_json::from_str(&before).unwrap();
    assert_eq!(live["id"], old["id"]);
    assert_eq!(live["name"], old["name"]);
    assert_eq!(live["original"], old["original"]);
    let draft_after: Value = serde_json::from_str(&raw(&lib.path, &draft_id)).unwrap();
    assert_eq!(draft_after["xp_ledger"], draft_before["xp_ledger"]);
    assert_eq!(draft_after["build"], draft_before["build"]);
    assert_eq!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("13.25")
    );
    let revision = lib.revision().unwrap();
    let after = raw(&lib.path, &id);
    let audit = audit_count(&lib.path);
    assert_eq!(
        lib.apply_advancement_draft(&review, Role::Storyteller)
            .unwrap(),
        saved
    );
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(raw(&lib.path, &id), after);
    assert_eq!(audit_count(&lib.path), audit);
    let reopened = lib
        .review_advancement_draft(&draft_id, revision, Role::Storyteller)
        .unwrap();
    assert!(reopened.applied);
    // An already applied plan remains an idempotent retry even after another award.
    lib.post_xp(&id, &award(), revision, Role::Storyteller)
        .unwrap();
    let later = raw(&lib.path, &id);
    let later_revision = lib.revision().unwrap();
    assert_eq!(
        lib.apply_advancement_draft(&review, Role::Storyteller)
            .unwrap(),
        saved
    );
    assert_eq!(raw(&lib.path, &id), later);
    assert_eq!(lib.revision().unwrap(), later_revision);
}
fn award() -> XpCommand {
    XpCommand {
        id: Uuid::new_v4().to_string(),
        kind: XpKind::Award,
        amount: "2".into(),
        day: "2026-01-02".into(),
        actor: "Synthetic ST".into(),
        reason: "New session".into(),
        evidence: "Synthetic attendance".into(),
        reversal_of: None,
        cost: None,
    }
}
#[test]
fn changed_source_or_draft_blocks_without_silent_rebase() {
    let (_dir, mut lib, id) = fixture("20.25");
    let draft_id = save(&mut lib, &id);
    let review = lib
        .review_advancement_draft(&draft_id, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    lib.post_xp(&id, &award(), lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let before = raw(&lib.path, &id);
    let revision = lib.revision().unwrap();
    assert_eq!(
        lib.review_advancement_draft(&draft_id, revision, Role::Storyteller)
            .unwrap_err()
            .code,
        "DRAFT_SOURCE_CHANGED"
    );
    assert!(
        lib.apply_advancement_draft(&review, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(lib.revision().unwrap(), revision);
    let new_draft = save(&mut lib, &id);
    let mut record: Value = serde_json::from_str(&raw(&lib.path, &new_draft)).unwrap();
    record["private_fields"]["Notes"] = json!("Manual draft edit");
    let conn = Connection::open(&lib.path).unwrap();
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), new_draft],
    )
    .unwrap();
    assert_eq!(
        lib.review_advancement_draft(&new_draft, lib.revision().unwrap(), Role::Storyteller)
            .unwrap_err()
            .code,
        "ADVANCEMENT_DRAFT_CHANGED"
    );
    assert_eq!(raw(&lib.path, &id), before);
}
#[test]
fn marker_failure_rolls_back_live_xp_traits_receipts_and_audit() {
    let (_dir, mut lib, id) = fixture("20.25");
    let draft_id = save(&mut lib, &id);
    let review = lib
        .review_advancement_draft(&draft_id, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let before = raw(&lib.path, &id);
    let draft_before = raw(&lib.path, &draft_id);
    let revision = lib.revision().unwrap();
    let audit = audit_count(&lib.path);
    Connection::open(&lib.path).unwrap().execute_batch(&format!("CREATE TRIGGER synthetic_draft_marker_failure BEFORE UPDATE ON records WHEN NEW.id='{draft_id}' BEGIN SELECT RAISE(ABORT,'synthetic marker failure'); END;")).unwrap();
    assert!(
        lib.apply_advancement_draft(&review, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(raw(&lib.path, &draft_id), draft_before);
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(audit_count(&lib.path), audit);
    assert!(
        lib.purchase_receipts(&id, Role::Storyteller)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn roles_test_copies_and_unrelated_edits_keep_boundaries() {
    let (_dir, mut lib, id) = fixture("20.25");
    let revision = lib.revision().unwrap();
    assert!(
        lib.save_advancement_draft(&id, &requests(), revision, Role::Player)
            .is_err()
    );
    let draft_id = save(&mut lib, &id);
    assert!(
        lib.review_advancement_draft(&draft_id, lib.revision().unwrap(), Role::Player)
            .is_err()
    );
    let review = lib
        .review_advancement_draft(&draft_id, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert!(lib.apply_advancement_draft(&review, Role::Player).is_err());
    lib.create_character(
        "Mortal",
        "Unrelated",
        lib.revision().unwrap(),
        Role::Storyteller,
    )
    .unwrap();
    assert!(
        lib.apply_advancement_draft(&review, Role::Storyteller)
            .is_err()
    );
    let fresh = lib
        .review_advancement_draft(&draft_id, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(fresh.quote.record_hash, review.quote.record_hash);
    assert_ne!(fresh.quote.revision, review.quote.revision);
    let test = lib
        .create_build(
            &id,
            BuildKind::Test,
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert!(
        lib.review_advancement_draft(&test, lib.revision().unwrap(), Role::Storyteller)
            .is_err()
    );
    assert!(
        lib.save_advancement_draft(
            &test,
            &requests(),
            lib.revision().unwrap(),
            Role::Storyteller
        )
        .is_err()
    );
    let fresh = lib
        .review_advancement_draft(&draft_id, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    lib.apply_advancement_draft(&fresh, Role::Storyteller)
        .unwrap();
}
#[test]
fn sibling_drafts_cannot_apply_the_same_purchase_plan_twice() {
    let (_dir, mut lib, id) = fixture("20.25");
    let requests = requests();
    let first = lib
        .save_advancement_draft(&id, &requests, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let second = lib
        .save_advancement_draft(&id, &requests, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let review = lib
        .review_advancement_draft(&first, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    lib.apply_advancement_draft(&review, Role::Storyteller)
        .unwrap();
    let before = raw(&lib.path, &id);
    assert_eq!(
        lib.review_advancement_draft(&second, lib.revision().unwrap(), Role::Storyteller)
            .unwrap_err()
            .code,
        "DRAFT_SOURCE_CHANGED"
    );
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(
        lib.purchase_receipts(&id, Role::Storyteller).unwrap().len(),
        2
    );
}
