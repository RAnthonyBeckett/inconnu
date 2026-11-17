use grapevine_workbench::{
    model::{BuildKind, Role},
    pricing::PricingProfile,
    printing::Options,
    sheet,
    store::Library,
    trait_advancement::{self as advance, Request, Target},
    trait_pricing::Category,
    xp::{Baseline, XpCommand, XpKind},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;
const ST: Role = Role::Storyteller;

fn fixture(ability: Option<&str>, unspent: &str) -> (TempDir, Library, String) {
    let dir = TempDir::new().unwrap();
    let mut lib = Library::create(&dir.path().join("traits.gvlib"), "Synthetic").unwrap();
    let id = lib.create_character("Vampire", "Synthetic", 0, ST).unwrap();
    if let Some(value) = ability {
        let mut edit = lib.sheet_edit(&id, ST).unwrap();
        let mut list = sheet::empty_list("AbilityList");
        list["Traits"] = json!([{"Name":"Academics","Value":value,"Note":"PRIVATE_ABILITY_NOTE"}]);
        edit.lists.insert("AbilityList".into(), list);
        lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
            .unwrap();
    }
    lib.initialize_xp(
        &id,
        &Baseline {
            day: "2026-01-01".into(),
            earned: unspent.into(),
            unspent: unspent.into(),
            actor: "Synthetic ST".into(),
            reason: "Synthetic opening".into(),
            evidence: "Synthetic".into(),
        },
        lib.revision().unwrap(),
        ST,
    )
    .unwrap();
    (dir, lib, id)
}
fn request(category: Category, target: Target) -> Request {
    Request {
        id: Uuid::new_v4().to_string(),
        category,
        target,
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        show_on_player_sheet: false,
        day: "2026-01-02".into(),
        actor: "PRIVATE_ACTOR".into(),
        reason: "PRIVATE_REASON".into(),
        policy_version: "synthetic-v1".into(),
        review_reference: "Synthetic eligibility, maxima, approval and storyline review".into(),
        review_confirmed: true,
    }
}
fn ability() -> Request {
    request(
        Category::Ability,
        Target::Trait {
            field: "AbilityList".into(),
            index: Some(0),
            name: "Academics".into(),
        },
    )
}
fn willpower() -> Request {
    request(
        Category::Willpower,
        Target::Resource {
            field: "Willpower".into(),
        },
    )
}
fn quote(lib: &Library, id: &str, r: &Request) -> advance::Quote {
    lib.quote_trait_advancement(id, r, lib.revision().unwrap(), ST)
        .unwrap()
}
fn unspent(lib: &Library, id: &str) -> String {
    lib.xp_view(id, ST).unwrap().unspent.unwrap()
}

#[test]
fn ability_band_atomic_retry_public_redaction_and_later_edit_audit() {
    let (_dir, mut lib, id) = fixture(Some("4"), "10.25");
    let mut first = ability();
    first.show_on_player_sheet = true;
    let q = quote(&lib, &id, &first);
    assert_eq!(
        (q.price.current, q.price.next, q.price.cost.as_str()),
        (4, 5, "1")
    );
    let receipt = lib.commit_trait_advancement(&q, ST).unwrap();
    let public = lib.record(&id, Role::Player).unwrap();
    assert_eq!(public.fields["AbilityList"]["Traits"][0]["Value"], "5");
    let doc = lib
        .print_document(
            &id,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap();
    let text = serde_json::to_string(&doc).unwrap();
    assert!(text.contains("Academics"));
    for secret in ["PRIVATE_ACTOR", "PRIVATE_REASON", "PRIVATE_ABILITY_NOTE"] {
        assert!(!text.contains(secret));
    }
    lib.create_character("Mortal", "Other", lib.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(
        lib.commit_trait_advancement(&q, ST)
            .unwrap()
            .committed_revision,
        receipt.committed_revision
    );
    assert_eq!(unspent(&lib, &id), "9.25");
    let second = quote(&lib, &id, &ability());
    assert_eq!(
        (
            second.price.current,
            second.price.next,
            second.price.cost.as_str()
        ),
        (5, 6, "2")
    );
    lib.commit_trait_advancement(&second, ST).unwrap();
    assert_eq!(unspent(&lib, &id), "7.25");
    let view = lib.xp_view(&id, ST).unwrap();
    assert!(
        !view
            .findings
            .iter()
            .any(|f| f.code == "TRAIT_ADVANCEMENT_SHEET_DRIFT")
    );
    let command = &view.ledger.unwrap().entries[0].command;
    assert!(
        advance::evidence_summary(command, false)
            .unwrap()
            .contains("p.124 / PDF125")
    );
    assert!(
        lib.post_xp(&id, command, lib.revision().unwrap(), ST)
            .is_err()
    );
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.lists.get_mut("AbilityList").unwrap()["Traits"][0]["Value"] = json!("7");
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        lib.xp_view(&id, ST)
            .unwrap()
            .findings
            .iter()
            .any(|f| f.code == "TRAIT_ADVANCEMENT_SHEET_DRIFT")
    );
    assert_eq!(unspent(&lib, &id), "7.25");
}

#[test]
fn private_list_stays_private_and_resource_purchase_cannot_be_refunded_by_ledger_only() {
    let (_dir, mut lib, id) = fixture(Some("1"), "10");
    lib.commit_trait_advancement(&quote(&lib, &id, &ability()), ST)
        .unwrap();
    assert!(
        !lib.record(&id, Role::Player)
            .unwrap()
            .fields
            .contains_key("AbilityList")
    );
    let q = quote(&lib, &id, &willpower());
    assert_eq!(q.price.cost, "3");
    lib.commit_trait_advancement(&q, ST).unwrap();
    assert_eq!(unspent(&lib, &id), "6");
    assert_eq!(lib.record(&id, ST).unwrap().fields["Willpower"], 1);
    assert!(
        !lib.record(&id, Role::Player)
            .unwrap()
            .fields
            .contains_key("Willpower")
    );
    let reversal = XpCommand {
        id: Uuid::new_v4().to_string(),
        kind: XpKind::Reversal,
        amount: String::new(),
        day: "2026-01-03".into(),
        actor: "Synthetic".into(),
        reason: "Synthetic refund attempt".into(),
        evidence: "Synthetic".into(),
        reversal_of: Some(q.request.id.clone()),
        cost: None,
    };
    assert!(
        lib.post_xp(&id, &reversal, lib.revision().unwrap(), ST)
            .is_err()
    );
    assert_eq!(unspent(&lib, &id), "6");
    drop(lib);
}

#[test]
fn stale_tampered_player_and_test_copy_cannot_apply_and_audit_failure_rolls_back() {
    let (_dir, mut lib, id) = fixture(None, "10");
    let q = quote(&lib, &id, &willpower());
    assert!(
        lib.quote_trait_advancement(&id, &willpower(), q.revision, Role::Player)
            .is_err()
    );
    assert!(lib.commit_trait_advancement(&q, Role::Player).is_err());
    let mut tampered = q.clone();
    tampered.price.cost = "1".into();
    assert!(lib.commit_trait_advancement(&tampered, ST).is_err());
    let copy = lib
        .create_build(&id, BuildKind::Test, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        lib.quote_trait_advancement(&copy, &willpower(), lib.revision().unwrap(), ST)
            .is_err()
    );
    assert!(lib.commit_trait_advancement(&q, ST).is_err());
    let q = quote(&lib, &id, &willpower());
    let conn = Connection::open(&lib.path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_trait_audit BEFORE INSERT ON audit WHEN NEW.operation='trait_advancement' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    assert!(lib.commit_trait_advancement(&q, ST).is_err());
    assert_eq!(lib.revision().unwrap(), q.revision);
    assert_eq!(unspent(&lib, &id), "10");
    assert!(
        !lib.record(&id, ST)
            .unwrap()
            .fields
            .contains_key("Willpower")
    );
    assert!(
        lib.trait_advancement_context(&id, ST)
            .unwrap()
            .history
            .is_empty()
    );
}

#[test]
fn unsupported_values_duplicate_labels_and_missing_review_do_not_gain_traits() {
    let (_dir, mut lib, id) = fixture(Some("descriptive"), "10");
    assert!(
        lib.quote_trait_advancement(&id, &ability(), lib.revision().unwrap(), ST)
            .is_err()
    );
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.lists.get_mut("AbilityList").unwrap()["Traits"] =
        json!([{"Name":"Academics","Value":"4"},{"Name":"ACADEMICS","Value":"1"}]);
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        lib.quote_trait_advancement(&id, &ability(), lib.revision().unwrap(), ST)
            .is_err()
    );
    for alteration in 0..3 {
        let mut r = willpower();
        match alteration {
            0 => r.review_confirmed = false,
            1 => r.profile = None,
            _ => {
                r.target = Target::Resource {
                    field: "Blood".into(),
                }
            }
        }
        assert!(
            lib.quote_trait_advancement(&id, &r, lib.revision().unwrap(), ST)
                .is_err()
        );
    }
    assert_eq!(unspent(&lib, &id), "10");
}

#[test]
fn future_award_cannot_fund_a_backdated_trait_or_a_negative_balance() {
    let (_dir, mut lib, id) = fixture(None, "0");
    assert!(
        lib.quote_trait_advancement(&id, &willpower(), lib.revision().unwrap(), ST)
            .is_err()
    );
    let award = XpCommand {
        id: Uuid::new_v4().to_string(),
        kind: XpKind::Award,
        amount: "5".into(),
        day: "2026-01-10".into(),
        actor: "Synthetic".into(),
        reason: "Synthetic award".into(),
        evidence: "Synthetic".into(),
        reversal_of: None,
        cost: None,
    };
    lib.post_xp(&id, &award, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        lib.quote_trait_advancement(&id, &willpower(), lib.revision().unwrap(), ST)
            .is_err()
    );
    assert_eq!(unspent(&lib, &id), "5");
    assert!(
        !lib.record(&id, ST)
            .unwrap()
            .fields
            .contains_key("Willpower")
    );
}

#[test]
fn ambiguous_legacy_ability_lists_are_not_replaced_by_a_new_empty_list() {
    let (_dir, lib, id) = fixture(None, "10");
    let conn = Connection::open(&lib.path).unwrap();
    let raw: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut record: Value = serde_json::from_str(&raw).unwrap();
    let list = json!({"Name":"Abilities","Traits":[{"Name":"Academics","Value":"2"}]});
    record["private_fields"]["UnmappedTraitLists"] = json!([list.clone(), list]);
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    let r = request(
        Category::Ability,
        Target::Trait {
            field: "AbilityList".into(),
            index: None,
            name: "Other".into(),
        },
    );
    assert!(
        lib.quote_trait_advancement(&id, &r, lib.revision().unwrap(), ST)
            .is_err()
    );
    assert_eq!(unspent(&lib, &id), "10");
}

fn correction(id: &str) -> grapevine_workbench::trait_corrections::Request {
    grapevine_workbench::trait_corrections::Request {
        id: Uuid::new_v4().to_string(),
        advancement_id: id.into(),
        day: "2026-01-03".into(),
        actor: "PRIVATE_CORRECTION_ACTOR".into(),
        reason: "Synthetic clerical correction".into(),
        evidence: "PRIVATE_CORRECTION_EVIDENCE".into(),
    }
}

#[test]
fn paired_corrections_refund_original_band_prices_in_reverse_dependency_order() {
    let (_dir, mut lib, id) = fixture(Some("4"), "10");
    let first = quote(&lib, &id, &ability());
    lib.commit_trait_advancement(&first, ST).unwrap();
    let second = quote(&lib, &id, &ability());
    lib.commit_trait_advancement(&second, ST).unwrap();
    assert!(
        lib.quote_trait_correction(
            &id,
            &correction(&first.request.id),
            lib.revision().unwrap(),
            ST
        )
        .is_err()
    );
    let q = lib
        .quote_trait_correction(
            &id,
            &correction(&second.request.id),
            lib.revision().unwrap(),
            ST,
        )
        .unwrap();
    assert_eq!(q.refund, "2");
    lib.commit_trait_correction(&q, ST).unwrap();
    assert_eq!(unspent(&lib, &id), "9");
    let row = &lib.record(&id, ST).unwrap().fields["AbilityList"]["Traits"][0];
    assert_eq!(row["Value"], "5");
    assert_eq!(row["AppTraitAdvancementId"], first.request.id);
    assert_eq!(row["Note"], "PRIVATE_ABILITY_NOTE");
    lib.create_character("Mortal", "Unrelated", lib.revision().unwrap(), ST)
        .unwrap();
    let revision = lib.revision().unwrap();
    lib.commit_trait_correction(&q, ST).unwrap();
    assert_eq!(lib.revision().unwrap(), revision);
    assert!(
        lib.quote_trait_correction(&id, &correction(&second.request.id), revision, ST)
            .is_err()
    );
    assert!(
        !lib.xp_view(&id, ST)
            .unwrap()
            .findings
            .iter()
            .any(|f| f.code == "TRAIT_ADVANCEMENT_SHEET_DRIFT")
    );
    let q = lib
        .quote_trait_correction(&id, &correction(&first.request.id), revision, ST)
        .unwrap();
    lib.commit_trait_correction(&q, ST).unwrap();
    assert_eq!(unspent(&lib, &id), "10");
    assert_eq!(
        lib.record(&id, ST).unwrap().fields["AbilityList"]["Traits"][0]["Value"],
        "4"
    );
    assert_eq!(
        lib.trait_advancement_context(&id, ST)
            .unwrap()
            .corrected_ids
            .len(),
        2
    );
}

#[test]
fn untouched_new_trait_can_be_removed_but_later_notes_and_changes_block_correction() {
    let (_dir, mut lib, id) = fixture(None, "10");
    let mut r = request(
        Category::Attribute,
        Target::Trait {
            field: "PhysicalList".into(),
            index: None,
            name: "Strong".into(),
        },
    );
    r.show_on_player_sheet = true;
    let grant = quote(&lib, &id, &r);
    lib.commit_trait_advancement(&grant, ST).unwrap();
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.lists.get_mut("PhysicalList").unwrap()["Traits"][0]["Note"] = json!("Preserve later note");
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        lib.quote_trait_correction(
            &id,
            &correction(&grant.request.id),
            lib.revision().unwrap(),
            ST
        )
        .is_err()
    );
    edit.lists.get_mut("PhysicalList").unwrap()["Traits"][0]["Note"] = json!("");
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    let q = lib
        .quote_trait_correction(
            &id,
            &correction(&grant.request.id),
            lib.revision().unwrap(),
            ST,
        )
        .unwrap();
    lib.commit_trait_correction(&q, ST).unwrap();
    assert!(
        lib.record(&id, Role::Player).unwrap().fields["PhysicalList"]["Traits"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(unspent(&lib, &id), "10");
    let text = serde_json::to_string(
        &lib.print_document(
            &id,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!text.contains("PRIVATE_CORRECTION"));
}

#[test]
fn correction_roles_stale_tamper_date_and_audit_failure_cannot_separate_refund_from_count() {
    let (_dir, mut lib, id) = fixture(None, "10");
    let grant = quote(&lib, &id, &willpower());
    lib.commit_trait_advancement(&grant, ST).unwrap();
    let mut r = correction(&grant.request.id);
    r.day = "2026-01-01".into();
    assert!(
        lib.quote_trait_correction(&id, &r, lib.revision().unwrap(), ST)
            .is_err()
    );
    r.day = "2026-01-03".into();
    assert!(
        lib.quote_trait_correction(&id, &r, lib.revision().unwrap(), Role::Player)
            .is_err()
    );
    let q = lib
        .quote_trait_correction(&id, &r, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(lib.commit_trait_correction(&q, Role::Player).is_err());
    let mut tampered = q.clone();
    tampered.refund = "20".into();
    assert!(lib.commit_trait_correction(&tampered, ST).is_err());
    let copy = lib
        .create_build(&id, BuildKind::Test, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        lib.quote_trait_correction(&copy, &r, lib.revision().unwrap(), ST)
            .is_err()
    );
    assert!(lib.commit_trait_correction(&q, ST).is_err());
    let q = lib
        .quote_trait_correction(&id, &r, lib.revision().unwrap(), ST)
        .unwrap();
    let conn = Connection::open(&lib.path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_correction BEFORE INSERT ON audit WHEN NEW.operation='trait_correction' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    assert!(lib.commit_trait_correction(&q, ST).is_err());
    assert_eq!(lib.revision().unwrap(), q.revision);
    assert_eq!(unspent(&lib, &id), "7");
    assert_eq!(lib.record(&id, ST).unwrap().fields["Willpower"], 1);
    conn.execute_batch("DROP TRIGGER fail_correction;").unwrap();
    lib.commit_trait_correction(&q, ST).unwrap();
    assert_eq!(unspent(&lib, &id), "10");
    assert_eq!(lib.record(&id, ST).unwrap().fields["Willpower"], 0);
    let command = lib
        .xp_view(&id, ST)
        .unwrap()
        .ledger
        .unwrap()
        .entries
        .last()
        .unwrap()
        .command
        .clone();
    assert!(
        advance::evidence_summary(&command, false)
            .unwrap()
            .contains("Original refund")
    );
    assert!(
        lib.post_xp(&id, &command, lib.revision().unwrap(), ST)
            .is_err()
    );
    let reverse = XpCommand {
        id: Uuid::new_v4().to_string(),
        kind: XpKind::Reversal,
        amount: String::new(),
        day: "2026-01-04".into(),
        actor: "Synthetic".into(),
        reason: "Synthetic".into(),
        evidence: "Synthetic".into(),
        reversal_of: Some(command.id),
        cost: None,
    };
    assert!(
        lib.post_xp(&id, &reverse, lib.revision().unwrap(), ST)
            .is_err()
    );
}
