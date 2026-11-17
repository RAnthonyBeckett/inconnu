use grapevine_workbench::{
    model::{BuildKind, Role},
    pricing::PricingProfile,
    printing::Options,
    store::Library,
    trait_advancement::{Request as Advance, Target},
    trait_pricing::Category,
    vampire_creation as creation,
    xp::Baseline,
};
use serde_json::{Value, json};
use tempfile::TempDir;
const ST: Role = Role::Storyteller;

#[test]
fn canonical_initial_mapping_requires_fresh_review_and_private_receipt_proof() {
    use grapevine_workbench::{
        creation_sources as sources, purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    };
    let (_dir, mut lib, id, mut request) = fixture();
    let row = &mut request.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"][0];
    sources::choose(row, "v20-core:discipline:animalism:1:feral-whispers").unwrap();
    row[sources::PROPOSED]["reviewed_signature"] = json!(sources::review_signature(row).unwrap());
    sources::choose_met(row, Some(ANIMALISM_FERAL_WHISPERS_ID)).unwrap();
    assert!(!quote(&lib, &id, &request).ready);
    let row = &mut request.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"][0];
    row[sources::PROPOSED]["reviewed_signature"] = json!(sources::review_signature(row).unwrap());
    let q = quote(&lib, &id, &request);
    assert!(q.ready, "{:?}", q.issues);
    assert_eq!(q.met_mappings.len(), 1);
    let receipt = lib.commit_vampire_creation(&q, ST).unwrap();
    let raw: String = rusqlite::Connection::open(&lib.path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let record: grapevine_workbench::model::Record = serde_json::from_str(&raw).unwrap();
    let row = &receipt.quote.effective_sheet.lists["DisciplineList"]["Traits"][0];
    assert_eq!(
        sources::verified_met(&record, row).as_deref(),
        Some(ANIMALISM_FERAL_WHISPERS_ID)
    );
    let mut forged = row.clone();
    forged[sources::MET_FIELD]["review_id"] = json!(uuid::Uuid::new_v4().to_string());
    assert!(sources::verified_met(&record, &forged).is_none());
    let mut renamed = row.clone();
    renamed["Name"] = json!("Edited after mapping");
    assert!(sources::verified_met(&record, &renamed).is_none());
    let doc = lib
        .print_document(
            &id,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap();
    assert!(doc.references.iter().any(|r| r.power == "Feral Whispers"));
    let copy = lib
        .create_build(&id, BuildKind::Test, lib.revision().unwrap(), ST)
        .unwrap();
    let copy_doc = lib
        .print_document(&copy, &Options::default(), lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        copy_doc
            .references
            .iter()
            .any(|r| r.power == "Feral Whispers")
    );
    assert!(
        lib.print_document(
            &copy,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Player
        )
        .is_err()
    );
    assert!(
        !doc.sections
            .iter()
            .any(|s| s.title.starts_with("Simple mechanics"))
    );
    assert!(lib.xp_view(&id, ST).unwrap().ledger.is_none());
    assert_eq!(lib.purchase_receipts(&id, ST).unwrap().len(), 0);
}

#[test]
fn choosing_a_published_name_never_converts_its_level_to_a_met_tier() {
    use grapevine_workbench::creation_sources as sources;
    let (_dir, lib, id, mut request) = fixture();
    let row = &mut request.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"][0];
    row["Value"] = json!("intermediate");
    let before = row.clone();
    assert!(sources::choose(row, "unknown-source").is_err());
    assert_eq!(*row, before);
    sources::choose(row, "v20-core:discipline:animalism:1:feral-whispers").unwrap();
    assert_eq!(row["Value"], "intermediate");
    row[sources::PROPOSED]["reviewed_signature"] =
        json!(grapevine_workbench::published_reference::trait_signature(row).unwrap());
    assert!(!quote(&lib, &id, &request).ready);
}

#[test]
fn selected_published_source_and_explicit_met_review_commit_with_creation_atomically() {
    use grapevine_workbench::{creation_sources as sources, published_reference as source};
    let (_dir, mut lib, id, mut request) = fixture();
    let row = &mut request.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"][0];
    sources::choose(row, "v20-core:discipline:animalism:1:feral-whispers").unwrap();
    let q = quote(&lib, &id, &request);
    assert!(!q.ready);
    assert!(q.published_sources.is_empty());
    let row = &mut request.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"][0];
    row[sources::PROPOSED]["reviewed_signature"] = json!(source::trait_signature(row).unwrap());
    let q = quote(&lib, &id, &request);
    assert!(q.ready, "{:?}", q.issues);
    assert_eq!(q.published_sources.len(), 1);
    assert!(
        source::verified_reference(&q.effective_sheet.lists["DisciplineList"]["Traits"][0])
            .is_some()
    );
    assert!(
        q.effective_sheet.lists["DisciplineList"]["Traits"][0]
            .get(sources::PROPOSED)
            .is_none()
    );
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_creation_source BEFORE INSERT ON audit WHEN NEW.operation='vampire_creation_apply' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    assert!(lib.commit_vampire_creation(&q, ST).is_err());
    assert!(
        lib.published_reference_context(&id, ST)
            .unwrap()
            .history
            .is_empty()
    );
    conn.execute_batch("DROP TRIGGER fail_creation_source;")
        .unwrap();
    let receipt = lib.commit_vampire_creation(&q, ST).unwrap();
    assert_eq!(lib.commit_vampire_creation(&q, ST).unwrap(), receipt);
    let context = lib.published_reference_context(&id, ST).unwrap();
    assert_eq!(context.history.len(), 1);
    let doc = lib
        .print_document(
            &id,
            &Options::default(),
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap();
    assert_eq!(doc.published_references.len(), 1);
    let text = serde_json::to_string(&doc).unwrap();
    assert!(text.contains("Feral Whispers"));
    assert!(!text.contains("PRIVATE_REVIEWER"));
    let mut changed = request.clone();
    changed.id = uuid::Uuid::new_v4().to_string();
    changed.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"][0]["Name"] =
        json!("Changed alias after review");
    assert!(!quote(&lib, &id, &changed).ready);
}

#[test]
fn older_creation_receipt_without_new_source_quote_field_still_opens_xp() {
    let (_dir, mut lib, id, request) = fixture();
    let q = quote(&lib, &id, &request);
    lib.commit_vampire_creation(&q, ST).unwrap();
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    let data: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut record: Value = serde_json::from_str(&data).unwrap();
    record["private_fields"][creation::FIELD][0]["quote"]
        .as_object_mut()
        .unwrap()
        .remove("published_sources");
    record["private_fields"][creation::FIELD][0]["quote"]
        .as_object_mut()
        .unwrap()
        .remove("met_mappings");
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    assert_eq!(
        lib.vampire_creation_context(&id, ST).unwrap().history.len(),
        1
    );
    lib.initialize_xp(&id, &baseline(), lib.revision().unwrap(), ST)
        .unwrap();
}

#[test]
fn sabbat_virtue_choices_specializations_and_generation_baseline_are_explicit() {
    let (_dir, lib, id, mut request) = fixture();
    request.budget.sabbat = true;
    request.budget.conviction = true;
    request.budget.instinct = true;
    request.budget.virtue_allocated = [3, 2, 2];
    request.sheet.lists.get_mut("BackgroundList").unwrap()["Traits"] = json!([]);
    request.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"]
        .as_array_mut()
        .unwrap()
        .push(json!({"Name":"Synthetic fourth","Value":"basic","Note":""}));
    request
        .sheet
        .identity
        .insert("Path".into(), "Synthetic reviewed Path".into());
    request.specializations.push("Academics".into());
    let q = quote(&lib, &id, &request);
    assert!(q.ready, "{:?}", q.issues);
    assert_eq!(q.generation.generation, 13);
    assert_eq!(q.budget.virtues, [3, 2, 3]);
    assert_eq!(q.budget.morality, 3);
    assert_eq!(q.budget.free_spent, 1);
    request.sheet.lists.get_mut("BackgroundList").unwrap()["Traits"] =
        json!([{"Name":"Generation","Value":"0","Note":""}]);
    assert_eq!(quote(&lib, &id, &request).generation.generation, 13);

    request.specializations.push("academics".into());
    assert!(!quote(&lib, &id, &request).ready);
    request.specializations.clear();
    request.standard_pools = false;
    assert!(!quote(&lib, &id, &request).ready);
    request.standard_pools = true;
    request.sheet.resources.insert("SelfControl".into(), 3);
    assert!(!quote(&lib, &id, &request).ready);
}
fn fixture() -> (TempDir, Library, String, creation::Request) {
    let dir = TempDir::new().unwrap();
    let mut lib =
        Library::create(&dir.path().join("creation.gvlib"), "Synthetic creation").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic newborn", 0, ST)
        .unwrap();
    let mut request = lib.vampire_creation_context(&id, ST).unwrap().request;
    for (key, rows) in [
        (
            "PhysicalList",
            json!([{"Name":"Strong","Value":"7","Note":"PRIVATE_TRAIT"}]),
        ),
        (
            "SocialList",
            json!([{"Name":"Charming","Value":"5","Note":""}]),
        ),
        (
            "MentalList",
            json!([{"Name":"Alert","Value":"3","Note":""}]),
        ),
        (
            "AbilityList",
            json!([{"Name":"Academics","Value":"4","Note":""},{"Name":"Athletics","Value":"1","Note":""}]),
        ),
        (
            "BackgroundList",
            json!([{"Name":"Generation","Value":"2","Note":""},{"Name":"Resources","Value":"3","Note":""}]),
        ),
        (
            "DisciplineList",
            json!([{"Name":"Synthetic first","Value":"basic","Note":""},{"Name":"Synthetic second","Value":"basic","Note":""},{"Name":"Synthetic third","Value":"basic","Note":""}]),
        ),
    ] {
        request.sheet.lists.get_mut(key).unwrap()["Traits"] = rows;
    }
    request.budget.virtue_allocated = [3, 2, 2];
    request.standard_pools = true;
    request.review_confirmed = true;
    request.actor = "PRIVATE_REVIEWER".into();
    request.day = "2026-11-13".into();
    request.reason = "PRIVATE_REASON".into();
    request.policy_version = "Synthetic local test v1".into();
    request.review_reference =
        "Synthetic concept, clan, starting powers and progression review".into();
    request.manual_power_reference =
        "Synthetic book / edition / page and Basic progression declaration".into();
    (dir, lib, id, request)
}
fn quote(lib: &Library, id: &str, r: &creation::Request) -> creation::Quote {
    lib.quote_vampire_creation(id, r, lib.revision().unwrap(), ST)
        .unwrap()
}
fn baseline() -> Baseline {
    Baseline {
        day: "2026-11-13".into(),
        earned: "10".into(),
        unspent: "10".into(),
        actor: "Synthetic".into(),
        reason: "Opening after creation".into(),
        evidence: "Synthetic".into(),
    }
}
#[test]
fn creation_round_trip_print_xp_advance_correction_and_recovery() {
    let (dir, mut lib, id, request) = fixture();
    let q = quote(&lib, &id, &request);
    assert!(q.ready, "{:?}", q.issues);
    assert_eq!(q.budget.free_remaining, 5);
    assert_eq!(q.generation.generation, 11);
    assert_eq!(q.effective_sheet.resources["Willpower"], 4);
    assert_eq!(q.effective_sheet.resources["Blood"], 12);
    assert!(!q.effective_sheet.resources.contains_key("TempBlood"));
    let receipt = lib.commit_vampire_creation(&q, ST).unwrap();
    let committed = lib.revision().unwrap();
    assert_eq!(lib.commit_vampire_creation(&q, ST).unwrap(), receipt);
    assert_eq!(lib.revision().unwrap(), committed);
    assert!(lib.xp_view(&id, ST).unwrap().ledger.is_none());
    let public = lib.record(&id, Role::Player).unwrap();
    assert_eq!(public.fields["PhysicalList"]["Traits"][0]["Value"], "7");
    for role in [Role::Player, ST] {
        let view = lib.record(&id, role).unwrap();
        assert!(!view.fields.contains_key(creation::FIELD));
    }
    let doc = lib
        .print_document(&id, &Options::default(), committed, Role::Player)
        .unwrap();
    let rendered = serde_json::to_string(&doc).unwrap();
    for secret in [
        "PRIVATE_TRAIT",
        "PRIVATE_REVIEWER",
        "PRIVATE_REASON",
        creation::FIELD,
    ] {
        assert!(!rendered.contains(secret));
    }
    assert!(rendered.contains("Strong"));
    assert!(rendered.contains("Humanity"));
    let backup = dir.path().join("creation-backup.gvlib");
    lib.backup(&backup, ST).unwrap();
    let path = lib.path.clone();
    drop(lib);
    let mut lib = Library::open(&path).unwrap();
    assert_eq!(
        lib.vampire_creation_context(&id, ST).unwrap().history.len(),
        1
    );
    lib.initialize_xp(&id, &baseline(), lib.revision().unwrap(), ST)
        .unwrap();
    assert!(!lib.vampire_creation_context(&id, ST).unwrap().editable);
    assert!(
        lib.quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
            .is_err()
    );
    let advance = Advance {
        id: uuid::Uuid::new_v4().to_string(),
        category: Category::Ability,
        target: Target::Trait {
            field: "AbilityList".into(),
            index: Some(0),
            name: "Academics".into(),
        },
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        show_on_player_sheet: true,
        day: "2026-11-14".into(),
        actor: "Synthetic".into(),
        reason: "Advance after creation".into(),
        policy_version: "Synthetic".into(),
        review_reference: "Reviewed".into(),
        review_confirmed: true,
    };
    let a = lib
        .quote_trait_advancement(&id, &advance, lib.revision().unwrap(), ST)
        .unwrap();
    lib.commit_trait_advancement(&a, ST).unwrap();
    assert_eq!(lib.xp_view(&id, ST).unwrap().unspent.as_deref(), Some("9"));
    let correction = grapevine_workbench::trait_corrections::Request {
        id: uuid::Uuid::new_v4().to_string(),
        advancement_id: advance.id,
        day: "2026-11-14".into(),
        actor: "Synthetic".into(),
        reason: "Clerical correction".into(),
        evidence: "Synthetic".into(),
    };
    let c = lib
        .quote_trait_correction(&id, &correction, lib.revision().unwrap(), ST)
        .unwrap();
    lib.commit_trait_correction(&c, ST).unwrap();
    assert_eq!(lib.xp_view(&id, ST).unwrap().unspent.as_deref(), Some("10"));
    lib.restore(&backup, lib.revision().unwrap(), ST).unwrap();
    assert!(lib.xp_view(&id, ST).unwrap().ledger.is_none());
    assert_eq!(
        lib.vampire_creation_context(&id, ST).unwrap().history[0],
        receipt
    );
    lib.initialize_xp(&id, &baseline(), lib.revision().unwrap(), ST)
        .unwrap();
}
#[test]
fn saved_draft_is_private_does_not_grant_traits_and_requires_explicit_stale_resolution() {
    let (_dir, mut lib, id, mut request) = fixture();
    request.budget.virtue_allocated = [0; 3];
    lib.save_vampire_creation_draft(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        !lib.record(&id, Role::Player)
            .unwrap()
            .fields
            .contains_key("PhysicalList")
    );
    assert_eq!(
        lib.vampire_creation_context(&id, ST).unwrap().request,
        request
    );
    assert!(
        lib.initialize_xp(&id, &baseline(), lib.revision().unwrap(), ST)
            .is_err()
    );
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.identity.insert("Clan".into(), "Changed".into());
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(
        lib.quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
            .unwrap_err()
            .code,
        "CREATION_DRAFT_STALE"
    );
    assert!(
        lib.save_vampire_creation_draft(&id, &request, lib.revision().unwrap(), ST)
            .is_err()
    );
    assert_eq!(
        lib.vampire_creation_context(&id, ST).unwrap().request,
        request
    );
    lib.discard_vampire_creation_draft(&id, lib.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(
        lib.vampire_creation_context(&id, ST)
            .unwrap()
            .request
            .sheet
            .identity["Clan"],
        "Changed"
    );
}
#[test]
fn totals_come_from_sheet_caps_powers_resources_and_manual_gates_are_checked() {
    let (_dir, lib, id, mut request) = fixture();
    request.budget.attribute_totals = [4096; 3];
    request.budget.basic_power_count = 0;
    assert!(quote(&lib, &id, &request).ready); // Editable totals cannot fake the actual sheet.
    request.sheet.lists.get_mut("PhysicalList").unwrap()["Traits"][0]["Value"] = json!("13");
    assert!(!quote(&lib, &id, &request).ready);
    request.sheet.lists.get_mut("PhysicalList").unwrap()["Traits"][0]["Value"] = json!("6");
    assert!(!quote(&lib, &id, &request).ready); // Other unused/free pools cannot replace a base Trait.
    request.sheet.lists.get_mut("PhysicalList").unwrap()["Traits"][0]["Value"] = json!("7");
    request.manual_power_reference.clear();
    assert!(!quote(&lib, &id, &request).ready);
    request.manual_power_reference = "Reviewed manual mapping".into();
    request.sheet.resources.insert("TempBlood".into(), 12);
    assert!(!quote(&lib, &id, &request).ready);
    request.current_resources_reference = "Game-start test and modifiers".into();
    assert!(quote(&lib, &id, &request).ready);
    request.sheet.resources.insert("TempBlood".into(), 13);
    assert!(!quote(&lib, &id, &request).ready);
}
#[test]
fn stale_tamper_wrong_role_retry_conflict_and_atomic_rollback() {
    let (_dir, mut lib, id, request) = fixture();
    let q = quote(&lib, &id, &request);
    assert!(
        lib.quote_vampire_creation(&id, &request, q.revision, Role::Player)
            .is_err()
    );
    assert!(lib.commit_vampire_creation(&q, Role::Player).is_err());
    let mut tampered = q.clone();
    tampered
        .effective_sheet
        .resources
        .insert("Blood".into(), 99);
    assert!(lib.commit_vampire_creation(&tampered, ST).is_err());
    assert_eq!(lib.revision().unwrap(), q.revision);
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_creation BEFORE INSERT ON audit WHEN NEW.operation='vampire_creation_apply' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    assert!(lib.commit_vampire_creation(&q, ST).is_err());
    assert_eq!(lib.revision().unwrap(), q.revision);
    assert!(
        lib.vampire_creation_context(&id, ST)
            .unwrap()
            .history
            .is_empty()
    );
    conn.execute_batch("DROP TRIGGER fail_creation;").unwrap();
    lib.create_character("Mortal", "Concurrent", lib.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(
        lib.commit_vampire_creation(&q, ST).unwrap_err().code,
        "STALE_REVISION"
    );
    let q = quote(&lib, &id, &request);
    lib.commit_vampire_creation(&q, ST).unwrap();
    let mut conflict = q;
    conflict.request.reason = "Different".into();
    assert_eq!(
        lib.commit_vampire_creation(&conflict, ST).unwrap_err().code,
        "CREATION_ID_CONFLICT"
    );
}
#[test]
fn post_review_sheet_change_blocks_xp_and_build_copies_do_not_inherit_reviews() {
    let (_dir, mut lib, id, request) = fixture();
    let q = quote(&lib, &id, &request);
    lib.commit_vampire_creation(&q, ST).unwrap();
    let copy = lib
        .create_build(&id, BuildKind::Test, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(lib.vampire_creation_context(&copy, ST).is_err());
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    edit.lists.get_mut("AbilityList").unwrap()["Traits"][0]["Value"] = json!("3");
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    assert_eq!(
        lib.initialize_xp(&id, &baseline(), lib.revision().unwrap(), ST)
            .unwrap_err()
            .code,
        "CREATION_REVIEW_NEEDED"
    );
}
#[test]
fn imported_and_opaque_fields_are_protected() {
    let (_dir, mut lib, id, request) = fixture();
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    let data: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut record: Value = serde_json::from_str(&data).unwrap();
    record["private_fields"]["OpaqueOriginal"] = json!({"arbitrary":[1,2,3]});
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    let q = quote(&lib, &id, &request);
    lib.commit_vampire_creation(&q, ST).unwrap();
    assert_eq!(
        lib.record(&id, ST).unwrap().fields["OpaqueOriginal"],
        json!({"arbitrary":[1,2,3]})
    );
    let data: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut record: Value = serde_json::from_str(&data).unwrap();
    record["source_sha256"] = json!("imported");
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    assert!(lib.vampire_creation_context(&id, ST).is_err());
}
