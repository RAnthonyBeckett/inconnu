use grapevine_workbench::{
    chronicle::Profile,
    legacy_profile::{self as legacy, Field},
    model::{Result, Role},
    store::Library,
    xp::Baseline,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use uuid::Uuid;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

fn current_profile(title: &str, token: &str, details: bool, contacts: bool) -> Profile {
    Profile {
        title: title.into(),
        website: format!("https://current-{token}.invalid"),
        email: format!("current-{token}@example.invalid"),
        phone: format!("CURRENT_PHONE_{token}"),
        meeting_place: format!("CURRENT_PLACE_{token}"),
        meeting_time: format!("CURRENT_TIME_{token}"),
        description: format!("CURRENT_DESCRIPTION_{token}"),
        house_rules: format!("KEEP_HOUSE_RULES_{token}"),
        private_notes: format!("KEEP_PRIVATE_NOTES_{token}"),
        publish_details: details,
        publish_contacts: contacts,
        owbn_bylaws_enabled: false,
        allow_non_binding_sources: false,
    }
}

fn request(
    hash: &str,
    selected: Vec<Field>,
    overwrite: bool,
    publication: bool,
) -> legacy::Request {
    legacy::Request {
        source_hash: hash.into(),
        selected,
        day: "2026-11-15".into(),
        actor: "PRIVATE_SYNTHETIC_ST".into(),
        reason: "PRIVATE_SYNTHETIC_RECONCILIATION_REASON".into(),
        review_reference: "PRIVATE_SYNTHETIC_REVIEW_REFERENCE".into(),
        confirmed: true,
        confirm_overwrite: overwrite,
        confirm_publication: publication,
        ..legacy::Request::default()
    }
}

fn import_xml(
    library: &mut Library,
    dir: &Path,
    name: &str,
    xml: &str,
) -> (PathBuf, String, Vec<u8>) {
    let source = dir.join(name);
    fs::write(&source, xml.as_bytes()).unwrap();
    let bytes = fs::read(&source).unwrap();
    let preview = library.preview_import(&source, "windows-1252", ST).unwrap();
    let hash = preview.source_hash.clone();
    library
        .commit_import(&preview, library.revision().unwrap(), false, ST)
        .unwrap();
    (source, hash, bytes)
}

fn candidate(context: &legacy::Context, field: Field) -> &legacy::Candidate {
    context
        .fields
        .iter()
        .find(|candidate| candidate.field == field)
        .unwrap()
}

fn stored_import_bytes(path: &Path, hash: &str) -> Vec<u8> {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT source_bytes FROM imports WHERE hash=?1",
            [hash],
            |row| row.get(0),
        )
        .unwrap()
}

fn stored_import_report(path: &Path, hash: &str) -> String {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT report FROM imports WHERE hash=?1", [hash], |row| {
            row.get(0)
        })
        .unwrap()
}

fn set_import_report(path: &Path, hash: &str, report: &Value) {
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .execute(
                "UPDATE imports SET report=?1 WHERE hash=?2",
                params![report.to_string(), hash],
            )
            .unwrap(),
        1
    );
}

fn set_import_bytes(path: &Path, hash: &str, bytes: &[u8]) {
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .execute(
                "UPDATE imports SET source_bytes=?1 WHERE hash=?2",
                params![bytes, hash],
            )
            .unwrap(),
        1
    );
}

fn profile_data(path: &Path) -> Value {
    let encoded: String = Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT data FROM chronicle_profile WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_str(&encoded).unwrap()
}

fn set_profile_data(path: &Path, value: &Value) {
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .execute(
                "UPDATE chronicle_profile SET data=?1 WHERE singleton=1",
                [value.to_string()],
            )
            .unwrap(),
        1
    );
}

fn records_snapshot(path: &Path) -> Vec<(String, String)> {
    let connection = Connection::open(path).unwrap();
    let mut statement = connection
        .prepare("SELECT id,data FROM records ORDER BY id")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn audit_count(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
        .unwrap()
}

fn xp_snapshot(view: &grapevine_workbench::xp::XpView) -> Value {
    json!({
        "ledger": serde_json::to_value(&view.ledger).unwrap(),
        "earned": view.earned,
        "unspent": view.unspent,
        "legacy_history_count": view.legacy_history_count,
        "legacy_history": serde_json::to_value(&view.legacy_history).unwrap(),
        "legacy_earned": view.legacy_earned,
        "legacy_unspent": view.legacy_unspent
    })
}

fn insert_report(path: &Path, name: &str, report: &Value) -> String {
    let bytes = format!("synthetic retained report fixture: {name}").into_bytes();
    let hash = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Connection::open(path)
        .unwrap()
        .execute(
            "INSERT INTO imports(hash,source_name,source_bytes,report) VALUES(?1,?2,?3,?4)",
            params![&hash, name, &bytes, report.to_string()],
        )
        .unwrap();
    hash
}

fn error_code<T: std::fmt::Debug>(result: Result<T>, expected: &str) {
    let error = result.unwrap_err();
    assert_eq!(error.code, expected, "{}", error.message);
}

fn baseline(unspent: &str) -> Baseline {
    Baseline {
        day: "2026-11-01".into(),
        earned: unspent.into(),
        unspent: unspent.into(),
        actor: "Synthetic opening staff".into(),
        reason: "Synthetic opening balance".into(),
        evidence: "Synthetic fixture".into(),
    }
}

#[test]
fn synthetic_binary_game_review_maps_all_fields_and_survives_recovery() {
    let dir = TempDir::new().unwrap();
    let library_path = dir.path().join("binary.gvlib");
    let mut library = Library::create(&library_path, "Initial synthetic library").unwrap();
    let token = Uuid::new_v4().simple().to_string();
    let initial = current_profile("Initial synthetic library", &token, false, false);
    library.save_chronicle_profile(&initial, 0, ST).unwrap();
    let character_name = format!("XP snapshot fixture {token}");
    let character = library
        .create_character("Vampire", &character_name, 1, ST)
        .unwrap();
    library
        .initialize_xp(&character, &baseline("15"), 2, ST)
        .unwrap();

    let mut raw_profile = profile_data(&library_path);
    raw_profile["future_private_profile_key"] = json!({"keep": ["synthetic", 7]});
    set_profile_data(&library_path, &raw_profile);

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-game.gv3");
    let source_bytes = fs::read(&source).unwrap();
    let preview = library.preview_import(&source, "windows-1252", ST).unwrap();
    assert!(preview.conflicts.is_empty());
    let source_hash = preview.source_hash.clone();
    library
        .commit_import(&preview, library.revision().unwrap(), false, ST)
        .unwrap();
    assert_eq!(
        stored_import_bytes(&library_path, &source_hash),
        source_bytes
    );
    let report_before = stored_import_report(&library_path, &source_hash);
    let records_before = records_snapshot(&library_path);
    let xp_before = xp_snapshot(&library.xp_view(&character, ST).unwrap());

    let sources = library.legacy_profile_sources(ST).unwrap();
    let listed = sources
        .iter()
        .find(|source| source.hash == source_hash)
        .unwrap();
    assert_eq!(listed.available_fields, 7);
    assert!(listed.issue.is_none());

    let context = library.legacy_profile_context(&source_hash, ST).unwrap();
    for field in Field::ALL {
        let row = candidate(&context, field);
        assert_eq!(row.current, field.get(&initial));
        assert!(
            row.proposed.is_some(),
            "synthetic binary source omitted {field:?}"
        );
        assert_eq!(row.would_be_public, field == Field::Title);
    }

    let keep_current = legacy::Request {
        source_hash: source_hash.clone(),
        ..Default::default()
    };
    error_code(
        library.quote_legacy_profile(&keep_current, context.revision, ST),
        "LEGACY_PROFILE_REVIEW",
    );
    assert_eq!(library.chronicle_profile(ST).unwrap().profile, initial);

    let request = request(&source_hash, Field::ALL.to_vec(), true, true);
    let quote = library
        .quote_legacy_profile(&request, context.revision, ST)
        .unwrap();
    assert_eq!(quote.changes.len(), 7);
    for field in Field::ALL {
        assert_eq!(
            field.get(&quote.proposed),
            candidate(&context, field).proposed.as_deref().unwrap()
        );
    }
    assert!(!quote.proposed.publish_details);
    assert!(!quote.proposed.publish_contacts);
    assert_eq!(quote.proposed.house_rules, initial.house_rules);
    assert_eq!(quote.proposed.private_notes, initial.private_notes);

    let receipt = library.commit_legacy_profile(&quote, ST).unwrap();
    let saved = library.chronicle_profile(ST).unwrap();
    assert_eq!(saved.revision, context.revision + 1);
    assert_eq!(saved.profile, quote.proposed);
    assert_eq!(library.title, saved.profile.title);
    assert_eq!(
        library
            .legacy_profile_context(&source_hash, ST)
            .unwrap()
            .history,
        vec![receipt]
    );
    assert_eq!(
        profile_data(&library_path)["future_private_profile_key"],
        json!({"keep":["synthetic",7]})
    );
    assert!(
        !serde_json::to_string(&library.public_chronicle_profile().unwrap())
            .unwrap()
            .contains(legacy::FIELD)
    );
    assert_eq!(records_snapshot(&library_path), records_before);
    assert_eq!(
        xp_snapshot(&library.xp_view(&character, ST).unwrap()),
        xp_before
    );
    assert_eq!(
        stored_import_bytes(&library_path, &source_hash),
        source_bytes
    );
    assert_eq!(
        stored_import_report(&library_path, &source_hash),
        report_before
    );
    assert!(
        library
            .legacy_profile_context(&source_hash, PLAYER)
            .is_err()
    );

    let mut ordinary_edit = saved.profile.clone();
    ordinary_edit.description.push_str(" preserved later edit");
    let next_revision = library
        .save_chronicle_profile(&ordinary_edit, saved.revision, ST)
        .unwrap();
    assert_eq!(
        library
            .legacy_profile_context(&source_hash, ST)
            .unwrap()
            .history
            .len(),
        1
    );
    let after_edit = profile_data(&library_path);
    assert_eq!(
        after_edit["future_private_profile_key"],
        json!({"keep":["synthetic",7]})
    );
    assert_eq!(after_edit[legacy::FIELD].as_array().unwrap().len(), 1);

    let backup = dir.path().join("binary-review-backup.gvlib");
    library.backup(&backup, ST).unwrap();
    let mut later_edit = library.chronicle_profile(ST).unwrap().profile;
    later_edit.meeting_time = "AFTER_BACKUP_ONLY".into();
    let later_revision = library
        .save_chronicle_profile(&later_edit, next_revision, ST)
        .unwrap();
    library.restore(&backup, later_revision, ST).unwrap();
    drop(library);

    let restored = Library::open(&library_path).unwrap();
    let restored_snapshot = restored.chronicle_profile(ST).unwrap();
    assert_eq!(
        restored_snapshot.profile.description,
        ordinary_edit.description
    );
    assert_ne!(restored_snapshot.profile.meeting_time, "AFTER_BACKUP_ONLY");
    assert_eq!(restored.title, restored_snapshot.profile.title);
    assert_eq!(
        restored
            .legacy_profile_context(&source_hash, ST)
            .unwrap()
            .history
            .len(),
        1
    );
    assert_eq!(
        profile_data(&library_path)["future_private_profile_key"],
        json!({"keep":["synthetic",7]})
    );
    assert_eq!(
        stored_import_bytes(&library_path, &source_hash),
        source_bytes
    );
    assert_eq!(
        stored_import_report(&library_path, &source_hash),
        report_before
    );
    assert_eq!(records_snapshot(&library_path), records_before);
    assert_eq!(
        xp_snapshot(&restored.xp_view(&character, ST).unwrap()),
        xp_before
    );
}

#[test]
fn xml_source_redacts_default_and_custom_st_text_and_requires_acknowledgements() {
    let dir = TempDir::new().unwrap();
    let library_path = dir.path().join("xml.gvlib");
    let mut library = Library::create(&library_path, "Current XML Chronicle").unwrap();
    let profile = current_profile("Current XML Chronicle", "xml", true, true);
    library.save_chronicle_profile(&profile, 0, ST).unwrap();

    let xml = r#"<grapevine version="3" chronicle="XML Legacy Chronicle" website="" email="new@example.invalid" phone="555-0199" usualtime="Friday evening" stcommentstart="__STAFF_START__" stcommentend="__STAFF_END__"><usualplace>Club [ST]default secret[/ST] __STAFF_START__custom secret__STAFF_END__ public text</usualplace><description></description></grapevine>"#;
    let (source, source_hash, source_bytes) =
        import_xml(&mut library, dir.path(), "legacy-settings.gex", xml);
    let context = library.legacy_profile_context(&source_hash, ST).unwrap();
    assert_eq!(context.fields.len(), 7);
    assert_eq!(
        candidate(&context, Field::Website).proposed.as_deref(),
        Some("")
    );
    assert_eq!(
        candidate(&context, Field::Description).proposed.as_deref(),
        Some("")
    );
    let place = candidate(&context, Field::MeetingPlace);
    assert!(place.private_text_removed);
    let safe_place = place.proposed.as_deref().unwrap();
    assert!(safe_place.contains("Club"));
    assert!(safe_place.contains("public text"));
    assert!(!safe_place.contains("default secret"));
    assert!(!safe_place.contains("custom secret"));
    assert!(candidate(&context, Field::Website).would_be_public);
    assert!(candidate(&context, Field::Email).would_be_public);

    let selected = vec![
        Field::Website,
        Field::Description,
        Field::Email,
        Field::MeetingPlace,
    ];
    let mut review = request(&source_hash, selected, false, false);
    error_code(
        library.quote_legacy_profile(&review, context.revision, ST),
        "LEGACY_PROFILE_OVERWRITE",
    );
    review.confirm_overwrite = true;
    error_code(
        library.quote_legacy_profile(&review, context.revision, ST),
        "LEGACY_PROFILE_PUBLICATION",
    );
    review.confirm_publication = true;
    let quote = library
        .quote_legacy_profile(&review, context.revision, ST)
        .unwrap();
    assert_eq!(quote.changes.len(), 4);
    assert_eq!(quote.proposed.website, "");
    assert_eq!(quote.proposed.description, "");
    assert_eq!(quote.proposed.email, "new@example.invalid");
    assert_eq!(quote.proposed.meeting_place, safe_place);
    assert!(quote.changes.iter().all(|change| change.would_be_public));
    library.commit_legacy_profile(&quote, ST).unwrap();

    let saved = library.chronicle_profile(ST).unwrap().profile;
    assert!(saved.publish_details);
    assert!(saved.publish_contacts);
    let public = library.public_chronicle_profile().unwrap();
    assert_eq!(public.profile.website, "");
    assert_eq!(public.profile.description, "");
    assert_eq!(public.profile.email, "new@example.invalid");
    assert_eq!(public.profile.meeting_place, safe_place);
    assert!(
        !serde_json::to_string(&public)
            .unwrap()
            .contains("custom secret")
    );
    let stored_report = stored_import_report(&library.path, &source_hash);
    assert!(stored_report.contains("default secret"));
    assert!(stored_report.contains("custom secret"));
    assert_eq!(
        stored_import_bytes(&library.path, &source_hash),
        source_bytes
    );
    assert_eq!(fs::read(source).unwrap(), source_bytes);
}

#[test]
fn unsupported_ambiguous_and_malformed_metadata_stays_visible_and_unselected() {
    let dir = TempDir::new().unwrap();
    let library_path = dir.path().join("unsupported.gvlib");
    let mut library = Library::create(&library_path, "Unsupported fixture").unwrap();

    let xml = r#"<grapevine version="3" chronicle="Nested Legacy Chronicle" website="https://nested.invalid" email="synthetic@example.invalid" phone="555-0188" usualtime="Tuesday" future_game_option="retained"><usualplace><room>Unsupported nested text</room></usualplace><description>Supported description</description></grapevine>"#;
    let (_, nested_hash, _) = import_xml(&mut library, dir.path(), "nested.gex", xml);
    let context = library.legacy_profile_context(&nested_hash, ST).unwrap();
    assert_eq!(
        candidate(&context, Field::MeetingPlace).proposed,
        None,
        "nested nodes must not be flattened into shareable profile text"
    );
    let nested_request = request(&nested_hash, vec![Field::MeetingPlace], true, true);
    error_code(
        library.quote_legacy_profile(&nested_request, context.revision, ST),
        "LEGACY_PROFILE_FIELD",
    );
    let nested_report: Value =
        serde_json::from_str(&stored_import_report(&library_path, &nested_hash)).unwrap();
    assert!(
        nested_report["metadata"]["XmlRootAttributes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|attribute| attribute["name"] == "future_game_option")
    );
    assert_eq!(
        nested_report["metadata"]["usualplace"]["children"][0]["tag"],
        "room"
    );
    assert!(library.legacy_profile_sources(PLAYER).is_err());

    let mixed_report = json!({
        "metadata": {
            "Game": {"ChronicleTitle":"Binary candidate"},
            "XmlRootAttributes": [{"name":"chronicle","namespace":null,"value":"XML candidate"}]
        }
    });
    let mixed_hash = insert_report(&library_path, "mixed-synthetic-report", &mixed_report);

    let duplicate_report = json!({
        "metadata": {
            "XmlRootAttributes": [
                {"name":"chronicle","namespace":null,"value":"Duplicate A"},
                {"name":"chronicle","namespace":null,"value":"Duplicate B"}
            ]
        }
    });
    let duplicate_hash = insert_report(
        &library_path,
        "duplicate-attribute-synthetic-report",
        &duplicate_report,
    );

    let malformed_report = json!({
        "metadata": {"Game": {"ChronicleTitle": 17}}
    });
    let malformed_hash = insert_report(
        &library_path,
        "malformed-field-synthetic-report",
        &malformed_report,
    );

    let sources = library.legacy_profile_sources(ST).unwrap();
    for hash in [&mixed_hash, &duplicate_hash] {
        let source = sources.iter().find(|source| &source.hash == hash).unwrap();
        assert_eq!(source.available_fields, 0);
        assert!(source.issue.is_some());
        error_code(
            library.legacy_profile_context(hash, ST),
            "LEGACY_PROFILE_AMBIGUOUS",
        );
    }
    let malformed = sources
        .iter()
        .find(|source| source.hash == malformed_hash)
        .unwrap();
    assert_eq!(malformed.available_fields, 0);
    assert!(malformed.issue.is_none());
    let malformed_context = library.legacy_profile_context(&malformed_hash, ST).unwrap();
    assert_eq!(candidate(&malformed_context, Field::Title).proposed, None);
    error_code(
        library.quote_legacy_profile(
            &request(&malformed_hash, vec![Field::Title], true, true),
            malformed_context.revision,
            ST,
        ),
        "LEGACY_PROFILE_FIELD",
    );

    let mut duplicate_selection = request(
        &nested_hash,
        vec![Field::Description, Field::Description],
        true,
        true,
    );
    duplicate_selection.source_hash = nested_hash.clone();
    error_code(
        library.quote_legacy_profile(&duplicate_selection, context.revision, ST),
        "LEGACY_PROFILE_FIELD",
    );
    assert!(
        library
            .legacy_profile_context(&nested_hash, PLAYER)
            .is_err()
    );

    let mut unknown_request = serde_json::to_value(legacy::Request::default()).unwrap();
    unknown_request["unreviewed_field"] = json!("must reject");
    assert!(serde_json::from_value::<legacy::Request>(unknown_request).is_err());

    let saved_profile = current_profile("Unsupported fixture", "history", false, false);
    let revision = library.revision().unwrap();
    library
        .save_chronicle_profile(&saved_profile, revision, ST)
        .unwrap();
    let mut malformed_history = profile_data(&library_path);
    malformed_history[legacy::FIELD] = json!("PRIVATE_MALFORMED_HISTORY_SENTINEL");
    set_profile_data(&library_path, &malformed_history);
    let public_error = library.public_chronicle_profile().unwrap_err();
    assert_eq!(public_error.code, "LEGACY_PROFILE_HISTORY");
    assert!(
        !public_error
            .message
            .contains("PRIVATE_MALFORMED_HISTORY_SENTINEL")
    );
}

#[test]
fn quotes_bind_source_profile_revision_and_request_identity() {
    let dir = TempDir::new().unwrap();
    let library_path = dir.path().join("bound.gvlib");
    let mut library = Library::create(&library_path, "Current Chronicle").unwrap();
    let initial = current_profile("Current Chronicle", "bound", false, false);
    library.save_chronicle_profile(&initial, 0, ST).unwrap();
    let xml = r#"<grapevine version="3" chronicle="Source Chronicle"><usualplace>Source room</usualplace></grapevine>"#;
    let (_, source_hash, _) = import_xml(&mut library, dir.path(), "bound.gex", xml);
    let context = library.legacy_profile_context(&source_hash, ST).unwrap();
    let initial_request = request(&source_hash, vec![Field::Title], true, true);
    let quote = library
        .quote_legacy_profile(&initial_request, context.revision, ST)
        .unwrap();

    let mut other = Library::create(&dir.path().join("other.gvlib"), "Other library").unwrap();
    error_code(
        other.commit_legacy_profile(&quote, ST),
        "LEGACY_PROFILE_LIBRARY",
    );

    let mut quote_tamper = quote.clone();
    quote_tamper.proposed.website = "FORGED_UNSELECTED_FIELD".into();
    error_code(
        library.commit_legacy_profile(&quote_tamper, ST),
        "STALE_REVISION",
    );

    let original_report: Value =
        serde_json::from_str(&stored_import_report(&library_path, &source_hash)).unwrap();
    let mut changed_report = original_report.clone();
    let attributes = changed_report["metadata"]["XmlRootAttributes"]
        .as_array_mut()
        .unwrap();
    attributes
        .iter_mut()
        .find(|attribute| attribute["name"] == "chronicle")
        .unwrap()["value"] = json!("Tampered source metadata");
    set_import_report(&library_path, &source_hash, &changed_report);
    error_code(library.commit_legacy_profile(&quote, ST), "STALE_REVISION");
    set_import_report(&library_path, &source_hash, &original_report);

    let original_profile_data = profile_data(&library_path);
    let mut changed_profile_data = original_profile_data.clone();
    changed_profile_data["outside_revision_change"] = json!("synthetic tamper");
    set_profile_data(&library_path, &changed_profile_data);
    error_code(library.commit_legacy_profile(&quote, ST), "STALE_REVISION");
    set_profile_data(&library_path, &original_profile_data);

    let mut profile = library.chronicle_profile(ST).unwrap().profile;
    profile.meeting_time.push_str(" revision bump");
    let bumped_revision = library
        .save_chronicle_profile(&profile, context.revision, ST)
        .unwrap();
    error_code(library.commit_legacy_profile(&quote, ST), "STALE_REVISION");

    let current = library.legacy_profile_context(&source_hash, ST).unwrap();
    let fresh_request = request(&source_hash, vec![Field::Title], true, true);
    let fresh_quote = library
        .quote_legacy_profile(&fresh_request, current.revision, ST)
        .unwrap();
    let before_audit = audit_count(&library_path);
    let receipt = library.commit_legacy_profile(&fresh_quote, ST).unwrap();
    let revision_after_commit = library.revision().unwrap();
    let audit_after_commit = audit_count(&library_path);
    assert_eq!(revision_after_commit, bumped_revision + 1);
    assert_eq!(audit_after_commit, before_audit + 1);
    assert_eq!(
        library.commit_legacy_profile(&fresh_quote, ST).unwrap(),
        receipt
    );
    assert_eq!(library.revision().unwrap(), revision_after_commit);
    assert_eq!(audit_count(&library_path), audit_after_commit);

    let mut conflicting_retry = fresh_quote;
    conflicting_retry.request.reason = "Different request under same ID".into();
    error_code(
        library.commit_legacy_profile(&conflicting_retry, ST),
        "LEGACY_PROFILE_ID",
    );
    assert_eq!(library.revision().unwrap(), revision_after_commit);
}

#[test]
fn audit_failure_rolls_back_title_profile_history_and_revision() {
    let dir = TempDir::new().unwrap();
    let library_path = dir.path().join("rollback.gvlib");
    let mut library = Library::create(&library_path, "Current rollback title").unwrap();
    let initial = current_profile("Current rollback title", "rollback", false, false);
    library.save_chronicle_profile(&initial, 0, ST).unwrap();
    let xml = r#"<grapevine version="3" chronicle="Applied only after audit"><description>Reviewed text</description></grapevine>"#;
    let (_, source_hash, _) = import_xml(&mut library, dir.path(), "rollback.gex", xml);
    let context = library.legacy_profile_context(&source_hash, ST).unwrap();
    let request = request(&source_hash, vec![Field::Title], true, true);
    let quote = library
        .quote_legacy_profile(&request, context.revision, ST)
        .unwrap();
    let old_profile = library.chronicle_profile(ST).unwrap().profile;
    let old_revision = library.revision().unwrap();
    let old_title = library.title.clone();
    let old_audit_count = audit_count(&library_path);

    Connection::open(&library_path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_legacy_review_audit \
             BEFORE INSERT ON audit \
             WHEN NEW.operation='legacy_chronicle_reconcile' \
             BEGIN SELECT RAISE(ABORT,'synthetic audit failure'); END;",
        )
        .unwrap();
    assert!(library.commit_legacy_profile(&quote, ST).is_err());
    assert_eq!(library.revision().unwrap(), old_revision);
    assert_eq!(library.title, old_title);
    assert_eq!(library.chronicle_profile(ST).unwrap().profile, old_profile);
    assert!(
        library
            .legacy_profile_context(&source_hash, ST)
            .unwrap()
            .history
            .is_empty()
    );
    assert_eq!(audit_count(&library_path), old_audit_count);

    Connection::open(&library_path)
        .unwrap()
        .execute_batch("DROP TRIGGER reject_legacy_review_audit;")
        .unwrap();
    let receipt = library.commit_legacy_profile(&quote, ST).unwrap();
    assert_eq!(receipt.committed_revision, old_revision + 1);
    assert_eq!(library.title, quote.proposed.title);
    assert_eq!(
        library.chronicle_profile(ST).unwrap().profile,
        quote.proposed
    );
    assert_eq!(
        library
            .legacy_profile_context(&source_hash, ST)
            .unwrap()
            .history,
        vec![receipt]
    );
    assert_eq!(audit_count(&library_path), old_audit_count + 1);
}

#[test]
fn stored_source_bytes_must_still_match_the_import_hash_at_commit() {
    let dir = TempDir::new().unwrap();
    let library_path = dir.path().join("source-bytes.gvlib");
    let mut library = Library::create(&library_path, "Source byte check").unwrap();
    let xml = r#"<grapevine version="3" chronicle="Pending source review"></grapevine>"#;
    let (_, source_hash, original_bytes) =
        import_xml(&mut library, dir.path(), "source-bytes.gex", xml);
    let context = library.legacy_profile_context(&source_hash, ST).unwrap();
    let request = request(&source_hash, vec![Field::Title], true, true);
    let quote = library
        .quote_legacy_profile(&request, context.revision, ST)
        .unwrap();

    let mut changed_bytes = stored_import_bytes(&library_path, &source_hash);
    assert_eq!(changed_bytes, original_bytes);
    changed_bytes[0] ^= 1;
    set_import_bytes(&library_path, &source_hash, &changed_bytes);
    error_code(
        library.commit_legacy_profile(&quote, ST),
        "LEGACY_PROFILE_SOURCE_CHANGED",
    );
    set_import_bytes(&library_path, &source_hash, &original_bytes);
    let mut request_tamper = quote;
    request_tamper.request.reason = "FORGED_REASON_AFTER_REVIEW".into();
    error_code(
        library.commit_legacy_profile(&request_tamper, ST),
        "STALE_REVISION",
    );
}
