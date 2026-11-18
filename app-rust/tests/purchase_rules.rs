use grapevine_workbench::{model::*, pricing::*, purchase_preview::*, purchases::*, xp::*};
use serde_json::json;
fn record() -> Record {
    Record {
        id: "synthetic-character".into(),
        kind: "Vampire".into(),
        name: "Synthetic".into(),
        player_id: None,
        legacy_id: None,
        public_fields: Default::default(),
        private_fields: Default::default(),
        original: json!(null),
        source_sha256: "synthetic".into(),
        st_markers: None,
        build: None,
        xp_ledger: Some(Ledger {
            baseline: Baseline {
                day: "2026-01-01".into(),
                earned: "10".into(),
                unspent: "10".into(),
                actor: "Synthetic ST".into(),
                reason: "Test opening".into(),
                evidence: "Synthetic opening".into(),
            },
            entries: vec![],
        }),
    }
}
fn request() -> PurchaseRequest {
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
        reason: "Synthetic test".into(),
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
#[test]
fn dated_quote_replay_rejects_debit_before_funding_even_when_current_balance_is_enough() {
    let mut r = record();
    let ledger = r.xp_ledger.as_mut().unwrap();
    ledger.baseline.earned = "2".into();
    ledger.baseline.unspent = "2".into();
    ledger.entries.push(Entry {
        command: XpCommand {
            id: "late-award".into(),
            kind: XpKind::Award,
            amount: "8".into(),
            day: "2026-01-03".into(),
            actor: "Synthetic ST".into(),
            reason: "Test".into(),
            evidence: "Synthetic".into(),
            reversal_of: None,
            cost: None,
        },
        recorded_unix_seconds: 1,
        origin_library_id: "synthetic-library".into(),
    });
    assert_eq!(replay(ledger).unwrap().1, 10 * SCALE);
    let before = serde_json::to_string(&r).unwrap();
    assert!(quote("synthetic-library", 2, &r, &request()).is_err());
    assert_eq!(before, serde_json::to_string(&r).unwrap());
}
#[test]
fn grant_preserves_private_trait_notes_and_receipt_detects_later_trait_or_spend_changes() {
    let mut r = record();
    let mut list = grapevine_workbench::sheet::empty_list("DisciplineList");
    list["Traits"] =
        json!([{"Name":"Old power","Value":"basic","Note":"[ST]PRIVATE_EVIDENCE[/ST]"}]);
    r.private_fields
        .insert("DisciplineList".into(), list.clone());
    r.public_fields.insert(
        "DisciplineList".into(),
        grapevine_workbench::importer::redact(&list),
    );
    let q = quote("synthetic-library", 2, &r, &request()).unwrap();
    let updated = apply(&r, &q, 1).unwrap();
    let receipt = receipt(&q, 3);
    assert!(
        updated.private_fields["DisciplineList"]
            .to_string()
            .contains("PRIVATE_EVIDENCE")
    );
    assert!(
        !updated.public_fields["DisciplineList"]
            .to_string()
            .contains("PRIVATE_EVIDENCE")
    );
    assert!(
        !updated.public_fields["DisciplineList"]
            .to_string()
            .contains("Synthetic approval")
    );
    assert!(findings(&updated, &receipt).is_empty());
    let mut drift = updated.clone();
    drift.private_fields.get_mut("DisciplineList").unwrap()["Traits"][1]["Value"] =
        json!("advanced");
    assert!(
        findings(&drift, &receipt)
            .iter()
            .any(|f| f.code == "PURCHASE_TRAIT_DRIFT")
    );
    let mut missing = updated;
    missing.xp_ledger.as_mut().unwrap().entries.clear();
    assert!(
        findings(&missing, &receipt)
            .iter()
            .any(|f| f.code == "PURCHASE_SPEND_MISMATCH")
    );
}
#[test]
fn incomplete_local_declarations_and_nonstandard_existing_lists_are_not_silently_accepted() {
    let mut req = request();
    req.teacher_rule = None;
    assert_eq!(
        quote("synthetic-library", 2, &record(), &req)
            .unwrap_err()
            .code,
        "PURCHASE_RULES_UNRESOLVED"
    );
    req = request();
    req.policy_reference.clear();
    assert_eq!(
        quote("synthetic-library", 2, &record(), &req)
            .unwrap_err()
            .code,
        "PURCHASE_EVIDENCE_REQUIRED"
    );
    let mut r = record();
    r.private_fields
        .insert("DisciplineList".into(), json!("unmapped legacy data"));
    assert_eq!(
        quote("synthetic-library", 2, &r, &request())
            .unwrap_err()
            .code,
        "PURCHASE_UNSUPPORTED_SHEET"
    );
}
