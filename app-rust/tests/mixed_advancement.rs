use grapevine_workbench::{
    mixed_advancement as mixed,
    model::{BuildKind, Role},
    pricing::{ClanRelation, PricingProfile},
    printing::Options,
    purchase_preview::{ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID},
    purchases::{PurchaseRequest, TeacherRule},
    sheet,
    store::Library,
    trait_advancement::{Request as TraitRequest, Target},
    trait_corrections,
    trait_pricing::Category,
    xp::{Baseline, XpCommand, XpKind},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

const ST: Role = Role::Storyteller;

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

fn fixture(ability: Option<&str>, unspent: &str) -> (TempDir, Library, String) {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("mixed.gvlib");
    let mut library = Library::create(&path, "Synthetic mixed advancement tests").unwrap();
    let id = library
        .create_character("Vampire", "Synthetic Vampire", 0, ST)
        .unwrap();
    if let Some(value) = ability {
        let mut edit = library.sheet_edit(&id, ST).unwrap();
        let mut list = sheet::empty_list("AbilityList");
        list["Traits"] = json!([{
            "Name": "Academics",
            "Value": value,
            "Note": "PRIVATE_ABILITY_NOTE"
        }]);
        edit.lists.insert("AbilityList".into(), list);
        library
            .save_sheet(&id, &edit, library.revision().unwrap(), ST)
            .unwrap();
    }
    library
        .initialize_xp(&id, &baseline(unspent), library.revision().unwrap(), ST)
        .unwrap();
    (directory, library, id)
}

fn power_request(power_id: &str, relation: ClanRelation) -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: Uuid::new_v4().to_string(),
        power_id: power_id.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(relation),
        affiliation_name: "Synthetic clan".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic next session".into(),
        actor: "PRIVATE_POWER_ACTOR".into(),
        reason: "PRIVATE_POWER_REASON".into(),
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

fn trait_request() -> TraitRequest {
    TraitRequest {
        id: Uuid::new_v4().to_string(),
        category: Category::Ability,
        target: Target::Trait {
            field: "AbilityList".into(),
            index: Some(0),
            name: "Academics".into(),
        },
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        show_on_player_sheet: false,
        day: "2026-01-02".into(),
        actor: "PRIVATE_TRAIT_ACTOR".into(),
        reason: "PRIVATE_TRAIT_REASON".into(),
        policy_version: "synthetic-policy-v1".into(),
        review_reference: "Synthetic maxima, approval and storyline review".into(),
        review_confirmed: true,
    }
}

fn power(request: PurchaseRequest) -> mixed::Request {
    mixed::Request::Power(Box::new(request))
}

fn advancement(request: TraitRequest) -> mixed::Request {
    mixed::Request::Trait(Box::new(request))
}

fn ordered_requests() -> Vec<mixed::Request> {
    vec![
        advancement(trait_request()),
        power(power_request(
            ANIMALISM_FERAL_WHISPERS_ID,
            ClanRelation::InClan,
        )),
        advancement(trait_request()),
        power(power_request(
            ANIMALISM_BECKONING_ID,
            ClanRelation::OutOfClan,
        )),
    ]
}

fn mixed_quote(lib: &Library, id: &str, requests: &[mixed::Request]) -> mixed::Quote {
    lib.quote_mixed_advancement(id, requests, lib.revision().unwrap(), ST)
        .unwrap()
}

fn raw(path: &Path, id: &str) -> String {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
}

fn audit_count(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
        .unwrap()
}

fn assert_no_advancement_audit_drift(lib: &Library, id: &str) {
    // The synthetic fixture intentionally lacks external policy and legacy-history proof.
    // Check that mixed steps add no receipt, cancellation, or ordinary-trait drift findings.
    let findings = lib.xp_view(id, ST).unwrap().findings;
    assert!(
        !findings.iter().any(|finding| {
            finding.code.starts_with("PURCHASE_")
                || finding.code.starts_with("CANCEL_")
                || finding.code == "TRAIT_ADVANCEMENT_SHEET_DRIFT"
        }),
        "{findings:#?}"
    );
}

fn step_hash(step: &mixed::Step) -> &str {
    match step {
        mixed::Step::Power(quote) => &quote.record_hash,
        mixed::Step::Trait(quote) => &quote.record_hash,
    }
}

fn award() -> XpCommand {
    XpCommand {
        id: Uuid::new_v4().to_string(),
        kind: XpKind::Award,
        amount: "2".into(),
        day: "2026-01-05".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic later session".into(),
        evidence: "Synthetic attendance".into(),
        reversal_of: None,
        cost: None,
    }
}

#[test]
fn ordered_plan_prices_each_increment_and_recomputes_actual_timestamp_hashes() {
    let (_directory, mut lib, id) = fixture(Some("4"), "20.25");
    let requests = ordered_requests();
    let before = raw(&lib.path, &id);
    let revision = lib.revision().unwrap();
    let audits = audit_count(&lib.path);

    let quote = mixed_quote(&lib, &id, &requests);
    assert_eq!(quote.total_spend, "10");
    assert_eq!(quote.before_unspent, "20.25");
    assert_eq!(quote.after_unspent, "10.25");
    assert_eq!(
        quote
            .steps
            .iter()
            .map(mixed::Step::cost)
            .collect::<Vec<_>>(),
        vec!["1", "3", "2", "4"]
    );
    assert_eq!(
        quote
            .steps
            .iter()
            .map(mixed::Step::balances)
            .collect::<Vec<_>>(),
        vec![
            ("20.25", "19.25"),
            ("19.25", "16.25"),
            ("16.25", "14.25"),
            ("14.25", "10.25"),
        ]
    );
    assert!(matches!(quote.steps[0], mixed::Step::Trait(_)));
    assert!(matches!(quote.steps[1], mixed::Step::Power(_)));
    assert!(matches!(quote.steps[2], mixed::Step::Trait(_)));
    assert!(matches!(quote.steps[3], mixed::Step::Power(_)));
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(audit_count(&lib.path), audits);

    let receipt = lib.commit_mixed_advancement(&quote, ST).unwrap();
    assert_eq!(receipt.quote, quote);
    assert_eq!(receipt.actual_steps.len(), 4);
    assert_eq!(lib.revision().unwrap(), revision + 1);
    assert_eq!(audit_count(&lib.path), audits + 1);
    assert_eq!(
        lib.xp_view(&id, ST).unwrap().unspent.as_deref(),
        Some("10.25")
    );
    assert_eq!(
        lib.record(&id, ST).unwrap().fields["AbilityList"]["Traits"][0]["Value"],
        "6"
    );
    assert_eq!(
        lib.record(&id, ST).unwrap().fields["DisciplineList"]["Traits"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        step_hash(&receipt.actual_steps[0]),
        step_hash(&quote.steps[0])
    );
    assert_ne!(
        step_hash(&receipt.actual_steps[1]),
        step_hash(&quote.steps[1])
    );
    let stored: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    let entries = stored["xp_ledger"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 4);
    assert!(entries.iter().all(|entry| {
        entry["recorded_unix_seconds"]
            .as_u64()
            .is_some_and(|seconds| seconds > 0)
    }));
}

#[test]
fn only_the_whole_group_retries_and_operation_ids_cannot_cross_boundaries() {
    let (_directory, mut lib, id) = fixture(Some("4"), "30.25");
    let trait_step = trait_request();
    let trait_id = trait_step.id.clone();
    let requests = vec![
        advancement(trait_step),
        power(power_request(
            ANIMALISM_FERAL_WHISPERS_ID,
            ClanRelation::InClan,
        )),
    ];
    let quote = mixed_quote(&lib, &id, &requests);

    let mut duplicate = requests.clone();
    let mixed::Request::Power(power) = &mut duplicate[1] else {
        unreachable!()
    };
    power.id = trait_id.clone();
    assert_eq!(
        lib.quote_mixed_advancement(&id, &duplicate, lib.revision().unwrap(), ST)
            .unwrap_err()
            .code,
        "MIXED_PLAN_ID"
    );

    let saved = lib.commit_mixed_advancement(&quote, ST).unwrap();

    let mut partial = quote.clone();
    partial.steps.pop();
    assert_eq!(
        lib.commit_mixed_advancement(&partial, ST).unwrap_err().code,
        "MIXED_PLAN_ID_CONFLICT"
    );

    lib.post_xp(&id, &award(), lib.revision().unwrap(), ST)
        .unwrap();
    let before_retry = raw(&lib.path, &id);
    let revision_before_retry = lib.revision().unwrap();
    assert_eq!(lib.commit_mixed_advancement(&quote, ST).unwrap(), saved);
    assert_eq!(lib.revision().unwrap(), revision_before_retry);
    assert_eq!(raw(&lib.path, &id), before_retry);

    // A new Power operation cannot reuse the ID already recorded by the mixed Trait step.
    let mut collision = power_request(ANIMALISM_BECKONING_ID, ClanRelation::InClan);
    collision.id = trait_id;
    let before_collision = raw(&lib.path, &id);
    assert!(
        lib.quote_purchase(&id, &collision, lib.revision().unwrap(), ST)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before_collision);

    // The reverse direction is also blocked: a mixed Trait cannot reuse a paid Power ID.
    let (_other_directory, mut other, other_id) = fixture(Some("4"), "20.25");
    let paid_power = power_request(ANIMALISM_FERAL_WHISPERS_ID, ClanRelation::InClan);
    let shared_id = paid_power.id.clone();
    let paid_quote = other
        .quote_purchase(&other_id, &paid_power, other.revision().unwrap(), ST)
        .unwrap();
    other.commit_purchase(&paid_quote, ST).unwrap();
    let mut trait_collision = trait_request();
    trait_collision.id = shared_id;
    let before_mixed_collision = raw(&other.path, &other_id);
    assert!(
        other
            .quote_mixed_advancement(
                &other_id,
                &[advancement(trait_collision)],
                other.revision().unwrap(),
                ST
            )
            .is_err()
    );
    assert_eq!(raw(&other.path, &other_id), before_mixed_collision);
}

#[test]
fn saved_draft_reopens_reviews_applies_once_and_rejects_edited_payloads() {
    let (directory, mut lib, id) = fixture(Some("4"), "20.25");
    let requests = vec![
        advancement(trait_request()),
        power(power_request(
            ANIMALISM_FERAL_WHISPERS_ID,
            ClanRelation::InClan,
        )),
    ];
    let draft_id = lib
        .save_mixed_advancement_draft(&id, &requests, lib.revision().unwrap(), ST)
        .unwrap();

    let second_draft_id = lib
        .save_mixed_advancement_draft(&id, &ordered_requests()[..2], lib.revision().unwrap(), ST)
        .unwrap();
    let mut edit = lib.sheet_edit(&second_draft_id, ST).unwrap();
    edit.lists.get_mut("AbilityList").unwrap()["Traits"][0]["Note"] = json!("EDITED_AFTER_PLAN");
    lib.save_sheet(&second_draft_id, &edit, lib.revision().unwrap(), ST)
        .unwrap();

    drop(lib);
    let mut reopened = Library::open(&directory.path().join("mixed.gvlib")).unwrap();
    assert_eq!(
        reopened
            .review_mixed_advancement_draft(&second_draft_id, reopened.revision().unwrap(), ST)
            .unwrap_err()
            .code,
        "ADVANCEMENT_DRAFT_CHANGED"
    );
    let review = reopened
        .review_mixed_advancement_draft(&draft_id, reopened.revision().unwrap(), ST)
        .unwrap();
    assert!(!review.applied);
    assert_eq!(review.quote.steps.len(), 2);
    let receipt = reopened.apply_mixed_advancement_draft(&review, ST).unwrap();
    assert_eq!(receipt.quote, review.quote);
    assert_eq!(
        reopened.xp_view(&id, ST).unwrap().unspent.as_deref(),
        Some("16.25")
    );

    reopened
        .post_xp(&id, &award(), reopened.revision().unwrap(), ST)
        .unwrap();
    let before_retry = raw(&reopened.path, &id);
    let revision_before_retry = reopened.revision().unwrap();
    assert_eq!(
        reopened.apply_mixed_advancement_draft(&review, ST).unwrap(),
        receipt
    );
    assert_eq!(reopened.revision().unwrap(), revision_before_retry);
    assert_eq!(raw(&reopened.path, &id), before_retry);
    assert_eq!(
        reopened.mixed_advancement_history(&id, ST).unwrap().len(),
        1
    );
}

#[test]
fn invalid_stale_forged_and_shortfall_plans_never_write_and_commit_rolls_back() {
    let (_directory, mut lib, id) = fixture(Some("4"), "10");
    let requests = ordered_requests();
    assert!(
        lib.quote_mixed_advancement(&id, &requests, lib.revision().unwrap(), Role::Player)
            .is_err()
    );

    for kind in [BuildKind::Draft, BuildKind::Test] {
        let build = lib
            .create_build(&id, kind, lib.revision().unwrap(), ST)
            .unwrap();
        assert!(
            lib.quote_mixed_advancement(&build, &requests, lib.revision().unwrap(), ST)
                .is_err()
        );
    }

    let quote = mixed_quote(&lib, &id, &requests);
    let mut forged = quote.clone();
    forged.total_spend = "1".into();
    let before_forgery = raw(&lib.path, &id);
    assert!(lib.commit_mixed_advancement(&forged, ST).is_err());
    assert_eq!(raw(&lib.path, &id), before_forgery);
    assert_eq!(lib.revision().unwrap(), quote.revision);

    lib.post_xp(&id, &award(), lib.revision().unwrap(), ST)
        .unwrap();
    let before_stale_commit = raw(&lib.path, &id);
    assert_eq!(
        lib.commit_mixed_advancement(&quote, ST).unwrap_err().code,
        "STALE_REVISION"
    );
    assert_eq!(raw(&lib.path, &id), before_stale_commit);

    let (_short_directory, short, short_id) = fixture(Some("4"), "7");
    let short_before = raw(&short.path, &short_id);
    let short_revision = short.revision().unwrap();
    assert!(
        short
            .quote_mixed_advancement(&short_id, &requests, short_revision, ST)
            .is_err()
    );
    assert_eq!(raw(&short.path, &short_id), short_before);
    assert_eq!(short.revision().unwrap(), short_revision);

    let (_rollback_directory, mut rollback, rollback_id) = fixture(Some("4"), "20.25");
    let rollback_quote = mixed_quote(&rollback, &rollback_id, &requests);
    let rollback_before = raw(&rollback.path, &rollback_id);
    let rollback_revision = rollback.revision().unwrap();
    let rollback_audits = audit_count(&rollback.path);
    let connection = Connection::open(&rollback.path).unwrap();
    connection.execute_batch(
        "CREATE TRIGGER synthetic_second_mixed_power_failure BEFORE INSERT ON purchase_receipts WHEN (SELECT COUNT(*) FROM purchase_receipts WHERE record_id=NEW.record_id)=1 BEGIN SELECT RAISE(ABORT,'synthetic second mixed power failure'); END;",
    ).unwrap();
    assert!(
        rollback
            .commit_mixed_advancement(&rollback_quote, ST)
            .is_err()
    );
    assert_eq!(raw(&rollback.path, &rollback_id), rollback_before);
    assert_eq!(rollback.revision().unwrap(), rollback_revision);
    assert_eq!(audit_count(&rollback.path), rollback_audits);
    assert!(
        rollback
            .purchase_receipts(&rollback_id, ST)
            .unwrap()
            .is_empty()
    );
    assert!(
        rollback
            .mixed_advancement_history(&rollback_id, ST)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn mixed_power_receipt_remains_standalone_cancellable() {
    let (_directory, mut lib, id) = fixture(Some("4"), "30.25");
    let trait_step = trait_request();
    let requests = vec![
        advancement(trait_step),
        power(power_request(
            ANIMALISM_FERAL_WHISPERS_ID,
            ClanRelation::InClan,
        )),
    ];
    let quote = mixed_quote(&lib, &id, &requests);
    let saved = lib.commit_mixed_advancement(&quote, ST).unwrap();
    let purchase = lib.purchase_receipts(&id, ST).unwrap().remove(0);
    assert!(purchase.batch.is_none());

    let cancellation = grapevine_workbench::cancellations::CancellationRequest {
        id: Uuid::new_v4().to_string(),
        day: "2026-01-04".into(),
        actor: "PRIVATE_CANCEL_ACTOR".into(),
        reason: "PRIVATE_CANCEL_REASON".into(),
        evidence: "PRIVATE_CANCEL_EVIDENCE".into(),
    };
    let cancellation_quote = lib
        .quote_cancellation(
            &purchase.quote.request.id,
            &cancellation,
            lib.revision().unwrap(),
            ST,
        )
        .unwrap();
    let cancellation_receipt = lib.commit_cancellation(&cancellation_quote, ST).unwrap();
    assert_eq!(
        lib.purchase_receipts(&id, ST).unwrap()[0].cancellation,
        Some(cancellation_receipt)
    );
    assert_eq!(
        lib.xp_view(&id, ST).unwrap().unspent.as_deref(),
        Some("29.25")
    );
    assert_no_advancement_audit_drift(&lib, &id);
    assert_eq!(lib.mixed_advancement_history(&id, ST).unwrap(), vec![saved]);
}

#[test]
fn trait_correction_audit_privacy_and_backup_restore_survive() {
    let (directory, mut lib, id) = fixture(Some("4"), "30.25");
    let trait_step = trait_request();
    let trait_id = trait_step.id.clone();
    let requests = vec![
        advancement(trait_step),
        power(power_request(
            ANIMALISM_FERAL_WHISPERS_ID,
            ClanRelation::InClan,
        )),
    ];
    let quote = mixed_quote(&lib, &id, &requests);
    let saved = lib.commit_mixed_advancement(&quote, ST).unwrap();
    let purchase = lib.purchase_receipts(&id, ST).unwrap().remove(0);
    assert!(purchase.batch.is_none());
    assert!(purchase.cancellation.is_none());

    let correction = trait_corrections::Request {
        id: Uuid::new_v4().to_string(),
        advancement_id: trait_id.clone(),
        day: "2026-01-05".into(),
        actor: "PRIVATE_CORRECTION_ACTOR".into(),
        reason: "Synthetic clerical correction".into(),
        evidence: "PRIVATE_CORRECTION_EVIDENCE".into(),
    };
    let correction_quote = lib
        .quote_trait_correction(&id, &correction, lib.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(correction_quote.refund, "1");
    lib.commit_trait_correction(&correction_quote, ST).unwrap();

    let revision_before_retry = lib.revision().unwrap();
    assert_eq!(lib.commit_mixed_advancement(&quote, ST).unwrap(), saved);
    assert_eq!(lib.revision().unwrap(), revision_before_retry);
    assert_eq!(
        lib.xp_view(&id, ST).unwrap().unspent.as_deref(),
        Some("27.25")
    );
    assert_no_advancement_audit_drift(&lib, &id);
    assert_eq!(
        lib.record(&id, ST).unwrap().fields["AbilityList"]["Traits"][0]["Value"],
        "4"
    );

    let player = lib.record(&id, Role::Player).unwrap();
    assert!(!player.fields.contains_key(mixed::FIELD));
    assert!(!player.fields.contains_key("AbilityList"));
    let player_json = serde_json::to_string(&player).unwrap();
    for secret in [
        "PRIVATE_POWER_ACTOR",
        "PRIVATE_POWER_REASON",
        "PRIVATE_TRAIT_ACTOR",
        "PRIVATE_TRAIT_REASON",
        "PRIVATE_ABILITY_NOTE",
        "PRIVATE_CORRECTION_EVIDENCE",
    ] {
        assert!(!player_json.contains(secret), "leaked {secret}");
    }
    assert!(lib.mixed_advancement_history(&id, Role::Player).is_err());
    assert!(lib.purchase_receipts(&id, Role::Player).is_err());
    assert!(
        !lib.record(&id, ST)
            .unwrap()
            .fields
            .contains_key(mixed::FIELD)
    );
    let printed = lib
        .print_document(
            &id,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap();
    let printed_json = serde_json::to_string(&printed).unwrap();
    assert!(!printed_json.contains("PRIVATE_POWER_ACTOR"));
    assert!(!printed_json.contains("PRIVATE_CORRECTION_EVIDENCE"));

    let backup = directory.path().join("mixed-backup.gvbackup");
    lib.backup(&backup, ST).unwrap();
    lib.post_xp(&id, &award(), lib.revision().unwrap(), ST)
        .unwrap();
    lib.restore(&backup, lib.revision().unwrap(), ST).unwrap();
    let history = lib.mixed_advancement_history(&id, ST).unwrap();
    assert_eq!(history, vec![saved]);
    let restored_purchase = lib.purchase_receipts(&id, ST).unwrap().remove(0);
    assert!(restored_purchase.batch.is_none());
    assert!(restored_purchase.cancellation.is_none());
    assert_eq!(
        lib.trait_advancement_context(&id, ST)
            .unwrap()
            .corrected_ids,
        vec![trait_id]
    );
    assert_eq!(
        lib.xp_view(&id, ST).unwrap().unspent.as_deref(),
        Some("27.25")
    );
    assert_no_advancement_audit_drift(&lib, &id);
}
