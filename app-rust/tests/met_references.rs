use grapevine_workbench::{
    met_references as met,
    model::{BuildKind, Role},
    printing::Options,
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    sheet,
    store::Library,
    xp::Baseline,
};
use serde_json::{Value, json};
use tempfile::TempDir;
const ST: Role = Role::Storyteller;
fn fixture() -> (TempDir, Library, String, met::Request) {
    let dir = TempDir::new().unwrap();
    let mut lib =
        Library::create(&dir.path().join("legacy-met.gvlib"), "Synthetic legacy").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic legacy <literal>", 0, ST)
        .unwrap();
    let mut edit = lib.sheet_edit(&id, ST).unwrap();
    let mut list = sheet::empty_list("DisciplineList");
    list["Traits"] = json!([{"Name":"Legacy animal speech","Value":"1","Note":"PRIVATE_MET_TRAIT","Opaque":{"keep":[1,2]}}]);
    edit.lists.insert("DisciplineList".into(), list);
    edit.visibility.insert("DisciplineList".into(), true);
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    lib.initialize_xp(
        &id,
        &Baseline {
            day: "2026-11-01".into(),
            earned: "10".into(),
            unspent: "10".into(),
            actor: "Synthetic".into(),
            reason: "Opening".into(),
            evidence: "Synthetic".into(),
        },
        lib.revision().unwrap(),
        ST,
    )
    .unwrap();
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    let data: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let mut record: Value = serde_json::from_str(&data).unwrap();
    record["source_sha256"] = json!("original-legacy-source");
    record["legacy_id"] = json!("legacy-vampire-1");
    record["original"] = json!({"source":"PRESERVED_ORIGINAL"});
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![record.to_string(), id],
    )
    .unwrap();
    let request = met::Request {
        power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
        day: "2026-11-14".into(),
        actor: "PRIVATE_MET_ACTOR".into(),
        reason: "PRIVATE_MET_REASON".into(),
        review_reference:
            "PRIVATE_MET_REVIEW: reviewed alias and numeric legacy meaning, not a level conversion"
                .into(),
        confirmed: true,
        confirm_alias: true,
        confirm_legacy_value: true,
        ..Default::default()
    };
    (dir, lib, id, request)
}
#[test]
fn imported_live_trait_mapping_preserves_values_xp_original_and_private_evidence() {
    let (dir, mut lib, id, request) = fixture();
    let before = lib.sheet_edit(&id, ST).unwrap();
    let q = lib
        .quote_met_reference(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    let receipt = lib.commit_met_reference(&q, ST).unwrap();
    assert_eq!(lib.commit_met_reference(&q, ST).unwrap(), receipt);
    let after = lib.sheet_edit(&id, ST).unwrap();
    let row = &after.lists["DisciplineList"]["Traits"][0];
    assert_eq!(row["Name"], "Legacy animal speech");
    assert_eq!(row["Value"], "1");
    assert_eq!(
        row["Opaque"],
        before.lists["DisciplineList"]["Traits"][0]["Opaque"]
    );
    assert_eq!(lib.xp_view(&id, ST).unwrap().unspent.as_deref(), Some("10"));
    assert!(lib.purchase_receipts(&id, ST).unwrap().is_empty());
    let doc = lib
        .print_document(
            &id,
            &Options {
                include_simple_mechanics: true,
                ..Default::default()
            },
            lib.revision().unwrap(),
            Role::Player,
        )
        .unwrap();
    assert!(
        doc.sections
            .iter()
            .any(|s| s.title == "Simple mechanics: Feral Whispers")
    );
    assert!(doc.references.iter().any(|r| r.power == "Feral Whispers"));
    let text = serde_json::to_string(&doc).unwrap();
    for secret in [
        "PRIVATE_MET_ACTOR",
        "PRIVATE_MET_REASON",
        "PRIVATE_MET_REVIEW",
        "PRIVATE_MET_TRAIT",
        met::FIELD,
    ] {
        assert!(!text.contains(secret));
    }
    for role in [ST, Role::Player] {
        assert!(
            !lib.record(&id, role)
                .unwrap()
                .fields
                .contains_key(met::FIELD)
        );
    }
    let copy = lib
        .create_build(&id, BuildKind::Test, lib.revision().unwrap(), ST)
        .unwrap();
    let copied = lib
        .print_document(
            &copy,
            &Options {
                include_simple_mechanics: true,
                ..Default::default()
            },
            lib.revision().unwrap(),
            ST,
        )
        .unwrap();
    assert!(
        copied
            .sections
            .iter()
            .any(|s| s.title == "Simple mechanics: Feral Whispers")
    );
    assert!(lib.met_reference_context(&copy, ST).is_err());
    let backup = dir.path().join("review-backup.gvlib");
    lib.backup(&backup, ST).unwrap();
    let clear = met::Request {
        id: uuid::Uuid::new_v4().to_string(),
        clear: true,
        power_id: String::new(),
        ..request
    };
    let q = lib
        .quote_met_reference(&id, &clear, lib.revision().unwrap(), ST)
        .unwrap();
    lib.commit_met_reference(&q, ST).unwrap();
    assert!(
        lib.sheet_edit(&id, ST).unwrap().lists["DisciplineList"]["Traits"][0]
            .get(met::TRAIT_FIELD)
            .is_none()
    );
    lib.restore(&backup, lib.revision().unwrap(), ST).unwrap();
    let path = lib.path.clone();
    drop(lib);
    let lib = Library::open(&path).unwrap();
    assert_eq!(lib.met_reference_context(&id, ST).unwrap().history.len(), 1);
    let raw: String = rusqlite::Connection::open(path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    let restored: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(restored["source_sha256"], "original-legacy-source");
    assert_eq!(restored["original"]["source"], "PRESERVED_ORIGINAL");
}
#[test]
fn alias_legacy_value_role_stale_tamper_and_rollback_guards() {
    let (_dir, mut lib, id, request) = fixture();
    let mut missing = request.clone();
    missing.confirm_alias = false;
    assert!(
        lib.quote_met_reference(&id, &missing, lib.revision().unwrap(), ST)
            .is_err()
    );
    missing = request.clone();
    missing.confirm_legacy_value = false;
    assert!(
        lib.quote_met_reference(&id, &missing, lib.revision().unwrap(), ST)
            .is_err()
    );
    assert!(lib.met_reference_context(&id, Role::Player).is_err());
    let q = lib
        .quote_met_reference(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(lib.commit_met_reference(&q, Role::Player).is_err());
    let mut bad = q.clone();
    bad.updated_trait["Value"] = json!("advanced");
    assert!(lib.commit_met_reference(&bad, ST).is_err());
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_met_reference BEFORE INSERT ON audit WHEN NEW.operation='met_reminder_mapping' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    assert!(lib.commit_met_reference(&q, ST).is_err());
    assert!(
        lib.met_reference_context(&id, ST)
            .unwrap()
            .history
            .is_empty()
    );
    assert_eq!(lib.revision().unwrap(), q.revision);
    conn.execute_batch("DROP TRIGGER fail_met_reference;")
        .unwrap();
    lib.create_character("Mortal", "Concurrent", lib.revision().unwrap(), ST)
        .unwrap();
    assert!(lib.commit_met_reference(&q, ST).is_err());
}
