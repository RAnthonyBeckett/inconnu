use grapevine_workbench::{
    model::{BuildKind, Edit, Role},
    pricing::{ClanRelation, PricingProfile},
    printing::{DocLanguage, Options},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{PurchaseRequest, TeacherRule},
    sheet,
    store::Library,
    xp::Baseline,
};
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn new_vampire_identity_xp_purchase_player_sheet_test_copy_and_recovery() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("new-game.gvlib");
    let backup = dir.path().join("after-purchase.gvbackup");
    let mut lib = Library::create(&path, "Synthetic new game").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic new Vampire", 0, Role::Storyteller)
        .unwrap();
    let mut sheet_edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    sheet_edit
        .identity
        .insert("Clan".into(), "Synthetic clan".into());
    sheet_edit
        .identity
        .insert("Nature".into(), "Synthetic nature".into());
    sheet_edit.resources.insert("Willpower".into(), 3);
    sheet_edit.visibility.insert("Willpower".into(), true);
    let mut ability = sheet::empty_list("AbilityList");
    ability["Traits"] =
        json!([{"Name":"Synthetic ability","Value":"2","Note":"PRIVATE_ABILITY_NOTE"}]);
    sheet_edit.lists.insert("AbilityList".into(), ability);
    sheet_edit.visibility.insert("AbilityList".into(), true);
    lib.save_character(
        &id,
        &Edit {
            name: "Synthetic new Vampire".into(),
            status: "Draft for ST review".into(),
            notes: "PRIVATE_ST_NOTE".into(),
        },
        &sheet_edit,
        1,
        Role::Storyteller,
    )
    .unwrap();
    lib.initialize_xp(
        &id,
        &Baseline {
            day: "2026-01-01".into(),
            earned: "10.25".into(),
            unspent: "10.25".into(),
            actor: "PRIVATE_ACTOR".into(),
            reason: "PRIVATE_OPENING_REASON".into(),
            evidence: "Synthetic recorded opening balance, not a creation budget".into(),
        },
        2,
        Role::Storyteller,
    )
    .unwrap();
    let request = PurchaseRequest {
        review_id: None,
        id: Uuid::new_v4().to_string(),
        power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(ClanRelation::OutOfClan),
        affiliation_name: "Synthetic clan".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic session 2026-01-03".into(),
        actor: "PRIVATE_ACTOR".into(),
        reason: "PRIVATE_PURCHASE_REASON".into(),
        policy_version: "synthetic-policy-v1".into(),
        policy_reference: "Synthetic ST adoption".into(),
        affiliation_evidence: "Synthetic out-of-clan review".into(),
        approval_reference: "Synthetic local approval declaration".into(),
        eligibility_evidence: "Synthetic eligibility declaration".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Synthetic qualified teacher declaration".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    };
    let quote = lib
        .quote_purchase(&id, &request, 3, Role::Storyteller)
        .unwrap();
    lib.commit_purchase(&quote, Role::Storyteller).unwrap();
    assert_eq!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("6.25")
    );
    assert_eq!(
        lib.purchase_receipts(&id, Role::Storyteller).unwrap().len(),
        1
    );
    lib.backup(&backup, Role::Storyteller).unwrap();
    drop(lib);

    let mut lib = Library::open(&path).unwrap();
    for language in [DocLanguage::English, DocLanguage::Portuguese] {
        let document = lib
            .print_document(
                &id,
                &Options {
                    language,
                    include_references: true,
                    ..Options::default()
                },
                4,
                Role::Player,
            )
            .unwrap();
        let text = serde_json::to_string(&document).unwrap();
        for visible in ["Synthetic clan", "Synthetic ability", "Feral Whispers"] {
            assert!(text.contains(visible), "{visible}");
        }
        for secret in [
            "PRIVATE_ST_NOTE",
            "PRIVATE_ABILITY_NOTE",
            "PRIVATE_ACTOR",
            "PRIVATE_OPENING_REASON",
            "PRIVATE_PURCHASE_REASON",
        ] {
            assert!(!text.contains(secret));
        }
    }
    assert_eq!(
        lib.record(&id, Role::Player).unwrap().fields["Willpower"],
        3
    );
    let test_id = lib
        .create_build(&id, BuildKind::Test, 4, Role::Storyteller)
        .unwrap();
    assert!(lib.record(&test_id, Role::Player).is_err());
    let mut test_sheet = lib.sheet_edit(&test_id, Role::Storyteller).unwrap();
    test_sheet
        .identity
        .insert("Clan".into(), "What-if alternate clan".into());
    lib.save_sheet(&test_id, &test_sheet, 5, Role::Storyteller)
        .unwrap();
    assert_eq!(
        lib.record(&id, Role::Player).unwrap().fields["Clan"],
        "Synthetic clan"
    );
    assert_eq!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("6.25")
    );
    lib.restore(&backup, 6, Role::Storyteller).unwrap();
    assert_eq!(
        lib.record(&id, Role::Player).unwrap().fields["Clan"],
        "Synthetic clan"
    );
    assert_eq!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("6.25")
    );
    assert_eq!(
        lib.purchase_receipts(&id, Role::Storyteller).unwrap().len(),
        1
    );
    assert!(lib.record(&test_id, Role::Storyteller).is_err());
}
