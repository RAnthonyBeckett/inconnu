use grapevine_workbench::{
    cancellations::*,
    model::Role,
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{PurchaseQuote, PurchaseRequest, TeacherRule},
    store::Library,
    xp::{Baseline, XpCommand, XpKind},
};
use rusqlite::Connection;
use serde::Deserialize;
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

fn bought() -> (TempDir, Library, String, PurchaseQuote) {
    let (d, mut l, id) = fixture("10.25");
    let q = l
        .quote_purchase(
            &id,
            &request(ANIMALISM_FERAL_WHISPERS_ID),
            l.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    l.commit_purchase(&q, Role::Storyteller).unwrap();
    (d, l, id, q)
}
fn cancel() -> CancellationRequest {
    CancellationRequest {
        id: Uuid::new_v4().to_string(),
        day: "2026-01-03".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic correction".into(),
        evidence: "Synthetic approval".into(),
    }
}
fn snapshot(l: &Library, id: &str) -> (i64, String, String, i64) {
    let c = Connection::open(&l.path).unwrap();
    (
        l.revision().unwrap(),
        c.query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
            .unwrap(),
        c.query_row("SELECT data FROM purchase_receipts", [], |r| r.get(0))
            .unwrap(),
        c.query_row("SELECT count(*) FROM audit", [], |r| r.get(0))
            .unwrap(),
    )
}
#[test]
fn cancellation_is_atomic_exact_retryable_and_original_receipt_is_retained() {
    let (_d, mut l, id, original) = bought();
    let q = l
        .quote_cancellation(
            &original.request.id,
            &cancel(),
            l.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(
        (&q.before_unspent, &q.after_unspent, &q.refund),
        (&"6.25".to_string(), &"10.25".to_string(), &"4".to_string())
    );
    let saved = l.commit_cancellation(&q, Role::Storyteller).unwrap();
    let rev = l.revision().unwrap();
    assert_eq!(saved, l.commit_cancellation(&q, Role::Storyteller).unwrap());
    assert_eq!(rev, l.revision().unwrap());
    let receipts = l.purchase_receipts(&id, Role::Storyteller).unwrap();
    assert_eq!(receipts[0].quote, original);
    assert_eq!(receipts[0].cancellation.as_ref(), Some(&saved));
    let v = l.xp_view(&id, Role::Storyteller).unwrap();
    assert_eq!(v.unspent.as_deref(), Some("10.25"));
    let ledger = v.ledger.unwrap();
    assert_eq!(ledger.entries.len(), 2);
    assert_eq!(ledger.entries[0].command.kind, XpKind::Spend);
    assert_eq!(ledger.entries[1].command.kind, XpKind::Reversal);
    assert!(
        !v.findings
            .iter()
            .any(|f| f.code == "PURCHASE_TRAIT_DRIFT" || f.code == "PURCHASE_SPEND_REVERSED")
    );
    assert!(
        l.quote_cancellation(&original.request.id, &cancel(), rev, Role::Storyteller)
            .is_err()
    );
    let mut conflict = q.clone();
    conflict.request.reason = "Different".into();
    assert!(l.commit_cancellation(&conflict, Role::Storyteller).is_err());
    assert!(l.purchase_receipts(&id, Role::Player).is_err());
    assert!(l.commit_cancellation(&q, Role::Player).is_err());
    let manual = XpCommand {
        id: q.request.id.clone(),
        kind: XpKind::Award,
        amount: "1".into(),
        day: "2026-01-04".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic".into(),
        evidence: "Synthetic".into(),
        reversal_of: None,
        cost: None,
    };
    assert!(l.post_xp(&id, &manual, rev, Role::Storyteller).is_err());
}
#[test]
fn failed_writes_forged_and_stale_cancellation_leave_all_state_unchanged() {
    for (table, event) in [
        ("records", "UPDATE"),
        ("purchase_receipts", "UPDATE"),
        ("audit", "INSERT"),
    ] {
        let (_d, mut l, id, p) = bought();
        let q = l
            .quote_cancellation(
                &p.request.id,
                &cancel(),
                l.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap();
        let before = snapshot(&l, &id);
        let c = Connection::open(&l.path).unwrap();
        c.execute_batch(&format!("CREATE TRIGGER fail_cancel BEFORE {event} ON {table} BEGIN SELECT RAISE(ABORT,'synthetic cancellation failure'); END;")).unwrap();
        assert!(l.commit_cancellation(&q, Role::Storyteller).is_err());
        assert_eq!(before, snapshot(&l, &id));
    }
    let (_d, mut l, id, p) = bought();
    let q = l
        .quote_cancellation(
            &p.request.id,
            &cancel(),
            l.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    let before = snapshot(&l, &id);
    let mut forged = q.clone();
    forged.refund = "100".into();
    assert!(l.commit_cancellation(&forged, Role::Storyteller).is_err());
    assert_eq!(before, snapshot(&l, &id));
    l.create_character("Vampire", "Other", l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let before = snapshot(&l, &id);
    assert!(l.commit_cancellation(&q, Role::Storyteller).is_err());
    assert_eq!(before, snapshot(&l, &id));
}
#[test]
fn cancellation_survives_restore_and_legacy_receipt_reader_does_not_need_new_fields() {
    #[derive(Deserialize)]
    struct LegacyReceipt {
        quote: PurchaseQuote,
        committed_revision: i64,
    }
    let (d, mut l, id, p) = bought();
    let active = d.path().join("active.gvbackup");
    l.backup(&active, Role::Storyteller).unwrap();
    let q = l
        .quote_cancellation(
            &p.request.id,
            &cancel(),
            l.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    l.commit_cancellation(&q, Role::Storyteller).unwrap();
    let canceled = d.path().join("cancelled.gvbackup");
    l.backup(&canceled, Role::Storyteller).unwrap();
    let c = Connection::open(&l.path).unwrap();
    let raw: String = c
        .query_row("SELECT data FROM purchase_receipts", [], |r| r.get(0))
        .unwrap();
    let old: LegacyReceipt = serde_json::from_str(&raw).unwrap();
    assert_eq!(old.quote, p);
    assert_eq!(old.committed_revision, p.revision + 1);
    drop(c);
    l.restore(&active, l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert!(
        l.purchase_receipts(&id, Role::Storyteller).unwrap()[0]
            .cancellation
            .is_none()
    );
    l.restore(&canceled, l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert!(
        l.purchase_receipts(&id, Role::Storyteller).unwrap()[0]
            .cancellation
            .is_some()
    );
    assert_eq!(
        l.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("10.25")
    );
    let path = l.path.clone();
    drop(l);
    let reopened = Library::open(&path).unwrap();
    assert!(
        reopened.purchase_receipts(&id, Role::Storyteller).unwrap()[0]
            .cancellation
            .is_some()
    );
}
