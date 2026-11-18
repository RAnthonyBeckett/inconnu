use grapevine_workbench::{
    importer,
    model::Record,
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{self, PurchaseReceipt, PurchaseRequest, TeacherRule},
    sheet,
    xp::{self, Baseline, Entry, Ledger, XpCommand, XpKind},
};
use serde_json::{Value, json};

fn record() -> Record {
    Record {
        id: "synthetic-character".into(),
        kind: "Vampire".into(),
        name: "Synthetic".into(),
        player_id: None,
        legacy_id: None,
        public_fields: Default::default(),
        private_fields: Default::default(),
        original: Value::Null,
        source_sha256: "synthetic".into(),
        st_markers: None,
        build: None,
        xp_ledger: Some(Ledger {
            baseline: Baseline {
                day: "2026-01-01".into(),
                earned: "100".into(),
                unspent: "100".into(),
                actor: "Synthetic ST".into(),
                reason: "Test opening".into(),
                evidence: "Synthetic opening".into(),
            },
            entries: vec![],
        }),
    }
}

fn purchase_request() -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: uuid::Uuid::new_v4().to_string(),
        power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(ClanRelation::InClan),
        affiliation_name: "Synthetic clan".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic next session".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic test purchase".into(),
        policy_version: "Synthetic1".into(),
        policy_reference: "Synthetic rules".into(),
        affiliation_evidence: "Synthetic affiliation".into(),
        approval_reference: "Synthetic approval".into(),
        eligibility_evidence: "Synthetic eligibility".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Synthetic teacher qualification".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    }
}

fn purchased(mut source: Record) -> (Record, PurchaseReceipt) {
    let quote = purchases::quote("synthetic-library", 2, &source, &purchase_request()).unwrap();
    source = purchases::apply(&source, &quote, 10).unwrap();
    (source, purchases::receipt(&quote, 3))
}

fn cancellation_request() -> grapevine_workbench::cancellations::CancellationRequest {
    grapevine_workbench::cancellations::CancellationRequest {
        id: uuid::Uuid::new_v4().to_string(),
        day: "2026-01-04".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic test cancellation".into(),
        evidence: "Synthetic cancellation evidence".into(),
    }
}

fn cancelled(
    source: &Record,
    purchase: &PurchaseReceipt,
) -> (
    grapevine_workbench::cancellations::CancellationQuote,
    Record,
    grapevine_workbench::cancellations::CancellationReceipt,
    PurchaseReceipt,
) {
    let quote = grapevine_workbench::cancellations::quote(
        "synthetic-library",
        3,
        source,
        purchase,
        &cancellation_request(),
    )
    .unwrap();
    let updated = grapevine_workbench::cancellations::apply(source, &quote, purchase, 20).unwrap();
    let receipt = grapevine_workbench::cancellations::CancellationReceipt {
        quote: quote.clone(),
        committed_revision: 4,
    };
    let mut stored_purchase = purchase.clone();
    stored_purchase.cancellation = Some(receipt.clone());
    (quote, updated, receipt, stored_purchase)
}

fn has_code(findings: &[xp::Finding], code: &str) -> bool {
    findings.iter().any(|item| item.code == code)
}

#[test]
fn cancellation_restores_an_originally_missing_discipline_list() {
    let (bought, purchase) = purchased(record());
    let before_hash = purchases::record_hash(&record()).unwrap();
    let (quote, updated, _, _) = cancelled(&bought, &purchase);

    assert_eq!(quote.restored_public_list, None);
    assert_eq!(quote.restored_private_list, None);
    assert!(!updated.public_fields.contains_key("DisciplineList"));
    assert!(!updated.private_fields.contains_key("DisciplineList"));
    let prior = updated.xp_ledger.as_ref().unwrap();
    assert_eq!(prior.entries.len(), 2);
    assert_eq!(prior.entries[0].command.kind, XpKind::Spend);
    assert_eq!(prior.entries[1].command.kind, XpKind::Reversal);
    assert_eq!(
        prior.entries[1].command.reversal_of.as_deref(),
        Some(purchase.quote.request.id.as_str())
    );
    assert_eq!(purchases::record_hash(&record()).unwrap(), before_hash);
}

#[test]
fn cancellation_restores_private_secrets_and_the_public_projection_exactly() {
    let mut source = record();
    let mut list = sheet::empty_list("DisciplineList");
    list["Traits"] = json!([{
        "Name": "Old power",
        "Value": "basic",
        "Note": "[ST]PRIVATE_EVIDENCE[/ST]"
    }]);
    let public = importer::redact(&list);
    source
        .private_fields
        .insert("DisciplineList".into(), list.clone());
    source
        .public_fields
        .insert("DisciplineList".into(), public.clone());

    let (bought, purchase) = purchased(source);
    let (quote, restored, _, _) = cancelled(&bought, &purchase);

    assert_eq!(quote.restored_public_list, Some(public.clone()));
    assert_eq!(quote.restored_private_list, Some(list.clone()));
    assert_eq!(restored.public_fields["DisciplineList"], public);
    assert_eq!(restored.private_fields["DisciplineList"], list);
    assert!(
        restored.private_fields["DisciplineList"]
            .to_string()
            .contains("PRIVATE_EVIDENCE")
    );
    assert!(
        !restored.public_fields["DisciplineList"]
            .to_string()
            .contains("PRIVATE_EVIDENCE")
    );
}

#[test]
fn cancellation_restores_a_private_only_list() {
    let mut source = record();
    let mut list = sheet::empty_list("DisciplineList");
    list["Traits"] = json!([{
        "Name": "Private power",
        "Value": "basic",
        "Note": "Private-only note"
    }]);
    source
        .private_fields
        .insert("DisciplineList".into(), list.clone());

    let (bought, purchase) = purchased(source);
    let (quote, restored, _, _) = cancelled(&bought, &purchase);

    assert_eq!(quote.restored_public_list, None);
    assert_eq!(quote.restored_private_list, Some(list.clone()));
    assert!(!restored.public_fields.contains_key("DisciplineList"));
    assert_eq!(restored.private_fields["DisciplineList"], list);
}

#[test]
fn later_award_spend_or_sheet_change_blocks_cancellation() {
    let (bought, purchase) = purchased(record());
    let make_later = |id: &str, kind: XpKind| Entry {
        command: XpCommand {
            id: id.into(),
            kind,
            amount: "1".into(),
            day: "2026-01-04".into(),
            actor: "Synthetic ST".into(),
            reason: "Later ledger event".into(),
            evidence: "Later event evidence".into(),
            reversal_of: None,
            cost: None,
        },
        recorded_unix_seconds: 11,
        origin_library_id: "synthetic-library".into(),
    };

    for entry in [
        make_later("later-award", XpKind::Award),
        make_later("later-spend", XpKind::Spend),
    ] {
        let mut changed = bought.clone();
        changed.xp_ledger.as_mut().unwrap().entries.push(entry);
        let result = grapevine_workbench::cancellations::quote(
            "synthetic-library",
            4,
            &changed,
            &purchase,
            &cancellation_request(),
        );
        assert_eq!(result.unwrap_err().code, "CANCEL_PURCHASE_ORDER");
    }

    let mut changed = bought;
    changed
        .public_fields
        .insert("LaterField".into(), json!("changed"));
    let result = grapevine_workbench::cancellations::quote(
        "synthetic-library",
        4,
        &changed,
        &purchase,
        &cancellation_request(),
    );
    assert_eq!(result.unwrap_err().code, "CANCEL_SNAPSHOT_UNKNOWN");
}

#[test]
fn stale_double_and_incomplete_cancellation_requests_are_rejected() {
    let (bought, purchase) = purchased(record());
    let quote = grapevine_workbench::cancellations::quote(
        "synthetic-library",
        3,
        &bought,
        &purchase,
        &cancellation_request(),
    )
    .unwrap();
    let mut changed = bought.clone();
    changed
        .public_fields
        .insert("LaterField".into(), json!("changed"));
    assert_eq!(
        grapevine_workbench::cancellations::apply(&changed, &quote, &purchase, 20)
            .unwrap_err()
            .code,
        "CANCEL_STALE"
    );

    let mut stored_purchase = purchase.clone();
    stored_purchase.cancellation = Some(grapevine_workbench::cancellations::CancellationReceipt {
        quote: quote.clone(),
        committed_revision: 4,
    });
    assert_eq!(
        grapevine_workbench::cancellations::quote(
            "synthetic-library",
            3,
            &bought,
            &stored_purchase,
            &quote.request,
        )
        .unwrap_err()
        .code,
        "CANCEL_ALREADY_RECORDED"
    );

    let mut incomplete = cancellation_request();
    incomplete.actor.clear();
    assert_eq!(
        grapevine_workbench::cancellations::quote(
            "synthetic-library",
            3,
            &bought,
            &purchase,
            &incomplete,
        )
        .unwrap_err()
        .code,
        "CANCEL_EVIDENCE_REQUIRED"
    );
}

#[test]
fn refund_is_exact_and_the_original_spend_remains_in_the_ledger() {
    let (bought, purchase) = purchased(record());
    let (quote, updated, _, _) = cancelled(&bought, &purchase);
    let entries = &updated.xp_ledger.as_ref().unwrap().entries;
    let original = &entries[0].command;
    let refund = &entries[1].command;

    assert_eq!(entries.len(), 2);
    assert_eq!(original.id, purchase.quote.request.id);
    assert_eq!(original.kind, XpKind::Spend);
    assert_eq!(refund.kind, XpKind::Reversal);
    assert_eq!(refund.amount, original.amount);
    assert_eq!(refund.amount, quote.refund);
    assert_eq!(refund.reversal_of.as_deref(), Some(original.id.as_str()));
    let before = xp::amount(&quote.before_unspent).unwrap();
    let after = xp::amount(&quote.after_unspent).unwrap();
    assert_eq!(after - before, xp::amount(&original.amount).unwrap());
    xp::replay(updated.xp_ledger.as_ref().unwrap()).unwrap();
}

#[test]
fn cancelled_purchase_audit_checks_refund_and_trait_reappearance() {
    let mut source = record();
    let mut list = sheet::empty_list("DisciplineList");
    list["Traits"] = json!([{"Name":"Old power","Value":"basic","Note":"Public"}]);
    source.public_fields.insert("DisciplineList".into(), list);
    let (bought, purchase) = purchased(source);
    let (_, updated, cancel, stored_purchase) = cancelled(&bought, &purchase);

    assert!(purchases::findings(&updated, &stored_purchase).is_empty());

    let mut later_award = updated.clone();
    later_award.xp_ledger.as_mut().unwrap().entries.push(Entry {
        command: XpCommand {
            id: uuid::Uuid::new_v4().to_string(),
            kind: XpKind::Award,
            amount: "1".into(),
            day: "2026-01-05".into(),
            actor: "Synthetic ST".into(),
            reason: "Later award".into(),
            evidence: "Later award evidence".into(),
            reversal_of: None,
            cost: None,
        },
        recorded_unix_seconds: 30,
        origin_library_id: "synthetic-library".into(),
    });
    assert!(purchases::findings(&later_award, &stored_purchase).is_empty());

    let mut missing_refund = updated.clone();
    missing_refund.xp_ledger.as_mut().unwrap().entries.pop();
    let findings = purchases::findings(&missing_refund, &stored_purchase);
    assert!(has_code(&findings, "CANCEL_REFUND_MISMATCH"));
    assert!(has_code(&findings, "PURCHASE_TRAIT_DRIFT"));

    let mut changed_refund = updated.clone();
    changed_refund
        .xp_ledger
        .as_mut()
        .unwrap()
        .entries
        .last_mut()
        .unwrap()
        .command
        .reason = "Different recorded reason".into();
    assert!(has_code(
        &purchases::findings(&changed_refund, &stored_purchase),
        "CANCEL_REFUND_MISMATCH"
    ));

    let mut trait_reappeared = updated;
    trait_reappeared
        .public_fields
        .get_mut("DisciplineList")
        .unwrap()["Traits"]
        .as_array_mut()
        .unwrap()
        .push(purchase.quote.trait_added.clone());
    let findings =
        grapevine_workbench::cancellations::findings(&trait_reappeared, &stored_purchase, &cancel);
    assert!(has_code(&findings, "CANCEL_TRAIT_REAPPEARED"));
    assert!(has_code(&findings, "PURCHASE_SPEND_REVERSED"));
}
