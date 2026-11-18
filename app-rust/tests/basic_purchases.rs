use grapevine_workbench::{
    cancellations::CancellationRequest,
    model::Role,
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::AnimalismPower,
    purchases::{PurchaseRequest, TeacherRule},
    store::Library,
    xp::Baseline,
};
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

#[test]
fn every_reviewed_tier_power_commits_its_own_family_and_cancels_exactly() {
    for power in AnimalismPower::ALL {
        let (_directory, mut library, id) = fixture("10.25");
        let input = request(power.stable_id());
        let quote = library
            .quote_purchase(&id, &input, 2, Role::Storyteller)
            .unwrap();
        assert_eq!(quote.price.power.family(), power.family());
        assert_eq!(
            quote.price.power_source.citation.printed_page,
            power.printed_page()
        );
        let expected = match power.tier() {
            grapevine_workbench::pricing::MetTier::Basic => "4",
            grapevine_workbench::pricing::MetTier::Intermediate => "7",
            grapevine_workbench::pricing::MetTier::Advanced => "10",
        };
        assert_eq!(quote.price.price.total, expected);
        let saved = library.commit_purchase(&quote, Role::Storyteller).unwrap();
        assert_eq!(saved.quote.request.power_id, power.stable_id());
        let audit = library.xp_view(&id, Role::Storyteller).unwrap();
        assert!(
            !audit
                .findings
                .iter()
                .any(|f| f.code == "AFFILIATION_COST_UNVERIFIED")
        );
        let ledger = audit.ledger.unwrap();
        assert_eq!(
            ledger.entries[0]
                .command
                .cost
                .as_ref()
                .unwrap()
                .power_family,
            format!("{} / Discipline", power.family())
        );
        let cancel = library
            .quote_cancellation(
                &input.id,
                &CancellationRequest {
                    id: Uuid::new_v4().to_string(),
                    day: "2026-01-03".into(),
                    actor: "Synthetic ST".into(),
                    reason: "Synthetic correction".into(),
                    evidence: "Synthetic correction evidence".into(),
                },
                3,
                Role::Storyteller,
            )
            .unwrap();
        library
            .commit_cancellation(&cancel, Role::Storyteller)
            .unwrap();
        let view = library.xp_view(&id, Role::Storyteller).unwrap();
        assert_eq!(view.unspent.as_deref(), Some("10.25"));
        assert_eq!(view.ledger.unwrap().entries.len(), 2);
        assert!(
            !view
                .findings
                .iter()
                .any(|f| f.code.starts_with("PURCHASE_") || f.code.starts_with("CANCEL_"))
        );
        assert!(
            library.purchase_receipts(&id, Role::Storyteller).unwrap()[0]
                .cancellation
                .is_some()
        );
    }
}

#[test]
fn new_family_review_reuse_and_receipt_survive_reopen_and_backup_restore() {
    let (d, mut library, id) = fixture("10.25");
    let power = AnimalismPower::ALL[2];
    let input = request(power.stable_id());
    let review = library
        .save_purchase_review(&id, &input, "Synthetic Basic policy", 2, Role::Storyteller)
        .unwrap();
    let mut reused = library
        .reuse_purchase_review(&id, &review.id, &input, 3, Role::Storyteller)
        .unwrap();
    assert!(
        library
            .quote_purchase(&id, &reused, 3, Role::Storyteller)
            .is_err()
    );
    reused.approval_reference = "Fresh synthetic approval".into();
    reused.teacher_evidence = "Fresh synthetic teacher ruling/evidence".into();
    let quote = library
        .quote_purchase(&id, &reused, 3, Role::Storyteller)
        .unwrap();
    let receipt = library.commit_purchase(&quote, Role::Storyteller).unwrap();
    assert_eq!(
        receipt.quote.request.review_id.as_deref(),
        Some(review.id.as_str())
    );
    let backup = d.path().join("new-family.gvbackup");
    library.backup(&backup, Role::Storyteller).unwrap();
    let path = library.path.clone();
    drop(library);
    let mut reopened = Library::open(&path).unwrap();
    assert_eq!(
        reopened.purchase_receipts(&id, Role::Storyteller).unwrap()[0],
        receipt
    );
    reopened
        .restore(&backup, reopened.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(
        reopened.purchase_receipts(&id, Role::Storyteller).unwrap()[0],
        receipt
    );
    assert!(reopened.purchase_receipts(&id, Role::Player).is_err());
}
