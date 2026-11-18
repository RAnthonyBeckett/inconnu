use grapevine_workbench::{
    chronicle::Profile,
    game_sessions::{Attendance, SessionRequest},
    legacy_calendar::{self as calendar, Choice, Request},
    model::{Result, Role},
    printing,
    store::Library,
    xp::Baseline,
};
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;
const PRIVATE_PLACE: &str = "PRIVATE_CALENDAR_PLACE_SENTINEL";
const PRIVATE_NOTES: &str = "PRIVATE_CALENDAR_NOTES_SENTINEL";
const PRIVATE_ACTOR: &str = "PRIVATE_CALENDAR_REVIEW_ACTOR_SENTINEL";
const PRIVATE_REASON: &str = "PRIVATE_CALENDAR_REVIEW_REASON_SENTINEL";
const PRIVATE_REFERENCE: &str = "PRIVATE_CALENDAR_REVIEW_REFERENCE_SENTINEL";
const PRIVATE_PROFILE_NOTE: &str = "PRIVATE_CHRONICLE_NOTE_SENTINEL";

const XML: &str = r#"<grapevine version="3"><calendar>
  <game date="2026-10-31" time="19:30"><place>PRIVATE_CALENDAR_PLACE_SENTINEL</place><notes>PRIVATE_CALENDAR_NOTES_SENTINEL</notes></game>
  <game date="11/15/2026" time="20:00"><place>Synthetic hall</place><notes>First same-day entry</notes></game>
  <game date="11/15/2026" time="21:00"><place>Synthetic hall</place><notes>Second same-day entry</notes></game>
  <game date="01/02/2026" time="19:00"><place>Ambiguous date room</place><notes>Requires manual date</notes></game>
  <game date="11/15/26" time="19:00"><place>Unsupported date room</place><notes>Requires manual date</notes></game>
</calendar></grapevine>"#;

struct Fixture {
    dir: TempDir,
    library: Library,
    source_path: PathBuf,
    source_hash: String,
    source_bytes: Vec<u8>,
    character_id: String,
}

fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let library_path = dir.path().join("legacy-calendar.gvlib");
    let mut library = Library::create(&library_path, "Synthetic current Chronicle").unwrap();
    let profile = Profile {
        title: "Synthetic current Chronicle".into(),
        website: "https://current.invalid".into(),
        email: "current@example.invalid".into(),
        phone: "555-0101".into(),
        meeting_place: "Current hall".into(),
        meeting_time: "Friday evening".into(),
        description: "Current description".into(),
        house_rules: "KEEP_CURRENT_HOUSE_RULES_SENTINEL".into(),
        private_notes: PRIVATE_PROFILE_NOTE.into(),
        publish_details: true,
        publish_contacts: false,
        owbn_bylaws_enabled: false,
        allow_non_binding_sources: false,
        house_rules_pack: None,
    };
    library.save_chronicle_profile(&profile, 0, ST).unwrap();
    let character_id = library
        .create_character("Vampire", "Synthetic calendar XP record", 1, ST)
        .unwrap();
    let baseline = Baseline {
        day: "2026-11-01".into(),
        earned: "8".into(),
        unspent: "8".into(),
        actor: "Synthetic opening staff".into(),
        reason: "Synthetic opening balance".into(),
        evidence: "Synthetic calendar test fixture".into(),
    };
    library
        .initialize_xp(&character_id, &baseline, library.revision().unwrap(), ST)
        .unwrap();

    let source_path = dir.path().join("synthetic-calendar.gex");
    fs::write(&source_path, XML.as_bytes()).unwrap();
    let source_bytes = fs::read(&source_path).unwrap();
    let preview = library
        .preview_import(&source_path, "windows-1252", ST)
        .unwrap();
    let source_hash = preview.source_hash.clone();
    library
        .commit_import(&preview, library.revision().unwrap(), false, ST)
        .unwrap();

    let mut raw = profile_data(&library_path);
    raw["future_private_calendar_test_key"] = json!({"retained": ["synthetic", 17]});
    set_profile_data(&library_path, &raw);
    Fixture {
        dir,
        library,
        source_path,
        source_hash,
        source_bytes,
        character_id,
    }
}

fn review_request(source_hash: &str, choices: Vec<Choice>) -> Request {
    Request {
        source_hash: source_hash.into(),
        choices,
        review_day: "2026-11-15".into(),
        actor: PRIVATE_ACTOR.into(),
        reason: PRIVATE_REASON.into(),
        review_reference: PRIVATE_REFERENCE.into(),
        confirmed: true,
        ..Request::default()
    }
}

fn choice(entry_index: usize, day: &str) -> Choice {
    Choice {
        entry_index,
        day: day.into(),
        title: format!("Reviewed synthetic session {entry_index}"),
    }
}

fn recovery_quote(f: &Fixture) -> calendar::Quote {
    let context = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap();
    let request = review_request(
        &f.source_hash,
        vec![choice(4, "2026-12-01"), choice(3, "2026-12-10")],
    );
    f.library
        .quote_legacy_calendar(&request, context.revision, ST)
        .unwrap()
}

fn profile_data(path: &std::path::Path) -> Value {
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

fn set_profile_data(path: &std::path::Path, value: &Value) {
    assert_eq!(
        Connection::open(path)
            .unwrap()
            .execute(
                "UPDATE chronicle_profile SET data=?1 WHERE singleton=1",
                [value.to_string()],
            )
            .unwrap(),
        1
    );
}

fn stored_import_bytes(path: &std::path::Path, hash: &str) -> Vec<u8> {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT source_bytes FROM imports WHERE hash=?1",
            [hash],
            |row| row.get(0),
        )
        .unwrap()
}

fn set_import_bytes(path: &std::path::Path, hash: &str, bytes: &[u8]) {
    assert_eq!(
        Connection::open(path)
            .unwrap()
            .execute(
                "UPDATE imports SET source_bytes=?1 WHERE hash=?2",
                params![bytes, hash],
            )
            .unwrap(),
        1
    );
}

fn stored_import_report(path: &std::path::Path, hash: &str) -> String {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT report FROM imports WHERE hash=?1", [hash], |row| {
            row.get(0)
        })
        .unwrap()
}

fn set_import_report(path: &std::path::Path, hash: &str, report: &Value) {
    assert_eq!(
        Connection::open(path)
            .unwrap()
            .execute(
                "UPDATE imports SET report=?1 WHERE hash=?2",
                params![report.to_string(), hash],
            )
            .unwrap(),
        1
    );
}

fn records_snapshot(path: &std::path::Path) -> Vec<(String, String)> {
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

fn xp_snapshot(library: &Library, character_id: &str) -> Value {
    let view = library.xp_view(character_id, ST).unwrap();
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

fn audit_count(path: &std::path::Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
        .unwrap()
}

fn error_code<T: std::fmt::Debug>(result: Result<T>, expected: &str) {
    let error = result.unwrap_err();
    assert_eq!(error.code, expected, "{}", error.message);
}

#[test]
fn imported_xml_calendar_preserves_private_entries_and_starts_unselected() {
    let mut f = fixture();
    let context = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap();
    assert_eq!(context.entries.len(), 5);
    assert_eq!(context.entries[0].raw_date, "2026-10-31");
    assert_eq!(
        context.entries[0].proposed_day.as_deref(),
        Some("2026-10-31")
    );
    assert_eq!(context.entries[0].time, "19:30");
    assert_eq!(context.entries[0].place, PRIVATE_PLACE);
    assert_eq!(context.entries[0].notes, PRIVATE_NOTES);
    assert_eq!(
        context.entries[1].proposed_day.as_deref(),
        Some("2026-11-15")
    );
    assert_eq!(
        context.entries[2].proposed_day.as_deref(),
        Some("2026-11-15")
    );
    assert_ne!(context.entries[1].index, context.entries[2].index);
    assert_eq!(context.entries[3].proposed_day, None);
    assert!(context.entries[3].issue.is_some());
    assert_eq!(context.entries[4].proposed_day, None);
    assert!(context.entries[4].issue.is_some());

    let source = f
        .library
        .legacy_calendar_sources(ST)
        .unwrap()
        .into_iter()
        .find(|source| source.hash == f.source_hash)
        .unwrap();
    assert_eq!(source.entries, 5);
    assert!(source.issue.is_none());

    let default = Request::default();
    assert!(default.choices.is_empty());
    assert!(!default.confirmed && !default.confirm_past && !default.confirm_overlap);
    assert!(f.library.legacy_calendar_sources(PLAYER).is_err());
    assert!(
        f.library
            .legacy_calendar_context(&f.source_hash, PLAYER)
            .is_err()
    );
    let player_quote = f
        .library
        .quote_legacy_calendar(
            &review_request(&f.source_hash, vec![choice(4, "2026-12-01")]),
            context.revision,
            ST,
        )
        .unwrap();
    assert!(
        f.library
            .commit_legacy_calendar(&player_quote, PLAYER)
            .is_err()
    );

    assert_eq!(fs::read(&f.source_path).unwrap(), f.source_bytes);
    assert_eq!(
        stored_import_bytes(&f.library.path, &f.source_hash),
        f.source_bytes
    );
    let report = stored_import_report(&f.library.path, &f.source_hash);
    assert!(report.contains(PRIVATE_PLACE) && report.contains(PRIVATE_NOTES));
}

#[derive(Deserialize)]
struct DateProbe {
    ole_days: i64,
    iso: String,
}

#[test]
fn binary_oa_dates_match_known_oracle_and_malformed_shapes_stay_unusable() {
    let probes: Vec<DateProbe> =
        serde_json::from_str(include_str!("fixtures/legacy-calendar-oadate-oracle.json")).unwrap();
    let mut rows = probes
        .iter()
        .map(|probe| {
            json!({
                "GameDate": {"ole_automation_days": probe.ole_days},
                "Time": "synthetic time",
                "Place": "synthetic place",
                "Notes": "private synthetic source notes"
            })
        })
        .collect::<Vec<_>>();
    rows.push(json!({
        "GameDate": {"ole_automation_days": 46341.5},
        "Time": "fractional source time"
    }));
    rows.push(json!({
        "GameDate": {"ole_automation_days": 2958466},
        "Time": "out of range"
    }));
    let (entries, _) = calendar::candidates(&json!({
        "metadata": {"Calendar": {"Entries": rows}}
    }))
    .unwrap();
    assert_eq!(entries.len(), probes.len() + 2);
    for (entry, probe) in entries.iter().zip(&probes) {
        assert_eq!(entry.proposed_day.as_deref(), Some(probe.iso.as_str()));
    }
    assert_eq!(entries[probes.len()].proposed_day, None);
    assert_eq!(entries[probes.len() + 1].proposed_day, None);

    let slash_report = json!({"metadata": {"Calendar": {"Entries": [
        {"GameDate": "03/15/2026"},
        {"GameDate": "01/02/2026"},
        {"GameDate": "11/15/26"}
    ]}}});
    let (slash_entries, _) = calendar::candidates(&slash_report).unwrap();
    assert_eq!(slash_entries[0].proposed_day.as_deref(), Some("2026-03-15"));
    assert_eq!(slash_entries[1].proposed_day, None);
    assert_eq!(slash_entries[2].proposed_day, None);

    let f = fixture();
    let report: Value =
        serde_json::from_str(&stored_import_report(&f.library.path, &f.source_hash)).unwrap();
    let mut namespaced = report.clone();
    namespaced["metadata"]["calendar"]["namespace"] = json!("urn:private-calendar");
    assert!(calendar::candidates(&namespaced).is_err());

    let mut duplicate_attribute = report.clone();
    let game_child = duplicate_attribute["metadata"]["calendar"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .position(|child| child["tag"] == "game")
        .unwrap();
    duplicate_attribute["metadata"]["calendar"]["children"][game_child]["attributes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"date","namespace":null,"value":"2026-12-01"}));
    assert!(calendar::candidates(&duplicate_attribute).is_err());

    let mut duplicate_calendar = report.clone();
    duplicate_calendar["raw_xml"] =
        json!("<grapevine version=\"3\"><calendar/><calendar/></grapevine>");
    assert!(calendar::candidates(&duplicate_calendar).is_err());

    let mut mixed = report;
    mixed["metadata"]["Calendar"] = json!({"Entries": []});
    assert!(calendar::candidates(&mixed).is_err());
}

#[test]
fn reviewed_application_requires_past_and_overlap_acknowledgements_and_preserves_state() {
    let mut f = fixture();
    let existing = SessionRequest {
        id: Uuid::new_v4().to_string(),
        title: "Existing same-day synthetic plan".into(),
        day: "2026-11-15".into(),
        actor: "Synthetic staff".into(),
        reason: "Overlap confirmation fixture".into(),
        evidence: "Synthetic schedule fixture".into(),
        attendance: vec![],
    };
    f.library
        .save_scheduled_session(&existing, f.library.revision().unwrap(), ST)
        .unwrap();
    let context = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap();
    let choices = vec![
        choice(0, "2026-10-31"),
        choice(1, "2026-11-15"),
        choice(2, "2026-11-15"),
        choice(3, "2026-11-16"),
    ];
    let mut request = review_request(&f.source_hash, choices);
    error_code(
        f.library
            .quote_legacy_calendar(&request, context.revision, ST),
        "LEGACY_CALENDAR_PAST",
    );
    request.confirm_past = true;
    error_code(
        f.library
            .quote_legacy_calendar(&request, context.revision, ST),
        "LEGACY_CALENDAR_OVERLAP",
    );
    request.confirm_overlap = true;
    let quote = f
        .library
        .quote_legacy_calendar(&request, context.revision, ST)
        .unwrap();
    assert_eq!(quote.plans.len(), 4);
    assert_ne!(quote.plans[1].request.id, quote.plans[2].request.id);
    assert_eq!(quote.plans[1].request.day, quote.plans[2].request.day);
    assert!(
        quote
            .plans
            .iter()
            .all(|plan| plan.request.attendance.is_empty())
    );
    assert!(quote.plans.iter().all(|plan| {
        plan.request.evidence.contains(&f.source_hash)
            && plan.request.evidence.contains(PRIVATE_REFERENCE)
            && !plan.request.evidence.contains(PRIVATE_NOTES)
            && !plan.request.evidence.contains(PRIVATE_PLACE)
    }));

    let records_before = records_snapshot(&f.library.path);
    let xp_before = xp_snapshot(&f.library, &f.character_id);
    let raw_before = profile_data(&f.library.path);
    let profile_before = f.library.chronicle_profile(ST).unwrap().profile;
    let source_report_before = stored_import_report(&f.library.path, &f.source_hash);
    let revision_before = f.library.revision().unwrap();
    let audit_before = audit_count(&f.library.path);
    let receipt = f.library.commit_legacy_calendar(&quote, ST).unwrap();
    let origin = f
        .library
        .legacy_calendar_origin(&receipt.plans[0].request.id, ST)
        .unwrap()
        .unwrap();
    assert_eq!(origin.entry.index, receipt.plans[0].entry_index);
    assert_eq!(origin.entry.place, PRIVATE_PLACE);
    assert_eq!(origin.entry.notes, PRIVATE_NOTES);
    assert!(
        f.library
            .legacy_calendar_origin(&receipt.plans[0].request.id, PLAYER)
            .is_err()
    );
    let mut bad_origin_bytes = f.source_bytes.clone();
    bad_origin_bytes.push(b' ');
    set_import_bytes(&f.library.path, &f.source_hash, &bad_origin_bytes);
    error_code(
        f.library
            .legacy_calendar_origin(&receipt.plans[0].request.id, ST),
        "LEGACY_CALENDAR_SOURCE_CHANGED",
    );
    set_import_bytes(&f.library.path, &f.source_hash, &f.source_bytes);
    assert_eq!(receipt.committed_revision, revision_before + 1);
    assert_eq!(f.library.revision().unwrap(), revision_before + 1);
    assert_eq!(
        audit_count(&f.library.path),
        audit_before + quote.plans.len() as i64 + 1
    );
    assert_eq!(f.library.title, profile_before.title);
    assert_eq!(
        f.library
            .legacy_calendar_context(&f.source_hash, ST)
            .unwrap()
            .history,
        vec![receipt]
    );

    let scheduled = f.library.scheduled_sessions(ST).unwrap();
    let imported = scheduled
        .iter()
        .filter(|session| {
            quote
                .plans
                .iter()
                .any(|plan| plan.request.id == session.request.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(imported.len(), 4);
    assert!(imported.iter().all(|session| !session.cancelled));
    assert!(
        imported
            .iter()
            .all(|session| session.request.attendance.is_empty())
    );
    assert_eq!(records_snapshot(&f.library.path), records_before);
    assert_eq!(xp_snapshot(&f.library, &f.character_id), xp_before);
    assert_eq!(
        f.library.chronicle_profile(ST).unwrap().profile,
        profile_before
    );
    let raw_after = profile_data(&f.library.path);
    assert_eq!(
        raw_after["future_private_calendar_test_key"],
        json!({"retained":["synthetic",17]})
    );
    assert_eq!(raw_after["publish_details"], raw_before["publish_details"]);
    assert_eq!(
        raw_after["publish_contacts"],
        raw_before["publish_contacts"]
    );
    assert_eq!(raw_after["house_rules"], raw_before["house_rules"]);
    assert_eq!(raw_after["private_notes"], raw_before["private_notes"]);
    assert_eq!(
        stored_import_bytes(&f.library.path, &f.source_hash),
        f.source_bytes
    );
    assert_eq!(fs::read(&f.source_path).unwrap(), f.source_bytes);
    assert_eq!(
        stored_import_report(&f.library.path, &f.source_hash),
        source_report_before
    );

    let public = serde_json::to_string(&f.library.public_chronicle_profile().unwrap()).unwrap();
    for private in [
        calendar::FIELD,
        PRIVATE_ACTOR,
        PRIVATE_REASON,
        PRIVATE_REFERENCE,
        PRIVATE_NOTES,
        PRIVATE_PROFILE_NOTE,
    ] {
        assert!(
            !public.contains(private),
            "Player profile exposed {private}"
        );
    }
    let print = f
        .library
        .print_document(
            &f.character_id,
            &printing::Options::default(),
            f.library.revision().unwrap(),
            PLAYER,
        )
        .unwrap();
    let html = printing::html(&print).unwrap();
    for private in [
        calendar::FIELD,
        PRIVATE_ACTOR,
        PRIVATE_REASON,
        PRIVATE_REFERENCE,
        PRIVATE_NOTES,
        PRIVATE_PROFILE_NOTE,
    ] {
        assert!(
            !html.contains(private),
            "Player print HTML exposed {private}"
        );
    }
}

#[test]
fn quote_rejects_source_profile_request_and_revision_tampering() {
    let mut f = fixture();
    let context = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap();
    let valid_request = review_request(&f.source_hash, vec![choice(4, "2026-12-01")]);
    error_code(
        f.library
            .quote_legacy_calendar(&valid_request, context.revision.saturating_sub(1), ST),
        "STALE_REVISION",
    );
    let original_quote = f
        .library
        .quote_legacy_calendar(&valid_request, context.revision, ST)
        .unwrap();
    let revision_before = f.library.revision().unwrap();
    let audit_before = audit_count(&f.library.path);
    let plans_before = f.library.scheduled_sessions(ST).unwrap();

    let mut request_tamper = original_quote.clone();
    request_tamper
        .request
        .reason
        .push_str(" changed after review");
    error_code(
        f.library.commit_legacy_calendar(&request_tamper, ST),
        "STALE_REVISION",
    );
    let mut revision_tamper = original_quote.clone();
    revision_tamper.revision += 1;
    error_code(
        f.library.commit_legacy_calendar(&revision_tamper, ST),
        "STALE_REVISION",
    );
    let mut source_metadata_tamper = original_quote.clone();
    source_metadata_tamper.source_metadata_hash.push('0');
    error_code(
        f.library
            .commit_legacy_calendar(&source_metadata_tamper, ST),
        "STALE_REVISION",
    );

    let saved_profile = profile_data(&f.library.path);
    let mut altered_profile = saved_profile.clone();
    altered_profile["future_private_calendar_test_key"]["tamper"] = json!(true);
    set_profile_data(&f.library.path, &altered_profile);
    error_code(
        f.library.commit_legacy_calendar(&original_quote, ST),
        "STALE_REVISION",
    );
    set_profile_data(&f.library.path, &saved_profile);

    let saved_report = stored_import_report(&f.library.path, &f.source_hash);
    let mut altered_report: Value = serde_json::from_str(&saved_report).unwrap();
    let game_child = altered_report["metadata"]["calendar"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .position(|child| child["tag"] == "game")
        .unwrap();
    altered_report["metadata"]["calendar"]["children"][game_child]["attributes"][0]["value"] =
        json!("2026-10-30");
    set_import_report(&f.library.path, &f.source_hash, &altered_report);
    error_code(
        f.library.commit_legacy_calendar(&original_quote, ST),
        "STALE_REVISION",
    );
    set_import_report(
        &f.library.path,
        &f.source_hash,
        &serde_json::from_str(&saved_report).unwrap(),
    );

    let mut changed_bytes = f.source_bytes.clone();
    changed_bytes.push(b' ');
    set_import_bytes(&f.library.path, &f.source_hash, &changed_bytes);
    error_code(
        f.library.commit_legacy_calendar(&original_quote, ST),
        "LEGACY_CALENDAR_SOURCE_CHANGED",
    );
    set_import_bytes(&f.library.path, &f.source_hash, &f.source_bytes);

    assert_eq!(f.library.revision().unwrap(), revision_before);
    assert_eq!(audit_count(&f.library.path), audit_before);
    assert_eq!(f.library.scheduled_sessions(ST).unwrap(), plans_before);
    assert!(
        f.library
            .legacy_calendar_context(&f.source_hash, ST)
            .unwrap()
            .history
            .is_empty()
    );
    assert_eq!(fs::read(&f.source_path).unwrap(), f.source_bytes);
    assert_eq!(
        stored_import_bytes(&f.library.path, &f.source_hash),
        f.source_bytes
    );
}

#[test]
fn atomic_review_retry_conflict_cancellation_and_recovery_preserve_history() {
    let mut f = fixture();
    let quote = recovery_quote(&f);
    assert_eq!(quote.plans.len(), 2);
    let records_before = records_snapshot(&f.library.path);
    let xp_before = xp_snapshot(&f.library, &f.character_id);
    let raw_before = profile_data(&f.library.path);
    let profile_before = f.library.chronicle_profile(ST).unwrap().profile;
    let source_report_before = stored_import_report(&f.library.path, &f.source_hash);
    let source_bytes_before = stored_import_bytes(&f.library.path, &f.source_hash);
    let revision_before = f.library.revision().unwrap();
    let audit_before = audit_count(&f.library.path);
    let schedules_before = f.library.scheduled_sessions(ST).unwrap();

    Connection::open(&f.library.path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_legacy_calendar_audit BEFORE INSERT ON audit
             WHEN NEW.operation='legacy_calendar_import'
             BEGIN SELECT RAISE(ABORT, 'synthetic audit failure'); END;",
        )
        .unwrap();
    assert!(f.library.commit_legacy_calendar(&quote, ST).is_err());
    assert_eq!(f.library.revision().unwrap(), revision_before);
    assert_eq!(audit_count(&f.library.path), audit_before);
    assert_eq!(f.library.scheduled_sessions(ST).unwrap(), schedules_before);
    assert_eq!(profile_data(&f.library.path), raw_before);
    assert_eq!(
        f.library.chronicle_profile(ST).unwrap().profile,
        profile_before
    );
    assert_eq!(records_snapshot(&f.library.path), records_before);
    assert_eq!(xp_snapshot(&f.library, &f.character_id), xp_before);
    assert_eq!(
        stored_import_bytes(&f.library.path, &f.source_hash),
        source_bytes_before
    );
    assert_eq!(
        stored_import_report(&f.library.path, &f.source_hash),
        source_report_before
    );
    assert!(
        f.library
            .legacy_calendar_context(&f.source_hash, ST)
            .unwrap()
            .history
            .is_empty()
    );

    Connection::open(&f.library.path)
        .unwrap()
        .execute_batch("DROP TRIGGER reject_legacy_calendar_audit;")
        .unwrap();
    let receipt = f.library.commit_legacy_calendar(&quote, ST).unwrap();
    let committed_revision = f.library.revision().unwrap();
    assert_eq!(committed_revision, revision_before + 1);
    assert_eq!(
        audit_count(&f.library.path),
        audit_before + quote.plans.len() as i64 + 1
    );
    assert_eq!(
        f.library.commit_legacy_calendar(&quote, ST).unwrap(),
        receipt,
        "an exact request retry returns its stored receipt"
    );
    assert_eq!(f.library.revision().unwrap(), committed_revision);

    let mut conflicting_retry = quote.clone();
    conflicting_retry
        .request
        .reason
        .push_str(" conflicting retry");
    error_code(
        f.library.commit_legacy_calendar(&conflicting_retry, ST),
        "LEGACY_CALENDAR_ID",
    );
    let plan_id = receipt.plans[0].request.id.clone();
    let second_plan_id = receipt.plans[1].request.id.clone();
    f.library
        .cancel_scheduled_session(&plan_id, committed_revision, ST)
        .unwrap();
    let after_cancel = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap();
    let reimport = review_request(&f.source_hash, vec![choice(4, "2026-12-01")]);
    error_code(
        f.library
            .quote_legacy_calendar(&reimport, after_cancel.revision, ST),
        "LEGACY_CALENDAR_IMPORTED",
    );

    let backup = f.dir.path().join("legacy-calendar-backup.gvlib");
    f.library.backup(&backup, ST).unwrap();
    let mut later_profile = f.library.chronicle_profile(ST).unwrap().profile;
    later_profile.description = "Later synthetic-only change".into();
    let before_restore = f
        .library
        .save_chronicle_profile(&later_profile, f.library.revision().unwrap(), ST)
        .unwrap();
    f.library.restore(&backup, before_restore, ST).unwrap();
    let library_path = f.library.path.clone();
    drop(f.library);

    let mut restored = Library::open(&library_path).unwrap();
    assert_eq!(restored.title, profile_before.title);
    assert_eq!(
        restored.chronicle_profile(ST).unwrap().profile,
        profile_before
    );
    assert_eq!(
        restored
            .legacy_calendar_context(&f.source_hash, ST)
            .unwrap()
            .history,
        vec![receipt.clone()]
    );
    let scheduled = restored.scheduled_sessions(ST).unwrap();
    let restored_plan = scheduled
        .iter()
        .find(|session| session.request.id == plan_id)
        .unwrap();
    assert!(
        restored_plan.cancelled,
        "restore must not resurrect a canceled plan"
    );
    assert!(
        scheduled
            .iter()
            .any(|session| { session.request.id == second_plan_id && !session.cancelled })
    );

    let mut attended_plan = receipt.plans[1].request.clone();
    attended_plan.attendance = vec![Attendance {
        record_id: f.character_id.clone(),
        present: true,
        award: None,
    }];
    let plan_revision = restored
        .save_scheduled_session(&attended_plan, restored.revision().unwrap(), ST)
        .unwrap();
    let session_quote = restored
        .quote_session(&attended_plan, plan_revision, ST)
        .unwrap();
    restored.commit_session(&session_quote, ST).unwrap();
    assert!(
        restored
            .session_receipts(ST)
            .unwrap()
            .iter()
            .any(|session| session.quote.request.id == second_plan_id)
    );
    assert_eq!(xp_snapshot(&restored, &f.character_id), xp_before);

    let completed_context = restored
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap();
    error_code(
        restored.quote_legacy_calendar(
            &review_request(&f.source_hash, vec![choice(3, "2026-12-10")]),
            completed_context.revision,
            ST,
        ),
        "LEGACY_CALENDAR_IMPORTED",
    );
    let retry_revision = restored.revision().unwrap();
    let retry_schedules = restored.scheduled_sessions(ST).unwrap();
    let retry_audit = audit_count(&library_path);
    assert_eq!(
        restored.commit_legacy_calendar(&quote, ST).unwrap(),
        receipt,
        "historical retry remains idempotent after restore, completion and reopen"
    );
    assert_eq!(restored.revision().unwrap(), retry_revision);
    assert_eq!(restored.scheduled_sessions(ST).unwrap(), retry_schedules);
    assert_eq!(audit_count(&library_path), retry_audit);
    assert_eq!(records_snapshot(&library_path), records_before);
    assert_eq!(xp_snapshot(&restored, &f.character_id), xp_before);
    assert_eq!(
        stored_import_bytes(&library_path, &f.source_hash),
        source_bytes_before
    );
    assert_eq!(
        stored_import_report(&library_path, &f.source_hash),
        source_report_before
    );
    let profile_raw = profile_data(&library_path);
    assert_eq!(
        profile_raw["future_private_calendar_test_key"],
        raw_before["future_private_calendar_test_key"]
    );
    for token in [PRIVATE_ACTOR, PRIVATE_REASON, PRIVATE_REFERENCE] {
        assert!(profile_raw[calendar::FIELD].to_string().contains(token));
    }
    let public = serde_json::to_string(&restored.public_chronicle_profile().unwrap()).unwrap();
    for private in [
        calendar::FIELD,
        PRIVATE_ACTOR,
        PRIVATE_REASON,
        PRIVATE_REFERENCE,
    ] {
        assert!(!public.contains(private));
    }
}

#[test]
fn malformed_private_history_returns_a_static_error_without_echoing_data() {
    let f = fixture();
    let secret = "CORRUPT_PRIVATE_HISTORY_SENTINEL";
    let mut raw = profile_data(&f.library.path);
    raw[calendar::FIELD] = json!([{"private_untrusted_value": secret}]);
    set_profile_data(&f.library.path, &raw);

    let error = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap_err();
    assert_eq!(error.code, "LEGACY_CALENDAR_HISTORY");
    assert_eq!(
        error.message,
        "Private calendar review history is malformed. Restore a trusted backup."
    );
    assert!(!error.message.contains(secret));
    let public_error = f.library.public_chronicle_profile().unwrap_err();
    assert_eq!(public_error.code, "LEGACY_CALENDAR_HISTORY");
    assert!(!public_error.message.contains(secret));
    let pure_error = calendar::history(Some(&raw)).unwrap_err();
    assert_eq!(pure_error.code, "LEGACY_CALENDAR_HISTORY");
    assert!(!pure_error.message.contains(secret));
}

#[test]
fn source_choice_and_history_bounds_fail_without_library_writes() {
    let f = fixture();
    let initial_revision = f.library.revision().unwrap();
    let initial_schedules = f.library.scheduled_sessions(ST).unwrap();
    let assert_unchanged = || {
        assert_eq!(f.library.revision().unwrap(), initial_revision);
        assert_eq!(f.library.scheduled_sessions(ST).unwrap(), initial_schedules);
    };

    let context = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap();
    let too_many_choices = (0..65)
        .map(|entry_index| choice(entry_index, "2026-12-01"))
        .collect::<Vec<_>>();
    let error = f
        .library
        .quote_legacy_calendar(
            &review_request(&f.source_hash, too_many_choices),
            context.revision,
            ST,
        )
        .unwrap_err();
    assert_eq!(error.code, "LEGACY_CALENDAR_REVIEW");
    assert_eq!(
        error.message,
        "Select bounded source entries and record exact ISO days, review date, staff name, reason, reference and confirmation."
    );
    assert_unchanged();

    let saved_report = stored_import_report(&f.library.path, &f.source_hash);
    let too_many_entries = (0..4097)
        .map(|_| json!({"GameDate": "2026-12-01"}))
        .collect::<Vec<_>>();
    set_import_report(
        &f.library.path,
        &f.source_hash,
        &json!({"metadata": {"Calendar": {"Entries": too_many_entries}}}),
    );
    let error = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap_err();
    assert_eq!(error.code, "LEGACY_CALENDAR_SOURCE");
    assert_eq!(
        error.message,
        "Retained calendar metadata is unsupported or ambiguous; original evidence is preserved."
    );
    assert_unchanged();
    set_import_report(
        &f.library.path,
        &f.source_hash,
        &serde_json::from_str(&saved_report).unwrap(),
    );

    let context = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap();
    let receipt = calendar::Receipt {
        library_id: context.library_id,
        request: review_request(&f.source_hash, vec![choice(4, "2026-12-01")]),
        source_name: context.source_name,
        source_metadata_hash: context.source_metadata_hash,
        quote_hash: "a".repeat(64),
        plans: vec![],
        committed_revision: context.revision + 1,
    };
    let mut raw = profile_data(&f.library.path);
    raw[calendar::FIELD] = json!(vec![receipt.clone(); 129]);
    set_profile_data(&f.library.path, &raw);
    let error = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap_err();
    assert_eq!(error.code, "LEGACY_CALENDAR_HISTORY");
    assert_eq!(
        error.message,
        "Private calendar review history is malformed. Restore a trusted backup."
    );
    assert_unchanged();

    let mut oversized_receipt = receipt;
    oversized_receipt.request.actor = "A".repeat(15_999);
    oversized_receipt.request.reason = "R".repeat(15_999);
    oversized_receipt.request.review_reference = "E".repeat(15_999);
    raw[calendar::FIELD] = json!(vec![oversized_receipt; 128]);
    set_profile_data(&f.library.path, &raw);
    let error = f
        .library
        .legacy_calendar_context(&f.source_hash, ST)
        .unwrap_err();
    assert_eq!(error.code, "LEGACY_CALENDAR_HISTORY");
    assert_eq!(
        error.message,
        "Private calendar review history is malformed. Restore a trusted backup."
    );
    assert_unchanged();
}
