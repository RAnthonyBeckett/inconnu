use grapevine_workbench::{
    character_lifecycle as life,
    model::{BuildKind, Role},
    printing::Options,
    store::Library,
    xp::Baseline,
};
use serde_json::{Value, json};
use tempfile::TempDir;
const ST: Role = Role::Storyteller;
fn fixture(imported: bool) -> (TempDir, Library, String, life::Request) {
    let dir = TempDir::new().unwrap();
    let mut lib =
        Library::create(&dir.path().join("lifecycle.gvlib"), "Synthetic lifecycle").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic character", 0, ST)
        .unwrap();
    if imported {
        let conn = rusqlite::Connection::open(&lib.path).unwrap();
        let mut r = raw(&lib, &id);
        r["original"] = json!({"source":"PRESERVE_ORIGINAL"});
        r["source_sha256"] = json!("legacy-sha");
        conn.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![r.to_string(), id],
        )
        .unwrap();
    }
    let request = life::Request {
        day: "2026-11-14".into(),
        actor: "PRIVATE_LIFE_ACTOR".into(),
        reason: "PRIVATE_LIFE_REASON".into(),
        review_reference: "PRIVATE_LIFE_REVIEW".into(),
        confirmed: true,
        confirm_legacy: imported,
        ..Default::default()
    };
    (dir, lib, id, request)
}
fn raw(lib: &Library, id: &str) -> Value {
    let s: String = rusqlite::Connection::open(&lib.path)
        .unwrap()
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .unwrap();
    serde_json::from_str(&s).unwrap()
}
fn opening(lib: &mut Library, id: &str) {
    lib.initialize_xp(
        id,
        &Baseline {
            day: "2026-11-14".into(),
            earned: "10".into(),
            unspent: "10".into(),
            actor: "Synthetic".into(),
            reason: "Opening".into(),
            evidence: "Synthetic review".into(),
        },
        lib.revision().unwrap(),
        ST,
    )
    .unwrap();
}
fn preview(lib: &Library, id: &str, r: &life::Request) -> life::Quote {
    lib.quote_character_lifecycle(id, r, lib.revision().unwrap(), ST)
        .unwrap()
}
#[test]
fn imported_activation_retirement_return_drift_privacy_and_recovery() {
    let (dir, mut lib, id, request) = fixture(true);
    assert!(
        lib.quote_character_lifecycle(&id, &request, lib.revision().unwrap(), ST)
            .is_err()
    );
    opening(&mut lib, &id);
    let before = raw(&lib, &id);
    let q = preview(&lib, &id, &request);
    let receipt = lib.commit_character_lifecycle(&q, ST).unwrap();
    assert_eq!(lib.commit_character_lifecycle(&q, ST).unwrap(), receipt);
    let after = raw(&lib, &id);
    for field in ["original", "source_sha256", "xp_ledger", "id", "legacy_id"] {
        assert_eq!(before[field], after[field]);
    }
    let mut expected = before["public_fields"].clone();
    expected["Status"] = json!("Active");
    assert_eq!(expected, after["public_fields"]);
    assert_eq!(
        lib.character_lifecycle_context(&id, ST)
            .unwrap()
            .history
            .len(),
        1
    );
    for role in [ST, Role::Player] {
        assert!(
            !serde_json::to_string(&lib.record(&id, role).unwrap())
                .unwrap()
                .contains("PRIVATE_LIFE")
        );
        for private in [false, true] {
            let doc = lib.print_document(
                &id,
                &Options {
                    scope: if private {
                        grapevine_workbench::printing::PrintScope::PrivateStoryteller
                    } else {
                        grapevine_workbench::printing::PrintScope::Public
                    },
                    include_notes: true,
                    ..Default::default()
                },
                lib.revision().unwrap(),
                role,
            );
            if let Ok(doc) = doc {
                assert!(
                    !serde_json::to_string(&doc)
                        .unwrap()
                        .contains("PRIVATE_LIFE")
                );
            }
        }
    }
    let build = lib
        .create_build(&id, BuildKind::Test, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(lib.character_lifecycle_context(&build, ST).is_err());
    assert!(
        raw(&lib, &build)["private_fields"]
            .get(life::FIELD)
            .is_none()
    );
    let retire = life::Request {
        id: uuid::Uuid::new_v4().to_string(),
        action: life::Action::Retire,
        ..request.clone()
    };
    lib.commit_character_lifecycle(&preview(&lib, &id, &retire), ST)
        .unwrap();
    let backup = dir.path().join("retired-backup.gvlib");
    lib.backup(&backup, ST).unwrap();
    let mut back = life::Request {
        id: uuid::Uuid::new_v4().to_string(),
        ..request
    };
    assert!(
        lib.quote_character_lifecycle(&id, &back, lib.revision().unwrap(), ST)
            .is_err()
    );
    back.confirm_return = true;
    lib.commit_character_lifecycle(&preview(&lib, &id, &back), ST)
        .unwrap();
    let v = lib.record(&id, ST).unwrap();
    let edit = grapevine_workbench::model::Edit {
        name: v.name,
        status: "Custom legacy status".into(),
        notes: v.notes,
    };
    lib.save_edit(&id, &edit, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        lib.character_lifecycle_context(&id, ST)
            .unwrap()
            .status_drift
    );
    lib.restore(&backup, lib.revision().unwrap(), ST).unwrap();
    let path = lib.path.clone();
    drop(lib);
    let lib = Library::open(&path).unwrap();
    let ctx = lib.character_lifecycle_context(&id, ST).unwrap();
    assert_eq!(ctx.status, "Retired");
    assert_eq!(ctx.history.len(), 2);
    assert!(!ctx.status_drift);
    assert_eq!(before["xp_ledger"], raw(&lib, &id)["xp_ledger"]);
}
#[test]
fn role_creation_review_stale_tamper_retry_conflict_and_rollback() {
    let (_dir, mut lib, id, mut r) = fixture(false);
    opening(&mut lib, &id);
    assert!(
        lib.quote_character_lifecycle(&id, &r, lib.revision().unwrap(), ST)
            .is_err()
    );
    r.action = life::Action::Retire;
    let q = preview(&lib, &id, &r);
    assert!(lib.character_lifecycle_context(&id, Role::Player).is_err());
    assert!(lib.commit_character_lifecycle(&q, Role::Player).is_err());
    let mut forged = q.clone();
    forged.after = "Active".into();
    assert!(lib.commit_character_lifecycle(&forged, ST).is_err());
    let conn = rusqlite::Connection::open(&lib.path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_lifecycle BEFORE INSERT ON audit WHEN NEW.operation='character_lifecycle' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    let before = raw(&lib, &id);
    assert!(lib.commit_character_lifecycle(&q, ST).is_err());
    assert_eq!(before, raw(&lib, &id));
    assert_eq!(lib.revision().unwrap(), q.revision);
    conn.execute_batch("DROP TRIGGER fail_lifecycle;").unwrap();
    lib.commit_character_lifecycle(&q, ST).unwrap();
    forged = q.clone();
    forged.request.reason = "Different".into();
    assert!(lib.commit_character_lifecycle(&forged, ST).is_err());
    r.id = uuid::Uuid::new_v4().to_string();
    r.action = life::Action::Activate;
    r.confirm_return = true;
    assert!(
        lib.quote_character_lifecycle(&id, &r, lib.revision().unwrap(), ST)
            .is_err()
    );
    // Retirement is allowed without finished creation or XP; it never grants anything.
    let id2 = lib
        .create_character("Vampire", "Unfinished", lib.revision().unwrap(), ST)
        .unwrap();
    r.action = life::Action::Retire;
    let stale = preview(&lib, &id2, &r);
    lib.create_character("Mortal", "Concurrent", lib.revision().unwrap(), ST)
        .unwrap();
    assert!(lib.commit_character_lifecycle(&stale, ST).is_err());
}

#[test]
fn reviewed_new_creation_and_later_advancement_need_distinct_confirmation() {
    use grapevine_workbench::{
        pricing::PricingProfile,
        trait_advancement::{Request as Advance, Target},
        trait_pricing::Category,
    };
    let (_dir, mut lib, id, r) = fixture(false);
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

    let creation = lib
        .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(creation.ready, "{:?}", creation.issues);
    lib.commit_vampire_creation(&creation, ST).unwrap();
    assert!(
        lib.quote_character_lifecycle(&id, &r, lib.revision().unwrap(), ST)
            .is_err()
    );
    opening(&mut lib, &id);
    assert!(
        lib.quote_character_lifecycle(&id, &r, lib.revision().unwrap(), ST)
            .is_ok()
    );
    let original_sheet = lib.sheet_edit(&id, ST).unwrap();
    let mut changed = original_sheet.clone();
    changed
        .identity
        .insert("Clan".into(), "Changed after review".into());
    lib.save_sheet(&id, &changed, lib.revision().unwrap(), ST)
        .unwrap();
    let bypass = life::Request {
        confirm_post_creation: true,
        ..r.clone()
    };
    assert!(
        lib.quote_character_lifecycle(&id, &bypass, lib.revision().unwrap(), ST)
            .is_err()
    );
    lib.save_sheet(&id, &original_sheet, lib.revision().unwrap(), ST)
        .unwrap();
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
        reason: "Advance".into(),
        policy_version: "Synthetic v1".into(),
        review_reference: "Synthetic cap review".into(),
        review_confirmed: true,
    };
    let a = lib
        .quote_trait_advancement(&id, &advance, lib.revision().unwrap(), ST)
        .unwrap();
    lib.commit_trait_advancement(&a, ST).unwrap();
    assert!(
        lib.quote_character_lifecycle(&id, &r, lib.revision().unwrap(), ST)
            .is_err()
    );
    let reviewed = life::Request {
        confirm_post_creation: true,
        ..r
    };
    let before = raw(&lib, &id);
    lib.commit_character_lifecycle(&preview(&lib, &id, &reviewed), ST)
        .unwrap();
    assert_eq!(before["xp_ledger"], raw(&lib, &id)["xp_ledger"]);
    assert_eq!(
        lib.character_lifecycle_context(&id, ST).unwrap().status,
        "Active"
    );
}
