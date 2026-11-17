use grapevine_workbench::{
    model::{BuildKind, Language, Role},
    operations::{self, Content, Request},
    player_points as pp,
    store::Library,
    xp,
};
use serde_json::json;
const ST: Role = Role::Storyteller;

#[test]
fn history_reports_are_private_scoped_readonly_and_pool_specific() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.gvlib");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    let character = lib
        .create_character("Vampire", "Character pool", 0, ST)
        .unwrap();
    let player = lib.create_player("Player pool", 1, ST).unwrap();
    lib.initialize_xp(
        &character,
        &xp::Baseline {
            day: "2026-01-01".into(),
            earned: "1".into(),
            unspent: "1".into(),
            actor: "SECRET_ACTOR".into(),
            reason: "SECRET_OPENING_REASON".into(),
            evidence: "SECRET_EVIDENCE".into(),
        },
        2,
        ST,
    )
    .unwrap();
    lib.initialize_player_points(
        &player,
        &pp::Baseline {
            day: "2026-01-01".into(),
            balance: "2".into(),
            actor: "SECRET_ACTOR".into(),
            reason: "SECRET_OPENING_REASON".into(),
            evidence: "SECRET_EVIDENCE".into(),
        },
        3,
        ST,
    )
    .unwrap();
    let command = xp::XpCommand {
        id: "spend1".into(),
        kind: xp::XpKind::Spend,
        amount: "0.1".into(),
        day: "2026-01-02".into(),
        actor: "SECRET_ACTOR".into(),
        reason: "SECRET_REASON".into(),
        evidence: "SECRET_EVIDENCE".into(),
        reversal_of: None,
        cost: None,
    };
    lib.post_xp(&character, &command, 4, ST).unwrap();
    lib.post_xp(
        &character,
        &xp::XpCommand {
            id: "reverse1".into(),
            kind: xp::XpKind::Reversal,
            amount: String::new(),
            day: "2026-01-03".into(),
            reversal_of: Some(command.id),
            ..command
        },
        5,
        ST,
    )
    .unwrap();
    let award = pp::Command {
        id: uuid::Uuid::new_v4().to_string(),
        kind: pp::Kind::Award,
        amount: "0.2".into(),
        day: "2026-01-02".into(),
        actor: "SECRET_ACTOR".into(),
        reason: "SECRET_REASON".into(),
        evidence: "SECRET_EVIDENCE".into(),
        reversal_of: None,
    };
    lib.post_player_points(&player, &award, 6, ST).unwrap();
    lib.post_player_points(
        &player,
        &pp::Command {
            id: uuid::Uuid::new_v4().to_string(),
            kind: pp::Kind::Reversal,
            amount: String::new(),
            day: "2026-01-03".into(),
            reversal_of: Some(award.id.clone()),
            ..award
        },
        7,
        ST,
    )
    .unwrap();
    let request = |player_pool, id: String, reasons| {
        if player_pool {
            Request::PlayerHistory {
                record_id: id,
                from: "2026-01-03".into(),
                through: "2026-01-03".into(),
                include_reasons: reasons,
            }
        } else {
            Request::CharacterHistory {
                record_id: id,
                from: "2026-01-03".into(),
                through: "2026-01-03".into(),
                include_reasons: reasons,
            }
        }
    };
    for (player_pool, id) in [(false, &character), (true, &player)] {
        let req = request(player_pool, id.clone(), false);
        assert!(lib.operations_snapshot(&req, Role::Player).is_err());
        let snapshot = lib.operations_snapshot(&req, ST).unwrap();
        let Content::History(h) = &snapshot.content else {
            panic!("Expected history")
        };
        assert!(
            !format!("{h:?}").contains("SECRET_"),
            "Remove private metadata from the projection itself"
        );
        let html = operations::html(&snapshot, Language::English).unwrap();
        assert!(!html.contains("SECRET_"));
        let opted = lib
            .operations_snapshot(&request(player_pool, id.clone(), true), ST)
            .unwrap();
        let opted = operations::html(&opted, Language::Portuguese).unwrap();
        assert!(opted.contains("SECRET_REASON"));
        assert!(!opted.contains("SECRET_ACTOR") && !opted.contains("SECRET_EVIDENCE"));
        assert!(
            lib.operations_snapshot(&request(!player_pool, id.clone(), false), ST)
                .is_err()
        );
    }
    assert_eq!(lib.revision().unwrap(), 8);
    assert_eq!(
        lib.xp_view(&character, ST).unwrap().unspent.as_deref(),
        Some("1")
    );
    assert_eq!(
        pp::replay(&lib.player_points(&player, ST).unwrap().unwrap()).unwrap(),
        "2"
    );
    let build = lib
        .create_build(&character, BuildKind::Draft, 8, ST)
        .unwrap();
    assert!(
        lib.operations_snapshot(&request(false, build, false), ST)
            .is_err()
    );
}

#[test]
fn player_roster_drops_contact_and_identifiers_before_projection_unless_requested() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("roster.gvlib"), "Synthetic").unwrap();
    let p = lib.create_player("Café <literal>", 0, ST).unwrap();
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    let raw: String = conn
        .query_row("SELECT data FROM records WHERE id=?1", [&p], |r| r.get(0))
        .unwrap();
    let mut raw: serde_json::Value = serde_json::from_str(&raw).unwrap();
    raw["legacy_id"] = json!("SECRET_LEGACY_ID");
    raw["private_fields"]["EMail"] = json!("SECRET_EMAIL");
    raw["private_fields"]["Phone"] = json!("SECRET_PHONE");
    raw["private_fields"]["Position"] = json!("Narrator");
    raw["private_fields"]["Status"] = json!("Active");
    raw["private_fields"]["Notes"] = json!("SECRET_NOTES");
    conn.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        rusqlite::params![raw.to_string(), p],
    )
    .unwrap();
    let request = |contacts, ids| Request::PlayerRoster {
        status: "active".into(),
        include_contacts: contacts,
        include_legacy_ids: ids,
    };
    let private = lib.operations_snapshot(&request(false, false), ST).unwrap();
    assert!(!format!("{:?}", private.content).contains("SECRET_"));
    let html = operations::html(&private, Language::English).unwrap();
    assert!(html.contains("Café &lt;literal&gt;") && !html.contains("SECRET_"));
    let contact = lib.operations_snapshot(&request(true, false), ST).unwrap();
    let html = operations::html(&contact, Language::English).unwrap();
    assert!(html.contains("SECRET_EMAIL") && html.contains("SECRET_PHONE"));
    assert!(!html.contains("SECRET_LEGACY_ID") && !html.contains("SECRET_NOTES"));
    let ids = lib.operations_snapshot(&request(false, true), ST).unwrap();
    let html = operations::html(&ids, Language::English).unwrap();
    assert!(html.contains("SECRET_LEGACY_ID") && !html.contains("SECRET_EMAIL"));
    assert!(
        lib.operations_snapshot(&request(true, true), Role::Player)
            .is_err()
    );
    assert_eq!(lib.revision().unwrap(), 1);
}

#[test]
fn xml_history_remains_evidence_without_creating_either_local_ledger() {
    let bytes=br#"<grapevine version="3"><vampire name="XML Character"><experience earned="10,5" unspent="7,25"><entry date="2026-01-02" change="0,25" type="0" reason="SECRET_LEGACY_REASON" earned="10,5" unspent="7,25"/><entry date="01/02/2026" change="1" type="3" reason="SECRET_LEGACY_REASON" earned="10,5" unspent="6,25"/></experience></vampire><player name="XML Player"><experience earned="9" unspent="8"><entry date="2026-01-02" change="1" type="0" reason="SECRET_LEGACY_REASON" earned="9" unspent="8"/></experience></player></grapevine>"#;
    let preview =
        grapevine_workbench::importer::normalize(bytes, "synthetic.gex", "windows-1252").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("xml.gvlib"), "Synthetic").unwrap();
    lib.commit_import(&preview, 0, false, ST).unwrap();
    for record in &preview.records {
        let request = if record.kind == "Player" {
            Request::PlayerHistory {
                record_id: record.id.clone(),
                from: "2026-01-01".into(),
                through: "2026-01-31".into(),
                include_reasons: false,
            }
        } else {
            Request::CharacterHistory {
                record_id: record.id.clone(),
                from: "2026-01-01".into(),
                through: "2026-01-31".into(),
                include_reasons: false,
            }
        };
        let snapshot = lib.operations_snapshot(&request, ST).unwrap();
        let html = operations::html(&snapshot, Language::English).unwrap();
        assert!(!html.contains("SECRET_LEGACY_REASON"));
        if record.kind == "Player" {
            assert!(lib.player_points(&record.id, ST).unwrap().is_none());
        } else {
            assert!(lib.xp_view(&record.id, ST).unwrap().ledger.is_none());
            assert!(
                html.contains("0,25") && html.contains("10,5"),
                "Keep legacy decimal notation verbatim"
            );
        }
    }
    assert_eq!(lib.revision().unwrap(), 1);
}
