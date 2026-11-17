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
fn plan(lib: &Library, id: &str) -> grapevine_workbench::purchase_batch::Quote {
    lib.quote_purchase_batch(id, &requests(), lib.revision().unwrap(), Role::Storyteller)
        .unwrap()
}
#[test]
fn cumulative_plan_is_readonly_then_saves_one_revision_and_all_receipts() {
    let (_dir, mut lib, id) = fixture("20.25");
    let before = raw(&lib.path, &id);
    let revision = lib.revision().unwrap();
    let audits = audit_count(&lib.path);
    let quote = plan(&lib, &id);
    assert_eq!(quote.total_spend, "7");
    assert_eq!(quote.after_unspent, "13.25");
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(audit_count(&lib.path), audits);
    let saved = lib
        .commit_purchase_batch(&quote, Role::Storyteller)
        .unwrap();
    assert_eq!(saved.receipts.len(), 2);
    assert_eq!(lib.revision().unwrap(), revision + 1);
    assert_eq!(audit_count(&lib.path), audits + 1);
    assert_eq!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("13.25")
    );
    let stored: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    assert_eq!(stored["xp_ledger"]["entries"].as_array().unwrap().len(), 2);
    assert!(
        stored["xp_ledger"]["entries"][1]["recorded_unix_seconds"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        lib.purchase_receipts(&id, Role::Storyteller).unwrap().len(),
        2
    );
    let cancellation = grapevine_workbench::cancellations::CancellationRequest {
        id: Uuid::new_v4().to_string(),
        day: "2026-01-04".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic cancellation".into(),
        evidence: "Synthetic review".into(),
    };
    for receipt in &saved.receipts {
        assert_eq!(
            lib.quote_cancellation(
                &receipt.quote.request.id,
                &cancellation,
                lib.revision().unwrap(),
                Role::Storyteller
            )
            .unwrap_err()
            .code,
            "BATCH_CANCELLATION_UNSUPPORTED"
        );
    }
    let findings = lib.xp_view(&id, Role::Storyteller).unwrap().findings;
    assert!(
        !findings
            .iter()
            .any(|finding| finding.code.starts_with("PURCHASE_")),
        "{findings:?}"
    );
    drop(lib);
    let mut reopened = Library::open(&_dir.path().join("purchases.gvlib")).unwrap();
    assert_eq!(
        reopened
            .commit_purchase_batch(&quote, Role::Storyteller)
            .unwrap(),
        saved
    );
}
#[test]
fn cumulative_shortfall_invalid_later_item_and_forgery_never_write() {
    let (_dir, lib, id) = fixture("6");
    let before = raw(&lib.path, &id);
    let revision = lib.revision().unwrap();
    assert!(
        lib.quote_purchase_batch(&id, &requests(), revision, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(lib.revision().unwrap(), revision);
    let (_dir, mut funded, id) = fixture("20.25");
    let before = raw(&funded.path, &id);
    let mut bad = requests();
    bad[1].teacher_evidence.clear();
    assert!(
        funded
            .quote_purchase_batch(&id, &bad, funded.revision().unwrap(), Role::Storyteller)
            .is_err()
    );
    let mut quote = plan(&funded, &id);
    quote.steps[1].after_unspent = "999".into();
    assert!(
        funded
            .commit_purchase_batch(&quote, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&funded.path, &id), before);
    assert!(
        funded
            .purchase_receipts(&id, Role::Storyteller)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn database_failure_rolls_back_character_receipts_revision_and_audit() {
    let (_dir, mut lib, id) = fixture("20.25");
    let quote = plan(&lib, &id);
    let before = raw(&lib.path, &id);
    let revision = lib.revision().unwrap();
    let audits = audit_count(&lib.path);
    let connection = Connection::open(&lib.path).unwrap();
    connection.execute_batch("CREATE TRIGGER synthetic_receipt_failure BEFORE INSERT ON purchase_receipts WHEN (SELECT COUNT(*) FROM purchase_receipts)=1 BEGIN SELECT RAISE(ABORT,'synthetic second receipt failure'); END;").unwrap();
    assert!(
        lib.commit_purchase_batch(&quote, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(audit_count(&lib.path), audits);
    assert!(
        lib.purchase_receipts(&id, Role::Storyteller)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn stale_plan_rebases_explicitly_on_new_awards_and_exact_retry_does_not_spend_again() {
    let (_dir, mut lib, id) = fixture("20.25");
    let quote = plan(&lib, &id);
    let requests = requests();
    let award = XpCommand {
        id: Uuid::new_v4().to_string(),
        kind: XpKind::Award,
        amount: "2".into(),
        day: "2026-01-02".into(),
        actor: "Synthetic ST".into(),
        reason: "New session".into(),
        evidence: "Synthetic attendance".into(),
        reversal_of: None,
        cost: None,
    };
    lib.post_xp(&id, &award, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let before = raw(&lib.path, &id);
    assert_eq!(
        lib.commit_purchase_batch(&quote, Role::Storyteller)
            .unwrap_err()
            .code,
        "STALE_PURCHASE"
    );
    assert_eq!(raw(&lib.path, &id), before);
    let fresh = lib
        .quote_purchase_batch(&id, &requests, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(fresh.before_unspent, "22.25");
    assert_eq!(fresh.after_unspent, "15.25");
    let saved = lib
        .commit_purchase_batch(&fresh, Role::Storyteller)
        .unwrap();
    let mut later = award.clone();
    later.id = Uuid::new_v4().to_string();
    lib.post_xp(&id, &later, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let before = raw(&lib.path, &id);
    let rev = lib.revision().unwrap();
    assert_eq!(
        lib.commit_purchase_batch(&fresh, Role::Storyteller)
            .unwrap(),
        saved
    );
    assert_eq!(lib.revision().unwrap(), rev);
    assert_eq!(raw(&lib.path, &id), before);
    let mut conflict = fresh.clone();
    conflict.steps[0].request.reason = "Changed declaration".into();
    assert_eq!(
        lib.commit_purchase_batch(&conflict, Role::Storyteller)
            .unwrap_err()
            .code,
        "BATCH_ID_CONFLICT"
    );
}
#[test]
fn roles_draft_and_test_copies_and_overlapping_ids_are_rejected() {
    let (_dir, mut lib, id) = fixture("20.25");
    let quote = plan(&lib, &id);
    assert!(
        lib.quote_purchase_batch(&id, &requests(), lib.revision().unwrap(), Role::Player)
            .is_err()
    );
    assert!(lib.commit_purchase_batch(&quote, Role::Player).is_err());
    for kind in [BuildKind::Draft, BuildKind::Test] {
        let build = lib
            .create_build(&id, kind, lib.revision().unwrap(), Role::Storyteller)
            .unwrap();
        assert!(
            lib.quote_purchase_batch(
                &build,
                &requests(),
                lib.revision().unwrap(),
                Role::Storyteller
            )
            .is_err()
        );
    }
    let quote = plan(&lib, &id);
    let single = lib
        .quote_purchase(
            &id,
            &quote.steps[0].request,
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    lib.commit_purchase(&single, Role::Storyteller).unwrap();
    let before = raw(&lib.path, &id);
    assert_eq!(
        lib.commit_purchase_batch(&quote, Role::Storyteller)
            .unwrap_err()
            .code,
        "BATCH_ID_CONFLICT"
    );
    assert_eq!(raw(&lib.path, &id), before);
}
#[test]
fn legacy_disciplines_are_preserved_and_ambiguous_or_owned_labels_block_purchase() {
    let (_dir, mut lib, id) = fixture("20.25");
    let mut record: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    // Reconstruct the old import shape rather than retain the new-character
    // empty public list, which intentionally is a canonical list already.
    record["public_fields"]
        .as_object_mut()
        .unwrap()
        .remove("DisciplineList");
    let legacy = json!({"Name":"Disciplines","Traits":[{"Name":"Unmapped Old Power","Value":"2","Note":"Keep this note"}],"Custom":"Keep metadata"});
    record["private_fields"]["UnmappedTraitLists"] = json!([legacy.clone()]);
    record["private_fields"]["UnknownField"] = json!({"opaque":true});
    let conn = Connection::open(&lib.path).unwrap();
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    let quote = plan(&lib, &id);
    lib.commit_purchase_batch(&quote, Role::Storyteller)
        .unwrap();
    let after: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    assert_eq!(
        after["private_fields"]["UnmappedTraitLists"],
        json!([legacy.clone()])
    );
    assert_eq!(
        after["private_fields"]["DisciplineList"]["Traits"][0],
        legacy["Traits"][0]
    );
    assert_eq!(
        after["private_fields"]["DisciplineList"]["Custom"],
        "Keep metadata"
    );
    assert_eq!(
        after["private_fields"]["UnknownField"],
        json!({"opaque":true})
    );
    // A new canonical list must not hide ambiguous duplicate legacy lists.
    record["private_fields"]["UnmappedTraitLists"] = json!([legacy.clone(), legacy]);
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    assert!(
        lib.quote_purchase_batch(&id, &requests(), lib.revision().unwrap(), Role::Storyteller)
            .is_err()
    );
}

#[test]
fn missing_group_receipt_is_reported_and_retry_never_partially_reapplies() {
    let (_dir, mut lib, id) = fixture("20.25");
    let quote = plan(&lib, &id);
    lib.commit_purchase_batch(&quote, Role::Storyteller)
        .unwrap();
    let conn = Connection::open(&lib.path).unwrap();
    conn.execute(
        "DELETE FROM purchase_receipts WHERE id=?1",
        [&quote.steps[0].request.id],
    )
    .unwrap();
    let before = raw(&lib.path, &id);
    let revision = lib.revision().unwrap();
    assert_eq!(
        lib.purchase_receipts(&id, Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_BATCH_INVALID"
    );
    assert_eq!(
        lib.commit_purchase_batch(&quote, Role::Storyteller)
            .unwrap_err()
            .code,
        "BATCH_ID_CONFLICT"
    );
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(raw(&lib.path, &id), before);
}
