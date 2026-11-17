use grapevine_workbench::{game_sessions::*, model::Role, store::Library, xp::Baseline};
use tempfile::TempDir;
#[test]
fn session_batch_is_atomic_idempotent_private_and_durable() {
    let d = TempDir::new().unwrap();
    let path = d.path().join("session.gvlib");
    let mut l = Library::create(&path, "Synthetic session").unwrap();
    let mut ids = Vec::new();
    for kind in ["Vampire", "Risen"] {
        let id = l
            .create_character(kind, "Synthetic", l.revision().unwrap(), Role::Storyteller)
            .unwrap();
        l.initialize_xp(
            &id,
            &Baseline {
                day: "2026-01-01".into(),
                earned: "10".into(),
                unspent: "10".into(),
                actor: "ST".into(),
                reason: "Synthetic".into(),
                evidence: "Synthetic".into(),
            },
            l.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
        ids.push(id);
    }
    let req = SessionRequest {
        id: uuid::Uuid::new_v4().to_string(),
        title: "Synthetic session".into(),
        day: "2026-01-02".into(),
        actor: "ST".into(),
        reason: "Manual award".into(),
        evidence: "Attendance".into(),
        attendance: ids
            .iter()
            .map(|id| Attendance {
                record_id: id.clone(),
                present: true,
                award: Some("2".into()),
            })
            .collect(),
    };
    let empty_backup = d.path().join("before-session.gvbackup");
    l.backup(&empty_backup, Role::Storyteller).unwrap();
    let rev = l.revision().unwrap();
    assert!(l.quote_session(&req, rev, Role::Player).is_err());
    let q = l.quote_session(&req, rev, Role::Storyteller).unwrap();
    assert!(l.commit_session(&q, Role::Player).is_err());
    let mut changed = q.clone();
    changed.awards[0].after_unspent = "999".into();
    assert!(l.commit_session(&changed, Role::Storyteller).is_err());
    assert_eq!(l.revision().unwrap(), rev);
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch("CREATE TRIGGER reject_second_award BEFORE UPDATE ON records WHEN OLD.kind='Risen' BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    assert!(l.commit_session(&q, Role::Storyteller).is_err());
    assert_eq!(l.revision().unwrap(), rev);
    assert!(l.session_receipts(Role::Storyteller).unwrap().is_empty());
    assert_eq!(l.quote_session(&req, rev, Role::Storyteller).unwrap(), q);
    c.execute_batch("DROP TRIGGER reject_second_award;")
        .unwrap();
    drop(c);
    let receipt = l.commit_session(&q, Role::Storyteller).unwrap();
    assert_eq!(l.commit_session(&q, Role::Storyteller).unwrap(), receipt);
    assert_eq!(l.revision().unwrap(), rev + 1);
    assert!(l.commit_session(&changed, Role::Storyteller).is_err());
    assert!(l.session_receipts(Role::Player).is_err());
    assert!(
        l.session_report(
            &q.request.id,
            grapevine_workbench::model::Language::English,
            Role::Player
        )
        .is_err()
    );
    let en = l
        .session_report(
            &q.request.id,
            grapevine_workbench::model::Language::English,
            Role::Storyteller,
        )
        .unwrap();
    let pt = l
        .session_report(
            &q.request.id,
            grapevine_workbench::model::Language::Portuguese,
            Role::Storyteller,
        )
        .unwrap();
    assert!(en.contains("Manual amounts") && pt.contains("Valores manuais"));
    assert!(!en.contains(&q.request.actor) && !en.contains("record_hashes"));
    let mut escaped = receipt.clone();
    escaped.quote.request.title = "<script>alert(1)</script>".into();
    let html = grapevine_workbench::session_report::html(
        &escaped,
        grapevine_workbench::model::Language::English,
    )
    .unwrap();
    assert!(html.contains("&lt;script&gt;") && !html.contains("<script>"));
    let backup = d.path().join("backup.gvbackup");
    l.backup(&backup, Role::Storyteller).unwrap();
    drop(l);
    let mut l = Library::open(&path).unwrap();
    assert_eq!(
        l.session_receipts(Role::Storyteller).unwrap(),
        vec![receipt.clone()]
    );
    assert_eq!(l.commit_session(&q, Role::Storyteller).unwrap(), receipt);
    let mut stale = req.clone();
    stale.id = uuid::Uuid::new_v4().to_string();
    assert!(l.quote_session(&stale, rev, Role::Storyteller).is_err());
    l.restore(&backup, l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(
        l.session_receipts(Role::Storyteller).unwrap(),
        vec![receipt.clone()]
    );
    l.restore(&empty_backup, l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert!(l.session_receipts(Role::Storyteller).unwrap().is_empty());
    assert!(
        l.session_report(
            &q.request.id,
            grapevine_workbench::model::Language::English,
            Role::Storyteller
        )
        .is_err()
    );
    l.restore(&backup, l.revision().unwrap(), Role::Storyteller)
        .unwrap();
    assert_eq!(
        l.session_receipts(Role::Storyteller).unwrap(),
        vec![receipt]
    );
}
