use grapevine_workbench::{
    model::{BuildKind, Role},
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::{ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID},
    purchase_reviews::{self, FIELD},
    purchases::{PurchaseRequest, TeacherRule},
    sheet,
    store::Library,
    xp::{Baseline, XpCommand, XpKind},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

fn baseline() -> Baseline {
    Baseline {
        day: "2026-01-01".into(),
        earned: "10.25".into(),
        unspent: "10.25".into(),
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
        reason: "Synthetic purchase review test".into(),
        policy_version: "synthetic-policy-v1".into(),
        policy_reference: "Synthetic local policy reference".into(),
        affiliation_evidence: "Synthetic affiliation evidence".into(),
        approval_reference: String::new(),
        eligibility_evidence: "Synthetic eligibility evidence".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: String::new(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    }
}

fn fixture() -> (TempDir, Library, String) {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("reviews.gvlib");
    let mut library = Library::create(&path, "Synthetic purchase review tests").unwrap();
    let id = library
        .create_character("Vampire", "Synthetic Vampire", 0, Role::Storyteller)
        .unwrap();
    library
        .initialize_xp(&id, &baseline(), 1, Role::Storyteller)
        .unwrap();
    (directory, library, id)
}

fn save_review(library: &mut Library, id: &str) -> purchase_reviews::ReviewBinding {
    let input = request(ANIMALISM_FERAL_WHISPERS_ID);
    library
        .save_purchase_review(
            id,
            &input,
            "Synthetic review",
            library.revision().unwrap(),
            Role::Storyteller,
        )
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

fn commit_review_linked_purchase() -> (TempDir, Library, String, purchase_reviews::ReviewBinding) {
    let (directory, mut library, id) = fixture();
    let binding = save_review(&mut library, &id);
    let mut input = library
        .reuse_purchase_review(
            &id,
            &binding.id,
            &request(ANIMALISM_FERAL_WHISPERS_ID),
            library.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    input.approval_reference = "Fresh synthetic approval".into();
    input.teacher_evidence = "Fresh synthetic teacher evidence".into();
    let quote = library
        .quote_purchase(&id, &input, library.revision().unwrap(), Role::Storyteller)
        .unwrap();
    library.commit_purchase(&quote, Role::Storyteller).unwrap();
    (directory, library, id, binding)
}

#[test]
fn private_binding_survives_backup_reopen_and_can_be_reused() {
    let (directory, mut library, id) = fixture();
    let binding = save_review(&mut library, &id);
    let stored: Value = serde_json::from_str(&raw(&library.path, &id)).unwrap();
    assert!(stored["public_fields"].get(FIELD).is_none());
    assert_eq!(stored["private_fields"][FIELD][0]["id"], binding.id);
    assert!(
        !library
            .record(&id, Role::Storyteller)
            .unwrap()
            .fields
            .contains_key(FIELD)
    );
    assert!(
        !library
            .record(&id, Role::Player)
            .unwrap()
            .fields
            .contains_key(FIELD)
    );

    let backup_path = directory.path().join("reviews-backup.gvlib");
    library.backup(&backup_path, Role::Storyteller).unwrap();
    drop(library);

    let reopened = Library::open(&backup_path).unwrap();
    assert_eq!(
        reopened.purchase_reviews(&id, Role::Storyteller).unwrap(),
        vec![(binding.clone(), true)]
    );
    let reused = reopened
        .reuse_purchase_review(
            &id,
            &binding.id,
            &request(ANIMALISM_FERAL_WHISPERS_ID),
            reopened.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(reused.review_id.as_deref(), Some(binding.id.as_str()));
    assert_eq!(reused.policy_reference, binding.policy.policy_reference);
    assert_eq!(reused.affiliation_evidence, binding.affiliation_evidence);
}

#[test]
fn exact_state_hash_goes_stale_after_xp_or_sheet_drift() {
    let (_directory, mut xp_library, xp_id) = fixture();
    let xp_binding = save_review(&mut xp_library, &xp_id);
    let xp_command = XpCommand {
        id: Uuid::new_v4().to_string(),
        kind: XpKind::Award,
        amount: "1".into(),
        day: "2026-01-03".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic later award".into(),
        evidence: "Synthetic award evidence".into(),
        reversal_of: None,
        cost: None,
    };
    xp_library
        .post_xp(
            &xp_id,
            &xp_command,
            xp_library.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert!(
        !xp_library
            .purchase_reviews(&xp_id, Role::Storyteller)
            .unwrap()[0]
            .1
    );
    let xp_record: grapevine_workbench::model::Record =
        serde_json::from_str(&raw(&xp_library.path, &xp_id)).unwrap();
    assert_ne!(
        xp_binding.record_hash,
        purchase_reviews::state_hash(&xp_record).unwrap()
    );
    assert_eq!(
        xp_library
            .reuse_purchase_review(
                &xp_id,
                &xp_binding.id,
                &request(ANIMALISM_FERAL_WHISPERS_ID),
                xp_library.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap_err()
            .code,
        "REVIEW_STALE"
    );

    let (_directory, mut sheet_library, sheet_id) = fixture();
    let sheet_binding = save_review(&mut sheet_library, &sheet_id);
    let mut sheet_edit = sheet_library
        .sheet_edit(&sheet_id, Role::Storyteller)
        .unwrap();
    let list = sheet_edit
        .lists
        .entry("DisciplineList".into())
        .or_insert_with(|| sheet::empty_list("DisciplineList"));
    list["Traits"].as_array_mut().unwrap().push(json!({
        "Name": "Synthetic discipline",
        "Value": "basic",
        "Note": "Synthetic sheet drift"
    }));
    sheet_library
        .save_sheet(
            &sheet_id,
            &sheet_edit,
            sheet_library.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert!(
        !sheet_library
            .purchase_reviews(&sheet_id, Role::Storyteller)
            .unwrap()[0]
            .1
    );
    let sheet_record: grapevine_workbench::model::Record =
        serde_json::from_str(&raw(&sheet_library.path, &sheet_id)).unwrap();
    assert_ne!(
        sheet_binding.record_hash,
        purchase_reviews::state_hash(&sheet_record).unwrap()
    );
    assert_eq!(
        sheet_library
            .reuse_purchase_review(
                &sheet_id,
                &sheet_binding.id,
                &request(ANIMALISM_FERAL_WHISPERS_ID),
                sheet_library.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap_err()
            .code,
        "REVIEW_STALE"
    );
}

#[test]
fn reuse_rejects_other_character_policy_or_power_and_player_scope() {
    let (_directory, mut library, id) = fixture();
    let binding = save_review(&mut library, &id);
    let revision = library.revision().unwrap();

    assert_eq!(
        library
            .save_purchase_review(
                &id,
                &request(ANIMALISM_FERAL_WHISPERS_ID),
                "Player attempt",
                revision,
                Role::Player,
            )
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert_eq!(
        library
            .purchase_reviews(&id, Role::Player)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert_eq!(
        library
            .reuse_purchase_review(
                &id,
                &binding.id,
                &request(ANIMALISM_FERAL_WHISPERS_ID),
                revision,
                Role::Player,
            )
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );

    let other_id = library
        .create_character(
            "Vampire",
            "Other synthetic Vampire",
            revision,
            Role::Storyteller,
        )
        .unwrap();
    let revision = library.revision().unwrap();
    assert_eq!(
        library
            .reuse_purchase_review(
                &other_id,
                &binding.id,
                &request(ANIMALISM_FERAL_WHISPERS_ID),
                revision,
                Role::Storyteller,
            )
            .unwrap_err()
            .code,
        "REVIEW_NOT_FOUND"
    );

    let mut changed_policy = library
        .reuse_purchase_review(
            &id,
            &binding.id,
            &request(ANIMALISM_FERAL_WHISPERS_ID),
            revision,
            Role::Storyteller,
        )
        .unwrap();
    changed_policy.policy_reference.push_str(" changed");
    assert_eq!(
        library
            .quote_purchase(&id, &changed_policy, revision, Role::Storyteller)
            .unwrap_err()
            .code,
        "REVIEW_MISMATCH"
    );

    let mut changed_power = library
        .reuse_purchase_review(
            &id,
            &binding.id,
            &request(ANIMALISM_FERAL_WHISPERS_ID),
            revision,
            Role::Storyteller,
        )
        .unwrap();
    changed_power.power_id = ANIMALISM_BECKONING_ID.into();
    assert_eq!(
        library
            .quote_purchase(&id, &changed_power, revision, Role::Storyteller)
            .unwrap_err()
            .code,
        "REVIEW_MISMATCH"
    );

    let build_id = library
        .create_build(&id, BuildKind::Draft, revision, Role::Storyteller)
        .unwrap();
    let revision = library.revision().unwrap();
    assert_eq!(
        library
            .save_purchase_review(
                &build_id,
                &request(ANIMALISM_FERAL_WHISPERS_ID),
                "Build review attempt",
                revision,
                Role::Storyteller,
            )
            .unwrap_err()
            .code,
        "REVIEW_SCOPE"
    );
    let mortal_id = library
        .create_character("Mortal", "Synthetic mortal", revision, Role::Storyteller)
        .unwrap();
    assert_eq!(
        library
            .save_purchase_review(
                &mortal_id,
                &request(ANIMALISM_FERAL_WHISPERS_ID),
                "Mortal review attempt",
                library.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap_err()
            .code,
        "REVIEW_SCOPE"
    );
}

#[test]
fn purchase_requires_fresh_approval_and_teacher_evidence_and_keeps_review_link() {
    let (_directory, mut library, id) = fixture();
    let binding = save_review(&mut library, &id);
    let mut reused = library
        .reuse_purchase_review(
            &id,
            &binding.id,
            &request(ANIMALISM_FERAL_WHISPERS_ID),
            library.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(reused.approval_reference, "");
    assert_eq!(reused.teacher_evidence, "");
    assert_eq!(
        library
            .quote_purchase(&id, &reused, library.revision().unwrap(), Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_EVIDENCE_REQUIRED"
    );

    reused.approval_reference = "Fresh synthetic approval".into();
    assert_eq!(
        library
            .quote_purchase(&id, &reused, library.revision().unwrap(), Role::Storyteller)
            .unwrap_err()
            .code,
        "PURCHASE_EVIDENCE_REQUIRED"
    );

    reused.teacher_evidence = "Fresh synthetic qualified-teacher evidence".into();
    let quote = library
        .quote_purchase(&id, &reused, library.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(
        quote.request.review_id.as_deref(),
        Some(binding.id.as_str())
    );
    let receipt = library.commit_purchase(&quote, Role::Storyteller).unwrap();
    assert_eq!(
        receipt.quote.request.review_id.as_deref(),
        Some(binding.id.as_str())
    );
}

#[test]
fn failed_review_record_or_audit_write_is_atomic() {
    let (_directory, mut library, id) = fixture();
    let request = request(ANIMALISM_FERAL_WHISPERS_ID);
    let before_record = raw(&library.path, &id);
    let before_revision = library.revision().unwrap();
    let before_audit = audit_count(&library.path);

    for (trigger, sql) in [
        (
            "fail_review_record",
            "CREATE TRIGGER fail_review_record BEFORE UPDATE ON records
             BEGIN SELECT RAISE(ABORT,'injected record failure'); END;",
        ),
        (
            "fail_review_audit",
            "CREATE TRIGGER fail_review_audit
             BEFORE INSERT ON audit WHEN NEW.operation='purchase_review'
             BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;",
        ),
    ] {
        Connection::open(&library.path)
            .unwrap()
            .execute_batch(sql)
            .unwrap();
        assert!(
            library
                .save_purchase_review(
                    &id,
                    &request,
                    "Synthetic review",
                    before_revision,
                    Role::Storyteller,
                )
                .is_err()
        );
        assert_eq!(library.revision().unwrap(), before_revision, "{trigger}");
        assert_eq!(raw(&library.path, &id), before_record, "{trigger}");
        assert_eq!(audit_count(&library.path), before_audit, "{trigger}");
        assert!(
            library
                .purchase_reviews(&id, Role::Storyteller)
                .unwrap()
                .is_empty()
        );
        Connection::open(&library.path)
            .unwrap()
            .execute_batch(&format!("DROP TRIGGER {trigger};"))
            .unwrap();
    }
}

#[test]
fn receipt_audit_flags_missing_or_changed_linked_review_metadata() {
    for remove_binding in [true, false] {
        let (_directory, library, id, _binding) = commit_review_linked_purchase();
        let mut record: grapevine_workbench::model::Record =
            serde_json::from_str(&raw(&library.path, &id)).unwrap();
        if remove_binding {
            record.private_fields.remove(FIELD);
        } else {
            record.private_fields.get_mut(FIELD).unwrap()[0]["policy"]["policy_reference"] =
                json!("Altered review declaration");
        }
        Connection::open(&library.path)
            .unwrap()
            .execute(
                "UPDATE records SET data=?1 WHERE id=?2",
                rusqlite::params![serde_json::to_string(&record).unwrap(), id],
            )
            .unwrap();

        let view = library.xp_view(&id, Role::Storyteller).unwrap();
        assert!(view.findings.iter().any(|finding| {
            finding.code == "PURCHASE_REVIEW_DRIFT" && finding.severity == "review"
        }));
    }
}
