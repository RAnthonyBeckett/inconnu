use grapevine_workbench::{
    game_sessions::{Attendance, SessionRequest},
    model::Role,
    store::Library,
};
use rusqlite::Connection;
use tempfile::TempDir;

fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn request(n: u64, character_id: &str, award: Option<&str>) -> SessionRequest {
    SessionRequest {
        id: id(n),
        title: format!("Synthetic session {n}"),
        day: "2026-02-01".into(),
        actor: "Synthetic ST".into(),
        reason: "Scheduled game".into(),
        evidence: "Synthetic plan".into(),
        attendance: vec![Attendance {
            record_id: character_id.into(),
            present: true,
            award: award.map(str::to_owned),
        }],
    }
}

fn opened() -> (TempDir, Library, String) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("scheduled.gvlib");
    let mut library = Library::create(&path, "Synthetic schedules").unwrap();
    let character = library
        .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
        .unwrap();
    (dir, library, character)
}

fn record_data(library: &Library, id: &str) -> String {
    Connection::open(&library.path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn save_retry_reopen_edit_stale_and_cancelled_edit_rejection() {
    let (dir, mut library, character) = opened();
    let first = request(1, &character, Some("2"));
    let initial_revision = library.revision().unwrap();
    let saved_revision = library
        .save_scheduled_session(&first, initial_revision, Role::Storyteller)
        .unwrap();
    assert_eq!(saved_revision, initial_revision + 1);
    assert_eq!(
        library
            .save_scheduled_session(&first, initial_revision, Role::Storyteller)
            .unwrap(),
        saved_revision
    );
    assert_eq!(library.revision().unwrap(), saved_revision);
    library
        .create_character(
            "Wraith",
            "Synthetic other",
            saved_revision,
            Role::Storyteller,
        )
        .unwrap();
    let after_unrelated_write = library.revision().unwrap();
    assert_eq!(
        library
            .save_scheduled_session(&first, initial_revision, Role::Storyteller)
            .unwrap(),
        after_unrelated_write
    );
    assert_eq!(library.revision().unwrap(), after_unrelated_write);

    let path = dir.path().join("scheduled.gvlib");
    drop(library);
    library = Library::open(&path).unwrap();
    assert_eq!(
        library.scheduled_sessions(Role::Storyteller).unwrap()[0].request,
        first
    );

    let mut edited = first.clone();
    edited.title = "Edited synthetic session".into();
    let current_revision = library.revision().unwrap();
    let edited_revision = library
        .save_scheduled_session(&edited, current_revision, Role::Storyteller)
        .unwrap();
    assert!(
        library
            .save_scheduled_session(&first, saved_revision, Role::Storyteller)
            .is_err()
    );
    assert_eq!(
        library.scheduled_sessions(Role::Storyteller).unwrap()[0].request,
        edited
    );

    let cancelled_revision = library
        .cancel_scheduled_session(&edited.id, edited_revision, Role::Storyteller)
        .unwrap();
    library
        .create_character(
            "Wraith",
            "Synthetic after cancel",
            cancelled_revision,
            Role::Storyteller,
        )
        .unwrap();
    let after_cancel_write = library.revision().unwrap();
    assert_eq!(
        library
            .cancel_scheduled_session(&edited.id, edited_revision, Role::Storyteller)
            .unwrap(),
        after_cancel_write
    );
    assert!(after_cancel_write > cancelled_revision);
    assert_eq!(library.revision().unwrap(), after_cancel_write);
    let cancelled = &library.scheduled_sessions(Role::Storyteller).unwrap()[0];
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.request, edited);
    assert!(
        library
            .save_scheduled_session(&first, cancelled_revision, Role::Storyteller)
            .is_err()
    );
}

#[test]
fn player_role_cannot_read_save_or_cancel_schedules() {
    let (_dir, mut library, character) = opened();
    let request = request(2, &character, None);
    assert!(library.scheduled_sessions(Role::Player).is_err());
    assert!(
        library
            .save_scheduled_session(&request, library.revision().unwrap(), Role::Player)
            .is_err()
    );
    assert!(
        library
            .cancel_scheduled_session(&request.id, library.revision().unwrap(), Role::Player)
            .is_err()
    );
}

#[test]
fn scheduled_award_needs_no_xp_opening_and_never_mutates_character_record() {
    let (_dir, mut library, character) = opened();
    let before = record_data(&library, &character);
    let request = request(3, &character, Some("1.25"));
    library
        .save_scheduled_session(&request, library.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(record_data(&library, &character), before);
}

#[test]
fn schedule_validation_rejects_absent_or_nonpositive_awards() {
    let (_dir, mut library, character) = opened();
    let revision = library.revision().unwrap();
    let mut absent = request(4, &character, Some("1"));
    absent.attendance[0].present = false;
    assert!(
        library
            .save_scheduled_session(&absent, revision, Role::Storyteller)
            .is_err()
    );
    let mut zero = request(5, &character, Some("0"));
    assert!(
        library
            .save_scheduled_session(&zero, revision, Role::Storyteller)
            .is_err()
    );
    zero.attendance[0].award = Some("1".into());
    assert!(
        library
            .save_scheduled_session(&zero, revision, Role::Storyteller)
            .is_ok()
    );
}

#[test]
fn empty_attendance_is_allowed_for_a_calendar_plan_but_not_a_commit_quote() {
    let (_dir, mut library, _character) = opened();
    let calendar_plan = SessionRequest {
        id: id(10),
        title: "Synthetic calendar plan".into(),
        day: "2026-02-12".into(),
        actor: "Synthetic ST".into(),
        reason: "Future event".into(),
        evidence: "Synthetic calendar".into(),
        attendance: Vec::new(),
    };
    let saved_revision = library
        .save_scheduled_session(
            &calendar_plan,
            library.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    assert_eq!(
        library.scheduled_sessions(Role::Storyteller).unwrap()[0].request,
        calendar_plan
    );
    assert!(
        library
            .quote_session(&calendar_plan, saved_revision, Role::Storyteller)
            .is_err()
    );
}

#[test]
fn backup_restore_preserves_scheduled_session_rows() {
    let (dir, mut library, character) = opened();
    let scheduled = request(6, &character, None);
    let revision = library.revision().unwrap();
    library
        .save_scheduled_session(&scheduled, revision, Role::Storyteller)
        .unwrap();
    let backup = dir.path().join("schedule-backup.gvlib");
    library.backup(&backup, Role::Storyteller).unwrap();

    let second = request(7, &character, None);
    library
        .save_scheduled_session(&second, library.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let current_revision = library.revision().unwrap();
    library
        .restore(&backup, current_revision, Role::Storyteller)
        .unwrap();
    let entries = library.scheduled_sessions(Role::Storyteller).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].request, scheduled);
}

#[test]
fn committed_receipt_replaces_plan_and_cancelled_plan_cannot_be_committed() {
    let (_dir, mut library, character) = opened();
    let planned = request(8, &character, None);
    let saved_revision = library
        .save_scheduled_session(&planned, library.revision().unwrap(), Role::Storyteller)
        .unwrap();
    let quote = library
        .quote_session(&planned, saved_revision, Role::Storyteller)
        .unwrap();
    library.commit_session(&quote, Role::Storyteller).unwrap();
    assert!(
        library
            .scheduled_sessions(Role::Storyteller)
            .unwrap()
            .iter()
            .all(|entry| entry.request.id != planned.id)
    );
    assert!(
        library
            .session_receipts(Role::Storyteller)
            .unwrap()
            .iter()
            .any(|receipt| receipt.quote.request.id == planned.id)
    );
    assert!(
        library
            .save_scheduled_session(&planned, library.revision().unwrap(), Role::Storyteller)
            .is_err()
    );

    let cancelled_plan = request(9, &character, None);
    let saved_revision = library
        .save_scheduled_session(
            &cancelled_plan,
            library.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
    let cancelled_quote = library
        .quote_session(&cancelled_plan, saved_revision, Role::Storyteller)
        .unwrap();
    library
        .cancel_scheduled_session(&cancelled_plan.id, saved_revision, Role::Storyteller)
        .unwrap();
    assert!(
        library
            .commit_session(&cancelled_quote, Role::Storyteller)
            .is_err()
    );
}
