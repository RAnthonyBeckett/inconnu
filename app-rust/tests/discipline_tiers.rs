use grapevine_workbench::{
    model::Role,
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{PurchaseRequest, TeacherRule},
    store::Library,
    xp::Baseline,
};
use rusqlite::Connection;
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

#[test]
fn mixed_tier_draft_prices_affiliation_and_applies_exact_cumulative_debits() {
    let (_dir, mut lib, id) = fixture("40");
    let mut basic = request(ANIMALISM_FERAL_WHISPERS_ID);
    basic.relation = Some(ClanRelation::InClan);
    let intermediate = request("met.lotn-revised.animalism.intermediate.quell_the_beast");
    let advanced = request("met.lotn-revised.animalism.advanced.drawing_out_the_beast");
    let requests = vec![basic, intermediate, advanced];
    let before = raw(&lib.path, &id);
    let quote = lib
        .quote_purchase_batch(&id, &requests, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(quote.total_spend, "20");
    assert_eq!(quote.after_unspent, "20");
    assert_eq!(
        quote
            .steps
            .iter()
            .map(|step| step.price.price.total.as_str())
            .collect::<Vec<_>>(),
        ["3", "7", "10"]
    );
    let draft = lib
        .save_advancement_draft(&id, &requests, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(raw(&lib.path, &id), before);
    let review = lib
        .review_advancement_draft(&draft, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    lib.apply_advancement_draft(&review, Role::Storyteller)
        .unwrap();
    assert_eq!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("20")
    );
    let saved = lib.purchase_receipts(&id, Role::Storyteller).unwrap();
    assert_eq!(saved.len(), 3);
    assert!(
        saved
            .iter()
            .any(|receipt| receipt.quote.price.tier
                == grapevine_workbench::pricing::MetTier::Advanced)
    );
}

#[test]
fn higher_tier_evidence_and_cumulative_shortfall_still_block_without_writes() {
    let (_dir, mut lib, id) = fixture("16");
    let before = raw(&lib.path, &id);
    let revision = lib.revision().unwrap();
    let mut advanced = request("met.lotn-revised.animalism.advanced.drawing_out_the_beast");
    advanced.teacher_evidence.clear();
    assert!(
        lib.quote_purchase_batch(&id, &[advanced.clone()], revision, Role::Storyteller)
            .is_err()
    );
    advanced.teacher_evidence = "Synthetic qualified teacher".into();
    let intermediate = request("met.lotn-revised.animalism.intermediate.quell_the_beast");
    assert!(
        lib.quote_purchase_batch(&id, &[intermediate, advanced], revision, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(lib.revision().unwrap(), revision);
    assert!(
        lib.purchase_receipts(&id, Role::Storyteller)
            .unwrap()
            .is_empty()
    );
    assert!(
        lib.save_advancement_draft(&id, &[], revision, Role::Storyteller)
            .is_err()
    );
}
