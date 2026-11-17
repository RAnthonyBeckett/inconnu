//! One entirely synthetic character crosses creation, session awards, purchases and recovery.
use grapevine_workbench::{
    character_lifecycle as lifecycle,
    chronicle::Profile,
    creation_sources,
    game_sessions::{Attendance, SessionRequest},
    legacy_calendar,
    legacy_profile::{self as legacy_profile, Field as LegacyField},
    mixed_advancement,
    model::{BuildKind, Edit, Role},
    pricing::{ClanRelation, PricingProfile},
    printing::{self, DocLanguage, Options},
    published_powers, published_reference,
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{PurchaseRequest, TeacherRule},
    safe_files,
    store::Library,
    trait_advancement::{Request as Advancement, Target},
    trait_corrections,
    trait_pricing::Category,
    xp::Baseline,
};
use serde_json::json;
use std::path::Path;
const ST: Role = Role::Storyteller;

fn retained_source_bytes(
    database: &Path,
    source_hash: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(rusqlite::Connection::open(database)?.query_row(
        "SELECT source_bytes FROM imports WHERE hash=?1",
        [source_hash],
        |row| row.get(0),
    )?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("fresh output directory required")?,
    );
    std::fs::create_dir(&out)?;
    let path = out.join("synthetic-vampire-cycle.gvlib");
    let mut lib = Library::create(&path, "Synthetic local Vampire cycle")?;
    let id = lib.create_character("Vampire", "Synthetic local Vampire <literal>", 0, ST)?;
    let mut request = lib.vampire_creation_context(&id, ST)?.request;
    for (key, value) in [
        ("Clan", "Ventrue"),
        ("Sect", "Camarilla"),
        ("Nature", "Director"),
        ("Demeanor", "Judge"),
    ] {
        request.sheet.identity.insert(key.into(), value.into());
    }
    for (key, rows) in [
        (
            "PhysicalList",
            json!([{"Name":"Strong","Value":"7","Note":"PRIVATE_CYCLE_TRAIT"}]),
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
            json!([{"Name":"Command","Value":"basic","Note":""},{"Name":"Awe","Value":"basic","Note":""},{"Name":"Dread Gaze","Value":"basic","Note":""}]),
        ),
    ] {
        request.sheet.lists.get_mut(key).unwrap()["Traits"] = rows;
    }
    request.budget.virtue_allocated = [3, 2, 2];
    request.standard_pools = true;
    request.review_confirmed = true;
    request.actor = "PRIVATE_CYCLE_ACTOR".into();
    request.day = "2026-11-13".into();
    request.reason = "PRIVATE_CYCLE_REASON".into();
    request.policy_version = "Synthetic MET profile v1".into();
    request.review_reference="Synthetic staff declaration: Ventrue starting eligibility, MET Basic mapping and progression reviewed against owned sources".into();
    for row in request.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"]
        .as_array_mut()
        .unwrap()
    {
        let name = row["Name"].as_str().unwrap();
        let source = published_powers::catalogue()?
            .entries
            .iter()
            .find(|p| p.book_id == "v20-core" && p.name == name)
            .ok_or("required named source absent")?;
        creation_sources::choose(row, &source.id)?;
        row[creation_sources::PROPOSED]["reviewed_signature"] =
            json!(published_reference::trait_signature(row)?);
    }
    let creation = lib.quote_vampire_creation(&id, &request, lib.revision()?, ST)?;
    assert!(creation.ready, "{:?}", creation.issues);
    assert_eq!(creation.published_sources.len(), 3);
    lib.commit_vampire_creation(&creation, ST)?;
    assert!(lib.xp_view(&id, ST)?.ledger.is_none());
    lib.initialize_xp(
        &id,
        &Baseline {
            day: "2026-11-13".into(),
            earned: "10".into(),
            unspent: "10".into(),
            actor: "PRIVATE_CYCLE_ACTOR".into(),
            reason: "Explicit reviewed opening; not creation currency".into(),
            evidence: "Synthetic historical opening declaration".into(),
        },
        lib.revision()?,
        ST,
    )?;
    assert_eq!(lib.xp_view(&id, ST)?.unspent.as_deref(), Some("10"));
    let draft = lib.record(&id, ST)?;
    assert_eq!(draft.status, "Draft");
    lib.save_edit(
        &id,
        &Edit {
            name: draft.name,
            status: "Draft".into(),
            notes: "PRIVATE_CYCLE_CHARACTER_NOTE".into(),
        },
        lib.revision()?,
        ST,
    )?;
    let activation = lifecycle::Request {
        action: lifecycle::Action::Activate,
        day: "2026-11-13".into(),
        actor: "PRIVATE_CYCLE_ACTOR".into(),
        reason: "Synthetic reviewed creation activation".into(),
        review_reference: "Synthetic initial creation and XP opening review".into(),
        confirmed: true,
        confirm_post_creation: true,
        ..Default::default()
    };
    let activation_quote = lib.quote_character_lifecycle(&id, &activation, lib.revision()?, ST)?;
    assert_eq!(
        (
            activation_quote.before.as_str(),
            activation_quote.after.as_str()
        ),
        ("Draft", "Active")
    );
    lib.commit_character_lifecycle(&activation_quote, ST)?;
    let activated = lib.character_lifecycle_context(&id, ST)?;
    assert_eq!(activated.status, "Active");
    assert_eq!(activated.history.len(), 1);
    assert!(!activated.status_drift);
    let session = SessionRequest {
        id: uuid::Uuid::new_v4().to_string(),
        title: "Synthetic session".into(),
        day: "2026-11-14".into(),
        actor: "PRIVATE_CYCLE_ACTOR".into(),
        reason: "Synthetic game award".into(),
        evidence: "PRIVATE_CYCLE_ATTENDANCE".into(),
        attendance: vec![Attendance {
            record_id: id.clone(),
            present: true,
            award: Some("2".into()),
        }],
    };
    let q = lib.quote_session(&session, lib.revision()?, ST)?;
    let session_receipt = lib.commit_session(&q, ST)?;
    assert_eq!(lib.commit_session(&q, ST)?, session_receipt);
    assert_eq!(lib.xp_view(&id, ST)?.unspent.as_deref(), Some("12"));
    let purchase = PurchaseRequest {
        review_id: None,
        id: uuid::Uuid::new_v4().to_string(),
        power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(ClanRelation::OutOfClan),
        affiliation_name: "Ventrue".into(),
        day: "2026-11-15".into(),
        available_from: "2026-11-16".into(),
        next_session_reference: "Synthetic next game".into(),
        actor: "PRIVATE_CYCLE_ACTOR".into(),
        reason: "PRIVATE_CYCLE_PURCHASE".into(),
        policy_version: "Synthetic MET profile v1".into(),
        policy_reference: "Synthetic local adoption".into(),
        affiliation_evidence: "Synthetic out-of-clan review".into(),
        approval_reference: "Synthetic staff ruling".into(),
        eligibility_evidence: "Synthetic source/prerequisite review".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Synthetic qualified teacher declaration".into(),
        standard_cost_confirmed: true,
    };
    let advancement = Advancement {
        id: uuid::Uuid::new_v4().to_string(),
        category: Category::Ability,
        target: Target::Trait {
            field: "AbilityList".into(),
            index: Some(0),
            name: "Academics".into(),
        },
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        show_on_player_sheet: true,
        day: "2026-11-15".into(),
        actor: "PRIVATE_CYCLE_ACTOR".into(),
        reason: "Synthetic advancement".into(),
        policy_version: "Synthetic v1".into(),
        review_reference: "Synthetic cap/eligibility review".into(),
        review_confirmed: true,
    };
    let plan_requests = vec![
        mixed_advancement::Request::Power(Box::new(purchase.clone())),
        mixed_advancement::Request::Trait(Box::new(advancement.clone())),
    ];
    let draft_id = lib.save_mixed_advancement_draft(&id, &plan_requests, lib.revision()?, ST)?;
    let saved_draft = lib.record(&draft_id, ST)?;
    assert_eq!(saved_draft.build, Some(BuildKind::Draft));
    assert_eq!(saved_draft.status, "Draft — private advancement plan");
    assert_eq!(lib.xp_view(&id, ST)?.unspent.as_deref(), Some("12"));
    let plan_review = lib.review_mixed_advancement_draft(&draft_id, lib.revision()?, ST)?;
    assert!(!plan_review.applied);
    assert_eq!(plan_review.quote.steps.len(), 2);
    assert_eq!(plan_review.quote.steps[0].request().id(), purchase.id);
    assert_eq!(plan_review.quote.steps[1].request().id(), advancement.id);
    assert_eq!(plan_review.quote.steps[0].cost(), "4");
    assert_eq!(plan_review.quote.steps[1].cost(), "1");
    assert_eq!(plan_review.quote.before_unspent, "12");
    assert_eq!(plan_review.quote.after_unspent, "7");
    let mixed_receipt = lib.apply_mixed_advancement_draft(&plan_review, ST)?;
    assert_eq!(mixed_receipt.quote, plan_review.quote);
    assert_eq!(mixed_receipt.actual_steps.len(), 2);
    assert!(
        lib.review_mixed_advancement_draft(&draft_id, lib.revision()?, ST)?
            .applied
    );
    assert_eq!(lib.mixed_advancement_history(&id, ST)?.len(), 1);
    assert_eq!(lib.xp_view(&id, ST)?.unspent.as_deref(), Some("7"));
    let correction = trait_corrections::Request {
        id: uuid::Uuid::new_v4().to_string(),
        advancement_id: advancement.id.clone(),
        day: "2026-11-15".into(),
        actor: "PRIVATE_CYCLE_ACTOR".into(),
        reason: "Synthetic clerical correction".into(),
        evidence: "PRIVATE_CYCLE_CORRECTION".into(),
    };
    let c = lib.quote_trait_correction(&id, &correction, lib.revision()?, ST)?;
    lib.commit_trait_correction(&c, ST)?;
    let view = lib.xp_view(&id, ST)?;
    assert_eq!(
        (view.earned.as_deref(), view.unspent.as_deref()),
        (Some("12"), Some("8"))
    );
    let traits_before_lifecycle = lib.sheet_edit(&id, ST)?;
    let xp_before_lifecycle = serde_json::to_value(lib.xp_view(&id, ST)?.ledger.as_ref())?;
    let retirement = lifecycle::Request {
        action: lifecycle::Action::Retire,
        day: "2026-11-15".into(),
        actor: "PRIVATE_CYCLE_ACTOR".into(),
        reason: "Synthetic reviewed retirement".into(),
        review_reference: "Synthetic staff review of cycle retirement".into(),
        confirmed: true,
        ..Default::default()
    };
    let retirement_quote = lib.quote_character_lifecycle(&id, &retirement, lib.revision()?, ST)?;
    assert_eq!(
        (
            retirement_quote.before.as_str(),
            retirement_quote.after.as_str()
        ),
        ("Active", "Retired")
    );
    lib.commit_character_lifecycle(&retirement_quote, ST)?;
    let retired = lib.character_lifecycle_context(&id, ST)?;
    assert_eq!(retired.status, "Retired");
    assert_eq!(retired.history.len(), 2);
    assert!(!retired.status_drift);
    assert_eq!(lib.sheet_edit(&id, ST)?, traits_before_lifecycle);
    assert_eq!(
        serde_json::to_value(lib.xp_view(&id, ST)?.ledger.as_ref())?,
        xp_before_lifecycle
    );
    let return_request = lifecycle::Request {
        action: lifecycle::Action::Activate,
        day: "2026-11-15".into(),
        actor: "PRIVATE_CYCLE_ACTOR".into(),
        reason: "Synthetic reviewed return after recorded advancement".into(),
        review_reference: "Synthetic staff review of advancement evidence and return".into(),
        confirmed: true,
        confirm_return: true,
        confirm_post_creation: true,
        ..Default::default()
    };
    let return_quote = lib.quote_character_lifecycle(&id, &return_request, lib.revision()?, ST)?;
    assert_eq!(
        (return_quote.before.as_str(), return_quote.after.as_str()),
        ("Retired", "Active")
    );
    lib.commit_character_lifecycle(&return_quote, ST)?;
    let returned = lib.character_lifecycle_context(&id, ST)?;
    assert_eq!(returned.status, "Active");
    assert_eq!(returned.history.len(), 3);
    assert!(!returned.status_drift);
    assert_eq!(lib.sheet_edit(&id, ST)?, traits_before_lifecycle);
    assert_eq!(
        serde_json::to_value(lib.xp_view(&id, ST)?.ledger.as_ref())?,
        xp_before_lifecycle
    );
    let mut profile_seed = lib.chronicle_profile(ST)?.profile;
    profile_seed.publish_details = true;
    profile_seed.publish_contacts = false;
    profile_seed.house_rules = "Synthetic house rules retained by review".into();
    profile_seed.private_notes = "KEEP_PRIVATE_CHRONICLE_NOTES".into();
    lib.save_chronicle_profile(&profile_seed, lib.revision()?, ST)?;
    let profile_before_import = lib.chronicle_profile(ST)?.profile;
    let legacy_xml = r#"<grapevine version="3" chronicle="Synthetic legacy Chronicle" website="https://legacy-cycle.invalid" email="legacy-cycle@example.invalid" phone="555-0198" usualtime="Friday evenings" stcommentstart="__STAFF_START__" stcommentend="__STAFF_END__"><player name="Synthetic profile test player" id="profile-test-player"/><usualplace>Community room __STAFF_START__LEGACY_PRIVATE_ST_TEXT__STAFF_END__ public lounge</usualplace><description>Synthetic legacy description</description><calendar><game date="2026-12-01" time="7pm"><place>PRIVATE_CYCLE_CALENDAR_PLACE</place><notes>PRIVATE_CYCLE_CALENDAR_NOTES</notes></game></calendar></grapevine>"#;
    let legacy_source = out.join("synthetic-legacy-chronicle.gex");
    std::fs::write(&legacy_source, legacy_xml.as_bytes())?;
    let legacy_source_bytes = std::fs::read(&legacy_source)?;
    let import_preview = lib.preview_import(&legacy_source, "windows-1252", ST)?;
    assert_eq!(import_preview.source_format, "grapevine-xml");
    assert_eq!(import_preview.records.len(), 1);
    let imported_player_id = import_preview
        .records
        .iter()
        .find(|record| record.kind == "Player")
        .ok_or("synthetic legacy source omitted its separate Player record")?
        .id
        .clone();
    let legacy_source_hash = import_preview.source_hash.clone();
    lib.commit_import(&import_preview, lib.revision()?, false, ST)?;
    let imported_player = lib.record(&imported_player_id, Role::Player)?;
    assert_eq!(imported_player.kind, "Player");
    assert_eq!(imported_player.name, "Synthetic profile test player");
    assert_eq!(
        retained_source_bytes(&path, &legacy_source_hash)?,
        legacy_source_bytes
    );
    let legacy_context = lib.legacy_profile_context(&legacy_source_hash, ST)?;
    assert_eq!(legacy_context.fields.len(), LegacyField::ALL.len());
    let place_candidate = legacy_context
        .fields
        .iter()
        .find(|candidate| candidate.field == LegacyField::MeetingPlace)
        .ok_or("legacy source omitted its meeting-place candidate")?;
    assert!(place_candidate.private_text_removed);
    assert!(place_candidate.would_be_public);
    let safe_place = place_candidate
        .proposed
        .as_deref()
        .ok_or("legacy meeting-place candidate was not reviewable")?;
    assert!(safe_place.contains("Community room"));
    assert!(safe_place.contains("public lounge"));
    assert!(!safe_place.contains("LEGACY_PRIVATE_ST_TEXT"));
    let legacy_request = legacy_profile::Request {
        source_hash: legacy_source_hash.clone(),
        selected: LegacyField::ALL.to_vec(),
        day: "2026-11-15".into(),
        actor: "PRIVATE_LEGACY_REVIEW_ACTOR".into(),
        reason: "PRIVATE_LEGACY_REVIEW_REASON".into(),
        review_reference: "PRIVATE_LEGACY_REVIEW_REFERENCE".into(),
        confirmed: true,
        confirm_overwrite: true,
        confirm_publication: true,
        ..Default::default()
    };
    let legacy_quote = lib.quote_legacy_profile(&legacy_request, legacy_context.revision, ST)?;
    assert_eq!(legacy_quote.changes.len(), LegacyField::ALL.len());
    assert_eq!(legacy_quote.proposed.title, "Synthetic legacy Chronicle");
    assert_eq!(legacy_quote.proposed.meeting_place, safe_place);
    assert_eq!(
        legacy_quote.proposed.publish_details,
        profile_before_import.publish_details
    );
    assert_eq!(
        legacy_quote.proposed.publish_contacts,
        profile_before_import.publish_contacts
    );
    assert_eq!(
        legacy_quote.proposed.house_rules,
        profile_before_import.house_rules
    );
    assert_eq!(
        legacy_quote.proposed.private_notes,
        profile_before_import.private_notes
    );
    let legacy_receipt = lib.commit_legacy_profile(&legacy_quote, ST)?;
    let saved_profile = lib.chronicle_profile(ST)?;
    assert_eq!(saved_profile.profile, legacy_quote.proposed);
    assert_eq!(lib.title, saved_profile.profile.title);
    assert_eq!(
        lib.legacy_profile_context(&legacy_source_hash, ST)?.history,
        vec![legacy_receipt.clone()]
    );
    assert_eq!(std::fs::read(&legacy_source)?, legacy_source_bytes);
    assert_eq!(
        retained_source_bytes(&path, &legacy_source_hash)?,
        legacy_source_bytes
    );
    let public_profile = lib.public_chronicle_profile()?;
    assert_eq!(public_profile.profile.title, saved_profile.profile.title);
    assert_eq!(public_profile.profile.email, "");
    assert_eq!(public_profile.profile.phone, "");
    let public_profile_json = serde_json::to_string(&public_profile)?;
    for private_value in [
        legacy_profile::FIELD,
        "PRIVATE_LEGACY_REVIEW_ACTOR",
        "PRIVATE_LEGACY_REVIEW_REASON",
        "PRIVATE_LEGACY_REVIEW_REFERENCE",
        "KEEP_PRIVATE_CHRONICLE_NOTES",
        "LEGACY_PRIVATE_ST_TEXT",
    ] {
        assert!(!public_profile_json.contains(private_value));
    }
    let calendar_context = lib.legacy_calendar_context(&legacy_source_hash, ST)?;
    assert_eq!(calendar_context.entries.len(), 1);
    assert_eq!(
        calendar_context.entries[0].proposed_day.as_deref(),
        Some("2026-12-01")
    );
    let calendar_request = legacy_calendar::Request {
        source_hash: legacy_source_hash.clone(),
        choices: vec![legacy_calendar::Choice {
            entry_index: 0,
            day: "2026-12-01".into(),
            title: "Synthetic future calendar placeholder".into(),
        }],
        review_day: "2026-11-15".into(),
        actor: "PRIVATE_CYCLE_CALENDAR_ACTOR".into(),
        reason: "PRIVATE_CYCLE_CALENDAR_REASON".into(),
        review_reference: "PRIVATE_CYCLE_CALENDAR_REFERENCE".into(),
        confirmed: true,
        ..Default::default()
    };
    let calendar_quote =
        lib.quote_legacy_calendar(&calendar_request, calendar_context.revision, ST)?;
    let calendar_receipt = lib.commit_legacy_calendar(&calendar_quote, ST)?;
    assert_eq!(lib.scheduled_sessions(ST)?.len(), 1);
    assert!(lib.scheduled_sessions(ST)?[0].request.attendance.is_empty());
    assert_eq!(lib.chronicle_profile(ST)?.profile, saved_profile.profile);
    assert_eq!(
        lib.legacy_profile_context(&legacy_source_hash, ST)?.history,
        vec![legacy_receipt.clone()]
    );
    let public_after_calendar = serde_json::to_string(&lib.public_chronicle_profile()?)?;
    assert!(!public_after_calendar.contains(legacy_calendar::FIELD));
    assert!(!public_after_calendar.contains("PRIVATE_CYCLE_CALENDAR"));
    let award_rows = lib.playtime_awards(&id, "2026-11", "2026-11", lib.revision()?, ST)?;
    assert_eq!(award_rows.len(), 1);
    let private_review = grapevine_workbench::playtime::Request {
        start_month: "2026-11".into(),
        end_month: "2026-11".into(),
        award_reviews: award_rows
            .into_iter()
            .map(|row| grapevine_workbench::playtime::AwardReview {
                entry_id: row.entry_id,
                entry_hash: row.entry_hash,
                class: grapevine_workbench::playtime::AwardClass::Regular,
                reason: "PRIVATE_CYCLE_SAVED_REVIEW_REASON".into(),
                reference: "PRIVATE_CYCLE_SAVED_REVIEW_REFERENCE".into(),
            })
            .collect(),
        ..Default::default()
    };
    let review_record_hash =
        lib.playtime_report(&id, &private_review, lib.revision()?, ST)?["record_hash"].clone();
    let saved_private_review = lib.save_playtime_award_review(
        &id,
        &uuid::Uuid::new_v4().to_string(),
        &private_review,
        lib.revision()?,
        ST,
    )?;
    assert_eq!(
        lib.playtime_report(&id, &private_review, lib.revision()?, ST)?["record_hash"],
        review_record_hash
    );
    assert_eq!(
        lib.legacy_profile_context(&legacy_source_hash, ST)?.history,
        vec![legacy_receipt.clone()]
    );
    assert_eq!(
        lib.legacy_calendar_context(&legacy_source_hash, ST)?
            .history,
        vec![calendar_receipt.clone()]
    );
    assert!(lib.session_receipts(Role::Player).is_err());
    for (label, language) in [
        ("en", DocLanguage::English),
        ("pt", DocLanguage::Portuguese),
    ] {
        let doc = lib.print_document(
            &id,
            &Options {
                language,
                ..Default::default()
            },
            lib.revision()?,
            Role::Player,
        )?;
        assert_eq!(doc.published_references.len(), 3);
        let html = printing::html(&doc)?;
        assert!(html.contains("Feral Whispers"));
        assert!(html.contains("&lt;literal&gt;"));
        for private_value in [
            "PRIVATE_CYCLE",
            legacy_profile::FIELD,
            legacy_calendar::FIELD,
            "PRIVATE_LEGACY_REVIEW_ACTOR",
            "PRIVATE_LEGACY_REVIEW_REASON",
            "PRIVATE_LEGACY_REVIEW_REFERENCE",
            "KEEP_PRIVATE_CHRONICLE_NOTES",
            "LEGACY_PRIVATE_ST_TEXT",
        ] {
            assert!(
                !html.contains(private_value),
                "Player HTML leaked {private_value}"
            );
        }
        safe_files::write_new(&out.join(format!("cycle-{label}.html")), html.as_bytes())?;
    }
    let backup = out.join("after-cycle.gvbackup");
    lib.backup(&backup, ST)?;
    let expected_profile: Profile = saved_profile.profile.clone();
    let mut after_backup_profile = lib.chronicle_profile(ST)?.profile;
    after_backup_profile.meeting_time = "AFTER_BACKUP_ONLY".into();
    lib.save_chronicle_profile(&after_backup_profile, lib.revision()?, ST)?;
    lib.save_edit(
        &id,
        &Edit {
            name: "Deliberate synthetic change".into(),
            status: "Active".into(),
            notes: String::new(),
        },
        lib.revision()?,
        ST,
    )?;
    lib.restore(&backup, lib.revision()?, ST)?;
    drop(lib);
    let lib = Library::open(&path)?;
    assert_eq!(
        lib.record(&id, Role::Player)?.name,
        "Synthetic local Vampire <literal>"
    );
    assert_eq!(lib.xp_view(&id, ST)?.unspent.as_deref(), Some("8"));
    assert_eq!(lib.published_reference_context(&id, ST)?.history.len(), 3);
    assert_eq!(lib.session_receipts(ST)?.len(), 1);
    assert_eq!(lib.purchase_receipts(&id, ST)?.len(), 1);
    let mixed_history_after_reopen = lib.mixed_advancement_history(&id, ST)?;
    assert_eq!(mixed_history_after_reopen, vec![mixed_receipt.clone()]);
    assert_eq!(mixed_history_after_reopen[0].quote.steps.len(), 2);
    assert_eq!(
        mixed_history_after_reopen[0].quote.steps[0].request().id(),
        purchase.id.as_str()
    );
    assert_eq!(
        mixed_history_after_reopen[0].quote.steps[1].request().id(),
        advancement.id.as_str()
    );
    let restored_draft = lib.record(&draft_id, ST)?;
    assert_eq!(restored_draft.build, Some(BuildKind::Draft));
    assert_eq!(restored_draft.status, "Draft — private advancement plan");
    let restored_draft_review =
        lib.review_mixed_advancement_draft(&draft_id, lib.revision()?, ST)?;
    assert!(restored_draft_review.applied);
    assert_eq!(restored_draft_review.quote, mixed_receipt.quote);
    let restored_profile = lib.chronicle_profile(ST)?;
    assert_eq!(restored_profile.profile, expected_profile);
    assert_eq!(lib.title, expected_profile.title);
    assert_ne!(restored_profile.profile.meeting_time, "AFTER_BACKUP_ONLY");
    assert_eq!(
        lib.legacy_profile_context(&legacy_source_hash, ST)?.history,
        vec![legacy_receipt.clone()]
    );
    let restored_profile_sources = lib.legacy_profile_sources(ST)?;
    let restored_source = restored_profile_sources
        .iter()
        .find(|source| source.hash == legacy_source_hash)
        .ok_or("restored legacy profile source was not retained")?;
    assert_eq!(restored_source.available_fields, LegacyField::ALL.len());
    assert!(restored_source.issue.is_none());
    assert_eq!(std::fs::read(&legacy_source)?, legacy_source_bytes);
    assert_eq!(
        retained_source_bytes(&path, &legacy_source_hash)?,
        legacy_source_bytes
    );
    let restored_public = lib.public_chronicle_profile()?;
    let restored_public_json = serde_json::to_string(&restored_public)?;
    for private_value in [
        legacy_profile::FIELD,
        "PRIVATE_LEGACY_REVIEW_ACTOR",
        "PRIVATE_LEGACY_REVIEW_REASON",
        "PRIVATE_LEGACY_REVIEW_REFERENCE",
        "KEEP_PRIVATE_CHRONICLE_NOTES",
        "LEGACY_PRIVATE_ST_TEXT",
    ] {
        assert!(!restored_public_json.contains(private_value));
    }
    let restored_player = lib.record(&imported_player_id, Role::Player)?;
    assert_eq!(restored_player.kind, "Player");
    assert_eq!(
        lib.legacy_calendar_context(&legacy_source_hash, ST)?
            .history,
        vec![calendar_receipt.clone()]
    );
    let restored_calendar = lib.scheduled_sessions(ST)?;
    assert_eq!(restored_calendar.len(), 1);
    assert_eq!(
        restored_calendar[0].request,
        calendar_receipt.plans[0].request
    );
    assert!(restored_calendar[0].request.attendance.is_empty());
    assert!(lib.legacy_calendar_sources(Role::Player).is_err());
    let calendar_public = serde_json::to_string(&lib.public_chronicle_profile()?)?;
    assert!(!calendar_public.contains(legacy_calendar::FIELD));
    assert!(!calendar_public.contains("PRIVATE_CYCLE_CALENDAR"));
    assert_eq!(
        lib.playtime_award_reviews(&id, ST)?,
        vec![saved_private_review.clone()]
    );
    let loaded_private =
        lib.load_playtime_award_review(&id, &saved_private_review.id, lib.revision()?, ST)?;
    assert_eq!(
        loaded_private.input.award_reviews,
        saved_private_review.award_reviews
    );
    assert_eq!(loaded_private.stale_reviews, 0);
    assert_eq!(loaded_private.unclassified_awards, 0);
    assert_eq!(
        loaded_private.input.class,
        grapevine_workbench::playtime::CapClass::Unknown
    );
    assert!(
        !loaded_private.input.policy_scope_confirmed
            && !loaded_private.input.full_month_eligible_confirmed
            && !loaded_private.input.complete_regular_history_confirmed
    );
    assert!(
        loaded_private.input.home_chronicle.is_empty()
            && loaded_private.input.review_evidence.is_empty()
    );
    assert!(lib.playtime_award_reviews(&id, Role::Player).is_err());
    assert!(
        !serde_json::to_string(&lib.public_chronicle_profile()?)?
            .contains(grapevine_workbench::playtime_review::FIELD)
    );
    let lifecycle_after_reopen = lib.character_lifecycle_context(&id, ST)?;
    assert_eq!(lifecycle_after_reopen.status, "Active");
    assert_eq!(lifecycle_after_reopen.history.len(), 3);
    assert!(!lifecycle_after_reopen.status_drift);
    assert_eq!(
        lifecycle_after_reopen
            .history
            .iter()
            .map(|receipt| (receipt.quote.before.as_str(), receipt.quote.after.as_str()))
            .collect::<Vec<_>>(),
        [
            ("Draft", "Active"),
            ("Active", "Retired"),
            ("Retired", "Active")
        ]
    );
    assert_eq!(
        lifecycle_after_reopen.history[0].quote.request.action,
        lifecycle::Action::Activate
    );
    assert_eq!(
        lifecycle_after_reopen.history[1].quote.request.action,
        lifecycle::Action::Retire
    );
    assert!(
        lifecycle_after_reopen.history[2]
            .quote
            .request
            .confirm_return
    );
    assert!(
        lifecycle_after_reopen.history[2]
            .quote
            .request
            .confirm_post_creation
    );
    assert_eq!(lib.sheet_edit(&id, ST)?, traits_before_lifecycle);
    assert_eq!(
        serde_json::to_value(lib.xp_view(&id, ST)?.ledger.as_ref())?,
        xp_before_lifecycle
    );
    let result = json!({"synthetic_only":true,"creation_sources":3,"creation_points_not_xp":true,"session_award_atomic_retry":true,"out_of_clan_purchase_cost":"4","ordinary_advancement_and_correction":true,"final_earned":"12","final_unspent":"8","public_print_privacy_en_pt":true,"restore_and_reopen":true,"official_approval_claimed":false,"reviewed_lifecycle_and_recovery":true,"mixed_saved_plan_and_recovery":true,"legacy_profile_and_recovery":true,"legacy_calendar_plans_and_recovery":true,"private_review_saved_and_recovered":true});
    safe_files::write_new(
        &out.join("results.json"),
        serde_json::to_vec_pretty(&result)?.as_slice(),
    )?;
    println!("{result}");
    Ok(())
}
