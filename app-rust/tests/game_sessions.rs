use grapevine_workbench::{
    game_sessions::{self, Attendance, SessionRequest},
    model::{BuildInfo, BuildKind, Record},
    xp::{Baseline, Entry, Ledger},
};
use serde_json::{Map, json};
use uuid::Uuid;

fn record(kind: &str, with_ledger: bool) -> Record {
    Record {
        id: Uuid::new_v4().to_string(),
        kind: kind.into(),
        name: format!("{kind} character"),
        player_id: None,
        legacy_id: None,
        public_fields: Map::new(),
        private_fields: Map::from_iter([("PrivateNotes".into(), json!("DO_NOT_COPY"))]),
        original: json!({}),
        source_sha256: "synthetic".into(),
        st_markers: None,
        build: None,
        xp_ledger: with_ledger.then(|| Ledger {
            baseline: Baseline {
                day: "2026-01-01".into(),
                earned: "10".into(),
                unspent: "10".into(),
                actor: "Initial recorder".into(),
                reason: "Synthetic opening".into(),
                evidence: "fixture".into(),
            },
            entries: vec![],
        }),
    }
}

fn request(attendance: Vec<Attendance>) -> SessionRequest {
    SessionRequest {
        id: Uuid::new_v4().to_string(),
        title: "Session One".into(),
        day: "2026-02-01".into(),
        actor: "ST".into(),
        reason: "Session award".into(),
        evidence: "session notes 2026-02-01".into(),
        attendance,
    }
}

fn attendance(record: &Record, present: bool, award: Option<&str>) -> Attendance {
    Attendance {
        record_id: record.id.clone(),
        present,
        award: award.map(str::to_owned),
    }
}

#[test]
fn mixed_template_awards_keep_absent_and_excluded_attendance_and_private_notes_out() {
    let vampire = record("Vampire", true);
    let risen = record("Risen", true);
    let excluded = record("Mage", false);
    let absent = record("Wraith", false);
    let records = [
        vampire.clone(),
        risen.clone(),
        excluded.clone(),
        absent.clone(),
    ];
    let request = request(vec![
        attendance(&vampire, true, Some("2.50")),
        attendance(&risen, true, Some("1.25")),
        attendance(&excluded, true, None),
        attendance(&absent, false, None),
    ]);
    let before = serde_json::to_value(&records).unwrap();
    let quote = game_sessions::prepare("library-a", 7, &records, &request).unwrap();

    assert_eq!(quote.request.attendance, request.attendance);
    assert_eq!(quote.awards.len(), 2);
    assert_eq!(quote.awards[0].command.amount, "2.5");
    assert_eq!(quote.awards[1].command.amount, "1.25");
    assert_eq!(quote.awards[0].before_unspent, "10");
    assert_eq!(quote.awards[0].after_unspent, "12.5");
    assert_eq!(quote.record_hashes.len(), 4);
    let encoded = serde_json::to_string(&quote).unwrap();
    assert!(!encoded.contains("DO_NOT_COPY"));
    assert_eq!(serde_json::to_value(&records).unwrap(), before);
}

#[test]
fn attendance_only_session_needs_no_ledgers_and_creates_no_awards() {
    let present = record("Changeling", false);
    let absent = record("Demon", false);
    let records = [present.clone(), absent.clone()];
    let request = request(vec![
        attendance(&present, true, None),
        attendance(&absent, false, None),
    ]);
    let quote = game_sessions::prepare("library-a", 0, &records, &request).unwrap();
    assert!(quote.awards.is_empty());
    assert_eq!(quote.request.attendance.len(), 2);
}

#[test]
fn invalid_amount_missing_ledger_and_absent_award_reject_the_whole_quote() {
    let ledgered = record("Vampire", true);
    let no_ledger = record("Risen", false);
    let records = [ledgered.clone(), no_ledger.clone()];
    for (rows, code) in [
        (vec![attendance(&ledgered, true, Some("0"))], "INVALID_XP"),
        (
            vec![
                attendance(&ledgered, true, Some("1")),
                attendance(&no_ledger, true, Some("1")),
            ],
            "SESSION_XP_OPENING_REQUIRED",
        ),
        (
            vec![attendance(&ledgered, false, Some("1"))],
            "SESSION_ABSENT_AWARD",
        ),
    ] {
        assert_eq!(
            game_sessions::prepare("library-a", 1, &records, &request(rows))
                .unwrap_err()
                .code,
            code
        );
    }
}

#[test]
fn deterministic_commands_and_duplicate_command_ids_are_checked() {
    let vampire = record("Vampire", true);
    let records = [vampire.clone()];
    let request = request(vec![attendance(&vampire, true, Some("1.125"))]);
    let first = game_sessions::prepare("library-a", 1, &records, &request).unwrap();
    let second = game_sessions::prepare("library-a", 1, &records, &request).unwrap();
    assert_eq!(first.awards[0].command, second.awards[0].command);

    let mut applied = vampire.clone();
    applied.xp_ledger.as_mut().unwrap().entries.push(Entry {
        command: first.awards[0].command.clone(),
        recorded_unix_seconds: 1,
        origin_library_id: "library-a".into(),
    });
    assert_eq!(
        game_sessions::prepare("library-a", 1, &[applied], &request)
            .unwrap_err()
            .code,
        "SESSION_COMMAND_DUPLICATE"
    );
}

#[test]
fn builds_noncharacters_missing_records_and_duplicate_attendance_are_rejected() {
    let mut build = record("Vampire", true);
    build.build = Some(BuildInfo {
        source_id: Uuid::new_v4().to_string(),
        source_revision: 1,
        kind: BuildKind::Test,
    });
    let noncharacter = record("Player", true);
    let ordinary = record("Vampire", true);
    for (records, rows, code) in [
        (
            vec![build.clone()],
            vec![attendance(&build, true, None)],
            "SESSION_CHARACTER_REQUIRED",
        ),
        (
            vec![noncharacter.clone()],
            vec![attendance(&noncharacter, true, None)],
            "SESSION_CHARACTER_REQUIRED",
        ),
        (
            vec![ordinary.clone()],
            vec![Attendance {
                record_id: Uuid::new_v4().to_string(),
                present: true,
                award: None,
            }],
            "SESSION_RECORD_MISSING",
        ),
        (
            vec![ordinary.clone()],
            vec![
                attendance(&ordinary, true, None),
                attendance(&ordinary, false, None),
            ],
            "SESSION_ATTENDANCE_DUPLICATE",
        ),
    ] {
        assert_eq!(
            game_sessions::prepare("library-a", 1, &records, &request(rows))
                .unwrap_err()
                .code,
            code
        );
    }
}
