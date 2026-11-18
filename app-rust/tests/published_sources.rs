use grapevine_workbench::{
    model::{BuildKind, Role},
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::{ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID},
    purchases::{PurchaseRequest, TeacherRule},
    store::Library,
    xp::Baseline,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::path::Path;
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

fn source_request(index: usize) -> grapevine_workbench::published_reference::Request {
    grapevine_workbench::published_reference::Request {
        id: Uuid::new_v4().to_string(),
        trait_index: index,
        source_id: "v20-core:discipline:animalism:1:feral-whispers".into(),
        day: "2026-01-03".into(),
        actor: "PRIVATE_SOURCE_ACTOR".into(),
        reason: "PRIVATE_SOURCE_REASON".into(),
        clear: false,
        rename_to_source: false,
        confirm_alias: false,
        confirm_index_evidence: false,
    }
}
fn seeded() -> (TempDir, Library, String) {
    let (dir, mut lib, id) = fixture("10.25");
    let request = request(ANIMALISM_FERAL_WHISPERS_ID);
    let quote = lib
        .quote_purchase(&id, &request, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    lib.commit_purchase(&quote, Role::Storyteller).unwrap();
    (dir, lib, id)
}
#[test]
fn installed_partial_catalogue_has_verified_dual_pages_and_no_invented_physical_names() {
    use grapevine_workbench::published_powers::{Coverage, Evidence, catalogue};
    let c = catalogue().unwrap();
    assert_eq!(c.entries.len(), 288);
    assert_eq!(
        c.entries
            .iter()
            .filter(|power| power.book_id == "v20-core")
            .count(),
        233
    );
    assert_eq!(c.book("da20-core").unwrap().coverage, Coverage::Partial);
    assert_eq!(
        c.entries
            .iter()
            .filter(|power| power.book_id == "da20-core")
            .count(),
        55
    );
    let da20 = c
        .power("da20-core:discipline:abombwe:1:predators-communion")
        .unwrap();
    assert_eq!(
        (
            da20.name.as_str(),
            da20.level,
            da20.printed_page,
            da20.pdf_page
        ),
        ("Predator’s Communion", 1, Some(189), 190)
    );
    let whispers = c
        .entries
        .iter()
        .find(|power| power.name == "Whispers to the Soul" && power.book_id == "v20-core")
        .unwrap();
    assert_eq!(
        (whispers.level, whispers.printed_page, whispers.pdf_page),
        (1, Some(173), 181)
    );
    for (id, level, printed, pdf) in [
        (
            "v20-core:discipline:animalism:1:feral-whispers",
            1,
            129,
            137,
        ),
        (
            "v20-core:discipline:animalism:3:quell-the-beast",
            3,
            130,
            138,
        ),
        (
            "v20-core:discipline:animalism:4:subsume-the-spirit",
            4,
            131,
            139,
        ),
        (
            "v20-core:discipline:animalism:5:drawing-out-the-beast",
            5,
            132,
            140,
        ),
    ] {
        let p = c.power(id).unwrap();
        assert_eq!(
            (p.level, p.printed_page, p.pdf_page),
            (level, Some(printed), pdf)
        );
        assert_eq!(p.evidence, Evidence::PageText);
    }
    assert!(!c.entries.iter().any(|p| {
        ["Celerity", "Fortitude", "Potence"].contains(&p.discipline.as_str()) && p.level <= 5
    }));
    let wire = serde_json::to_string(c).unwrap();
    for forbidden in [
        "system_text",
        "source_file",
        "raw_marker",
        "\\\\",
        "\"tier\"",
        "\"price\"",
        "\"body\"",
    ] {
        assert!(!wire.contains(forbidden), "{forbidden}");
    }
}
#[test]
fn attribution_is_readonly_until_commit_and_preserves_xp_value_original_and_met_receipt() {
    let (_dir, mut lib, id) = seeded();
    let before = raw(&lib.path, &id);
    let old: Value = serde_json::from_str(&before).unwrap();
    let revision = lib.revision().unwrap();
    let quote = lib
        .quote_published_reference(&id, &source_request(0), revision, Role::Storyteller)
        .unwrap();
    assert_eq!(quote.reference.as_ref().unwrap().power.level, 1);
    assert_eq!(raw(&lib.path, &id), before);
    let saved = lib
        .commit_published_reference(&quote, Role::Storyteller)
        .unwrap();
    let after: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    assert_eq!(after["xp_ledger"], old["xp_ledger"]);
    assert_eq!(after["original"], old["original"]);
    assert_eq!(lib.revision().unwrap(), revision + 1);
    let context = lib
        .published_reference_context(&id, Role::Storyteller)
        .unwrap();
    assert_eq!(context.history.len(), 1);
    assert_eq!(context.traits[0]["Value"], quote.updated_trait["Value"]);
    assert_eq!(
        context.traits[0]["AppSourcePowerId"],
        ANIMALISM_FERAL_WHISPERS_ID
    );
    assert!(
        grapevine_workbench::published_reference::verified_reference(&context.traits[0]).is_some()
    );
    assert!(
        !lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .findings
            .iter()
            .any(|f| f.code == "PURCHASE_TRAIT_DRIFT")
    );
    let before_retry = raw(&lib.path, &id);
    let audit = audit_count(&lib.path);
    let rev = lib.revision().unwrap();
    assert_eq!(
        lib.commit_published_reference(&quote, Role::Storyteller)
            .unwrap(),
        saved
    );
    assert_eq!(raw(&lib.path, &id), before_retry);
    assert_eq!(lib.revision().unwrap(), rev);
    assert_eq!(audit_count(&lib.path), audit);
}
#[test]
fn alias_and_renaming_are_explicit_and_published_level_never_replaces_game_value() {
    let (_dir, mut lib, id) = seeded();
    let mut request = source_request(0);
    request.source_id = "v20-core:discipline:animalism:3:quell-the-beast".into();
    let revision = lib.revision().unwrap();
    assert_eq!(
        lib.quote_published_reference(&id, &request, revision, Role::Storyteller)
            .unwrap_err()
            .code,
        "PUBLISHED_REFERENCE_NAME"
    );
    request.confirm_alias = true;
    let quote = lib
        .quote_published_reference(&id, &request, revision, Role::Storyteller)
        .unwrap();
    assert_eq!(quote.updated_trait["Name"], "Feral Whispers");
    assert_eq!(quote.updated_trait["Value"], "basic");
    assert_eq!(quote.reference.as_ref().unwrap().power.level, 3);
    request.confirm_alias = false;
    request.rename_to_source = true;
    let quote = lib
        .quote_published_reference(&id, &request, revision, Role::Storyteller)
        .unwrap();
    assert_eq!(quote.updated_trait["Name"], "Quell the Beast");
    assert_eq!(quote.updated_trait["Value"], "basic");
    lib.commit_published_reference(&quote, Role::Storyteller)
        .unwrap();
    assert_eq!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("6.25")
    );
    assert!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .findings
            .iter()
            .any(|f| f.code == "PURCHASE_TRAIT_DRIFT")
    );
}
#[test]
fn source_reference_printing_is_scoped_and_never_includes_staff_review_data() {
    use grapevine_workbench::printing::{self, Options, PrintScope};
    let (_dir, mut lib, id) = seeded();
    // This fixture explicitly makes its existing trait list public. Source attribution
    // itself must never promote a private list into Player output.
    let mut public_record: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    public_record["public_fields"]["DisciplineList"] =
        public_record["private_fields"]["DisciplineList"].clone();
    Connection::open(&lib.path)
        .unwrap()
        .execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![public_record.to_string(), id],
        )
        .unwrap();

    let quote = lib
        .quote_published_reference(
            &id,
            &source_request(0),
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    lib.commit_published_reference(&quote, Role::Storyteller)
        .unwrap();
    let doc = lib
        .print_document(
            &id,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap();
    assert_eq!(doc.published_references.len(), 1);
    assert_eq!(
        doc.published_references[0].source.power.printed_page,
        Some(129)
    );
    assert_eq!(doc.published_references[0].source.power.pdf_page, 137);
    for doc in [
        doc,
        lib.print_document(
            &id,
            &Options {
                scope: PrintScope::PrivateStoryteller,
                include_notes: true,
                ..Default::default()
            },
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap(),
    ] {
        let json = serde_json::to_string(&doc).unwrap();
        let html = printing::html(&doc).unwrap();
        for secret in [
            "PRIVATE_SOURCE_ACTOR",
            "PRIVATE_SOURCE_REASON",
            "_AppPublishedReferenceReviews",
        ] {
            assert!(!json.contains(secret));
            assert!(!html.contains(secret));
        }
        assert!(html.contains("published level 1"));
        assert!(html.contains("PDF"));
    }
    let doc = lib
        .print_document(
            &id,
            &Options {
                include_references: false,
                ..Default::default()
            },
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap();
    assert!(doc.published_references.is_empty());
    let view = lib.record(&id, Role::Player).unwrap();
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("PRIVATE_SOURCE_")
    );
}
#[test]
fn clearing_a_reference_is_reviewed_reversible_metadata_not_a_refund_or_trait_deletion() {
    let (_dir, mut lib, id) = seeded();
    let quote = lib
        .quote_published_reference(
            &id,
            &source_request(0),
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    lib.commit_published_reference(&quote, Role::Storyteller)
        .unwrap();
    let before: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    let mut clear = source_request(0);
    clear.clear = true;
    clear.source_id.clear();
    let quote = lib
        .quote_published_reference(&id, &clear, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert!(quote.reference.is_none());
    lib.commit_published_reference(&quote, Role::Storyteller)
        .unwrap();
    let after: Value = serde_json::from_str(&raw(&lib.path, &id)).unwrap();
    assert_eq!(before["xp_ledger"], after["xp_ledger"]);
    let context = lib
        .published_reference_context(&id, Role::Storyteller)
        .unwrap();
    assert_eq!(context.traits[0]["Name"], "Feral Whispers");
    assert!(
        context.traits[0]
            .get(grapevine_workbench::published_reference::TRAIT_FIELD)
            .is_none()
    );
    assert_eq!(context.history.len(), 2);
}
#[test]
fn roles_stale_state_tampered_quote_and_locked_drafts_cannot_write() {
    let (_dir, mut lib, id) = seeded();
    let request = source_request(0);
    let revision = lib.revision().unwrap();
    assert!(lib.published_reference_context(&id, Role::Player).is_err());
    assert!(
        lib.quote_published_reference(&id, &request, revision, Role::Player)
            .is_err()
    );
    let quote = lib
        .quote_published_reference(&id, &request, revision, Role::Storyteller)
        .unwrap();
    assert!(
        lib.commit_published_reference(&quote, Role::Player)
            .is_err()
    );
    let before = raw(&lib.path, &id);
    let mut forged = quote.clone();
    forged.updated_trait["Value"] = json!("Advanced");
    assert!(
        lib.commit_published_reference(&forged, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before);
    lib.create_character("Mortal", "Other", revision, Role::Storyteller)
        .unwrap();
    assert!(
        lib.commit_published_reference(&quote, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before);
    let ordinary = lib
        .create_build(
            &id,
            BuildKind::Draft,
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert!(
        lib.quote_published_reference(
            &ordinary,
            &request,
            lib.revision().unwrap(),
            Role::Storyteller
        )
        .is_ok()
    );
    let planned = lib
        .save_advancement_draft(
            &id,
            &[request_purchase_second()],
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(
        lib.quote_published_reference(
            &planned,
            &request,
            lib.revision().unwrap(),
            Role::Storyteller
        )
        .unwrap_err()
        .code,
        "PUBLISHED_REFERENCE_LOCKED_DRAFT"
    );
}
fn request_purchase_second() -> PurchaseRequest {
    request(ANIMALISM_BECKONING_ID)
}
#[test]
fn database_failure_rolls_back_attribution_history_revision_and_audit() {
    let (_dir, mut lib, id) = seeded();
    let quote = lib
        .quote_published_reference(
            &id,
            &source_request(0),
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    let before = raw(&lib.path, &id);
    let revision = lib.revision().unwrap();
    let audit = audit_count(&lib.path);
    Connection::open(&lib.path).unwrap().execute_batch("CREATE TRIGGER synthetic_source_failure BEFORE INSERT ON audit WHEN NEW.operation='published_source_attribution' BEGIN SELECT RAISE(ABORT,'synthetic source failure'); END;").unwrap();
    assert!(
        lib.commit_published_reference(&quote, Role::Storyteller)
            .is_err()
    );
    assert_eq!(raw(&lib.path, &id), before);
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(audit_count(&lib.path), audit);
}
#[test]
fn later_trait_or_source_metadata_drift_omits_the_reference_without_touching_xp() {
    let (_dir, mut lib, id) = seeded();
    let quote = lib
        .quote_published_reference(
            &id,
            &source_request(0),
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    let mut row = quote.updated_trait.clone();
    assert!(grapevine_workbench::published_reference::verified_reference(&row).is_some());
    row["Value"] = json!("Advanced");
    assert!(grapevine_workbench::published_reference::verified_reference(&row).is_none());
    row = quote.updated_trait.clone();
    row[grapevine_workbench::published_reference::TRAIT_FIELD]["facts_hash"] = json!("bad");
    assert!(grapevine_workbench::published_reference::verified_reference(&row).is_none());
    lib.commit_published_reference(&quote, Role::Storyteller)
        .unwrap();
    assert_eq!(
        lib.xp_view(&id, Role::Storyteller)
            .unwrap()
            .unspent
            .as_deref(),
        Some("6.25")
    );
}

#[test]
fn separate_copy_inherits_source_facts_without_copying_old_record_review_identity() {
    let (_dir, mut lib, id) = seeded();
    let quote = lib
        .quote_published_reference(
            &id,
            &source_request(0),
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    lib.commit_published_reference(&quote, Role::Storyteller)
        .unwrap();
    let copy = lib
        .create_build(
            &id,
            BuildKind::Draft,
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    let context = lib
        .published_reference_context(&copy, Role::Storyteller)
        .unwrap();
    assert!(context.history.is_empty());
    assert!(
        grapevine_workbench::published_reference::verified_reference(&context.traits[0]).is_some()
    );
    let quote = lib
        .quote_published_reference(
            &copy,
            &source_request(0),
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    lib.commit_published_reference(&quote, Role::Storyteller)
        .unwrap();
    assert_eq!(
        lib.published_reference_context(&id, Role::Storyteller)
            .unwrap()
            .history
            .len(),
        1
    );
    assert_eq!(
        lib.published_reference_context(&copy, Role::Storyteller)
            .unwrap()
            .history
            .len(),
        1
    );
}
