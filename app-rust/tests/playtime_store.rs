use grapevine_workbench::{
    model::{BuildKind, Edit, Role},
    playtime::{CapClass, Request},
    store::Library,
    xp::{Baseline, XpCommand, XpKind},
};
use tempfile::TempDir;

fn fixture() -> (TempDir, Library, String) {
    let d = TempDir::new().unwrap();
    let mut library = Library::create(&d.path().join("playtime.gvlib"), "Synthetic").unwrap();
    let id = library
        .create_character("Vampire", "Synthetic character", 0, Role::Storyteller)
        .unwrap();
    library
        .initialize_xp(
            &id,
            &Baseline {
                day: "2026-01-01".into(),
                earned: "200".into(),
                unspent: "200".into(),
                actor: "PRIVATE_ACTOR".into(),
                reason: "PRIVATE_REASON".into(),
                evidence: "PRIVATE_BASELINE".into(),
            },
            1,
            Role::Storyteller,
        )
        .unwrap();
    (d, library, id)
}
fn request() -> Request {
    Request {
        start_month: "2026-01".into(),
        end_month: "2026-01".into(),
        class: CapClass::Standard,
        policy_scope_confirmed: true,
        complete_regular_history_confirmed: true,
        full_month_eligible_confirmed: true,
        home_chronicle: "PRIVATE_CHRONICLE".into(),
        review_evidence: "PRIVATE_REVIEW".into(),
        ..Default::default()
    }
}
fn award(library: &mut Library, id: &str, day: &str, amount: &str) {
    let revision = library.revision().unwrap();
    library
        .post_xp(
            id,
            &XpCommand {
                id: uuid::Uuid::new_v4().to_string(),
                kind: XpKind::Award,
                day: day.into(),
                amount: amount.into(),
                actor: "PRIVATE_ACTOR".into(),
                reason: "PRIVATE_REASON".into(),
                evidence: "PRIVATE_AWARD".into(),
                reversal_of: None,
                cost: None,
            },
            revision,
            Role::Storyteller,
        )
        .unwrap();
}

#[test]
fn monthly_review_uses_earned_opening_is_read_only_and_exports_no_private_fields() {
    let (_d, mut library, id) = fixture();
    library
        .save_edit(
            &id,
            &Edit {
                name: "Synthetic character".into(),
                status: "Active".into(),
                notes: "PRIVATE_ST_NOTE".into(),
            },
            2,
            Role::Storyteller,
        )
        .unwrap();
    award(&mut library, &id, "2026-01-01", "12");
    let revision = library.revision().unwrap();
    let explicit_review = reviewed_request(&library, &id, request());
    let report = library
        .playtime_report(&id, &explicit_review, revision, Role::Storyteller)
        .unwrap();
    assert_eq!(report["report"]["status"], "WithinDeclaredCeiling");
    assert_eq!(report["report"]["months"][0]["opening_earned"], "200");
    assert_eq!(report["report"]["months"][0]["regular_awarded"], "12");
    assert_eq!(report["report"]["months"][0]["ceiling"], "12");
    assert!(!serde_json::to_string(&report).unwrap().contains("PRIVATE_"));
    assert_eq!(library.revision().unwrap(), revision);
    assert!(
        report["declared_assumptions"]["review_evidence_supplied"]
            .as_bool()
            .unwrap()
    );
    assert!(
        library
            .playtime_report(&id, &request(), revision, Role::Player)
            .is_err()
    );
    assert!(
        library
            .playtime_report(&id, &request(), revision - 1, Role::Storyteller)
            .is_err()
    );
    let build = library
        .create_build(&id, BuildKind::Test, revision, Role::Storyteller)
        .unwrap();
    assert!(
        library
            .playtime_report(&build, &request(), revision + 1, Role::Storyteller)
            .is_err()
    );
}

#[test]
fn monthly_review_reports_over_cap_and_backup_restore_keeps_same_calculation() {
    let (d, mut library, id) = fixture();
    award(&mut library, &id, "2026-01-02", "12");
    award(&mut library, &id, "2026-02-01", "11");
    let mut input = request();
    input.end_month = "2026-02".into();
    let input = reviewed_request(&library, &id, input);
    let revision = library.revision().unwrap();
    let report = library
        .playtime_report(&id, &input, revision, Role::Storyteller)
        .unwrap();
    assert_eq!(report["report"]["status"], "ExceedsDeclaredCeiling");
    assert_eq!(report["report"]["months"][1]["opening_earned"], "212");
    assert_eq!(report["report"]["months"][1]["ceiling"], "10");
    let backup = d.path().join("review.gvbackup");
    library.backup(&backup, Role::Storyteller).unwrap();
    award(&mut library, &id, "2026-02-02", "1");
    library
        .restore(&backup, library.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let restored = library
        .playtime_report(&id, &input, library.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(restored["report"], report["report"]);
}

fn reviewed_request(library: &Library, id: &str, mut input: Request) -> Request {
    input.award_reviews = library
        .playtime_awards(
            id,
            &input.start_month,
            &input.end_month,
            library.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap()
        .into_iter()
        .map(|row| grapevine_workbench::playtime::AwardReview {
            entry_id: row.entry_id,
            entry_hash: row.entry_hash,
            class: grapevine_workbench::playtime::AwardClass::Regular,
            reason: "PRIVATE_CLASSIFICATION_REASON".into(),
            reference: "PRIVATE_CLASSIFICATION_REFERENCE".into(),
        })
        .collect();
    input
}
