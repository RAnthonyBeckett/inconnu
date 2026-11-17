use grapevine_workbench::{
    game_sessions::{Attendance, SessionRequest},
    importer,
    model::{Language, Role},
    operations::{self, CalendarState, Content, Request},
    store::Library,
};
const ST: Role = Role::Storyteller;

fn session(title: &str, day: &str, id: &str) -> SessionRequest {
    SessionRequest {
        id: uuid::Uuid::new_v4().to_string(),
        title: title.into(),
        day: day.into(),
        actor: "PRIVATE_ACTOR".into(),
        reason: "PRIVATE_REASON".into(),
        evidence: "PRIVATE_EVIDENCE".into(),
        attendance: vec![Attendance {
            record_id: id.into(),
            present: true,
            award: None,
        }],
    }
}

#[test]
fn roster_and_blank_sign_in_are_explicit_scoped_private_snapshots() {
    let input=br#"<grapevine version="3"><player name="PRIVATE_PLAYER_LABEL" id="player1"/><vampire id="c1" name="A &lt;literal&gt;" player="PRIVATE_PLAYER_LABEL" status="Active" NPC="yes" Group="Court" Subgroup="Circle"><notes>PRIVATE_CHARACTER_NOTE</notes></vampire><mage id="c2" name="Other family" status="Active"/></grapevine>"#;
    let preview = importer::normalize(input, "synthetic.gex", "windows-1252").unwrap();
    let c = &preview
        .records
        .iter()
        .find(|r| r.kind == "Vampire")
        .unwrap()
        .id;
    let dir = tempfile::tempdir().unwrap();
    let mut lib =
        Library::create(&dir.path().join("reports.gvlib"), "Synthetic & company").unwrap();
    lib.commit_import(&preview, 0, false, ST).unwrap();
    let req = Request::Roster {
        family: "Vampire".into(),
        status: "active".into(),
        include_players: false,
    };
    assert!(lib.operations_snapshot(&req, Role::Player).is_err());
    let s = lib.operations_snapshot(&req, ST).unwrap();
    assert_eq!(s.revision, 1);
    let Content::Roster { rows, .. } = &s.content else {
        panic!("Expected roster")
    };
    assert_eq!(rows.len(), 1);
    assert!(rows[0].player.is_empty());
    let html = operations::html(&s, Language::English).unwrap();
    assert!(html.contains("A &lt;literal&gt;") && html.contains("Synthetic &amp; company"));
    assert!(!html.contains("Other family") && !html.contains("PRIVATE_"));
    let plan = session("Planned sign-in <literal>", "2026-11-10", c);
    lib.save_scheduled_session(&plan, 1, ST).unwrap();
    let req = Request::SignIn {
        schedule_id: plan.id.clone(),
        include_players: true,
    };
    let signed = lib.operations_snapshot(&req, ST).unwrap();
    let Content::SignIn { rows, missing, .. } = &signed.content else {
        panic!("Expected sign-in")
    };
    assert_eq!(*missing, 0);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].player, "PRIVATE_PLAYER_LABEL");
    let pt = operations::html(&signed, Language::Portuguese).unwrap();
    assert!(
        pt.contains("Folha de presença em branco")
            && pt.contains("<td></td>")
            && pt.contains("PRIVATE_PLAYER_LABEL")
    );
    for secret in [
        "PRIVATE_ACTOR",
        "PRIVATE_REASON",
        "PRIVATE_EVIDENCE",
        "PRIVATE_CHARACTER_NOTE",
    ] {
        assert!(!pt.contains(secret));
    }
    assert_eq!(lib.revision().unwrap(), 2);
    assert!(
        lib.session_receipts(ST).unwrap().is_empty(),
        "Printing must not commit attendance"
    );
}

#[test]
fn calendar_separates_plans_cancellations_and_committed_receipts() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("reports.gvlib"), "Synthetic").unwrap();
    let id = lib.create_character("Vampire", "Attendee", 0, ST).unwrap();
    let planned = session("Planned", "2026-11-11", &id);
    let cancelled = session("Cancelled", "2026-11-12", &id);
    let committed = session("Completed <literal>", "2026-11-10", &id);
    let outside = session("Out of range", "2026-12-01", &id);
    lib.save_scheduled_session(&planned, 1, ST).unwrap();
    lib.save_scheduled_session(&cancelled, 2, ST).unwrap();
    lib.cancel_scheduled_session(&cancelled.id, 3, ST).unwrap();
    lib.save_scheduled_session(&outside, 4, ST).unwrap();
    let q = lib.quote_session(&committed, 5, ST).unwrap();
    lib.commit_session(&q, ST).unwrap();
    let range = |include_cancelled| Request::Calendar {
        from: "2026-11-10".into(),
        through: "2026-11-12".into(),
        include_cancelled,
    };
    let s = lib.operations_snapshot(&range(false), ST).unwrap();
    let Content::Calendar { rows, .. } = &s.content else {
        panic!("Expected calendar")
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].state, CalendarState::Completed);
    assert_eq!(rows[0].attended, Some(1));
    assert_eq!(rows[1].state, CalendarState::Planned);
    assert_eq!(rows[1].attended, None);
    let with_cancelled = lib.operations_snapshot(&range(true), ST).unwrap();
    let Content::Calendar { rows, .. } = &with_cancelled.content else {
        panic!("Expected calendar")
    };
    assert_eq!(rows[2].state, CalendarState::Cancelled);
    let html = operations::html(&with_cancelled, Language::English).unwrap();
    assert!(
        html.contains("Completed &lt;literal&gt;") && html.contains("Includes cancelled plans")
    );
    assert!(!html.contains("Out of range") && !html.contains("PRIVATE_"));
    let unavailable = Request::SignIn {
        schedule_id: cancelled.id,
        include_players: false,
    };
    assert!(lib.operations_snapshot(&unavailable, ST).is_err());
    assert_eq!(lib.revision().unwrap(), 6);
}
