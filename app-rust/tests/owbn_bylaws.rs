use grapevine_workbench::{
    mixed_advancement,
    model::{Role, SheetEdit},
    owbn_bylaws::{
        self as owbn, Evidence, LevelKind, PacketAuthority, Regulation, RegulationEvidence,
        SelectedPower, TierMappingBasis,
    },
    pricing::{ClanRelation, MetTier, PricingProfile},
    printing::{DocLanguage, Options},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{PurchaseRequest, TeacherRule},
    store::Library,
    xp::Baseline,
};

const ST: Role = Role::Storyteller;

fn request() -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: uuid::Uuid::new_v4().to_string(),
        power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(ClanRelation::OutOfClan),
        affiliation_name: "Synthetic Clan".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic session".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic acquisition".into(),
        policy_version: "synthetic".into(),
        policy_reference: "Local policy".into(),
        affiliation_evidence: "Local affiliation review".into(),
        approval_reference: "Ordinary local approval".into(),
        eligibility_evidence: "Local eligibility review".into(),
        teacher_rule: Some(TeacherRule::NotRequiredByLocalRuling),
        teacher_evidence: "Local interpretation".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Evidence::default(),
    }
}

fn library_with_vampire(generation: i64) -> (tempfile::TempDir, Library, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("synthetic.gvlib"), "Synthetic").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic Vampire", 0, ST)
        .unwrap();
    let mut sheet = SheetEdit::default();
    sheet.resources.insert("Generation".into(), generation);
    lib.save_sheet(&id, &sheet, 1, ST).unwrap();
    lib.initialize_xp(
        &id,
        &Baseline {
            day: "2026-01-01".into(),
            earned: "20".into(),
            unspent: "20".into(),
            actor: "Synthetic ST".into(),
            reason: "Synthetic opening".into(),
            evidence: "Synthetic fixture".into(),
        },
        2,
        ST,
    )
    .unwrap();
    (dir, lib, id)
}

fn enable(lib: &mut Library, allow_non_binding_sources: bool) {
    let mut snapshot = lib.chronicle_profile(ST).unwrap();
    snapshot.profile.owbn_bylaws_enabled = true;
    snapshot.profile.allow_non_binding_sources = allow_non_binding_sources;
    lib.save_chronicle_profile(&snapshot.profile, snapshot.revision, ST)
        .unwrap();
}

#[test]
fn policy_off_allows_a_pc_below_eighth_and_on_blocks_every_purchase_route() {
    let (_dir, mut lib, id) = library_with_vampire(7);
    let request = request();
    let old = lib
        .quote_purchase(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    enable(&mut lib, true);
    let revision = lib.revision().unwrap();
    let error = lib.quote_purchase(&id, &request, revision, ST).unwrap_err();
    assert_eq!(error.code, "OWBN_PC_GENERATION");
    assert_eq!(
        lib.quote_purchase_batch(&id, std::slice::from_ref(&request), revision, ST)
            .unwrap_err()
            .code,
        "OWBN_PC_GENERATION"
    );
    assert_eq!(
        lib.quote_mixed_advancement(
            &id,
            &[mixed_advancement::Request::Power(Box::new(request))],
            revision,
            ST
        )
        .unwrap_err()
        .code,
        "OWBN_PC_GENERATION"
    );
    assert!(lib.commit_purchase(&old, ST).is_err());
    assert_eq!(lib.revision().unwrap(), revision);
    assert!(
        lib.xp_view(&id, ST)
            .unwrap()
            .ledger
            .unwrap()
            .entries
            .is_empty()
    );
}

#[test]
fn eighth_generation_can_buy_with_separate_teacher_and_source_evidence() {
    let (_dir, mut lib, id) = library_with_vampire(8);
    enable(&mut lib, false);
    let revision = lib.revision().unwrap();
    let mut request = request();
    assert_eq!(
        lib.quote_purchase(&id, &request, revision, ST)
            .unwrap_err()
            .code,
        "OWBN_TEACHER_REQUIRED"
    );
    request.owbn_evidence.teacher_exception_reference = "SYNTHETIC_PRIVATE_PACKET_REF".into();
    assert_eq!(
        lib.quote_purchase(&id, &request, revision, ST)
            .unwrap_err()
            .code,
        "OWBN_NON_BINDING_SOURCE"
    );
    request.owbn_evidence.confirm_non_binding_sources = true;
    let quote = lib.quote_purchase(&id, &request, revision, ST).unwrap();
    let mut edited = quote.clone();
    edited
        .request
        .owbn_evidence
        .teacher_exception_reference
        .clear();
    assert_eq!(
        lib.commit_purchase(&edited, ST).unwrap_err().code,
        "OWBN_TEACHER_REQUIRED"
    );
    let receipt = lib.commit_purchase(&quote, ST).unwrap();
    assert_eq!(lib.commit_purchase(&quote, ST).unwrap(), receipt);
    assert!(
        !serde_json::to_string(&lib.record(&id, Role::Player).unwrap())
            .unwrap()
            .contains("SYNTHETIC_PRIVATE_PACKET_REF")
    );
}

fn evidence(id: &str, regulation: Regulation, reference: &str) -> RegulationEvidence {
    RegulationEvidence {
        rule_id: id.into(),
        regulation,
        owbn_coordinator_reference: reference.into(),
    }
}

#[test]
fn regulation_tiers_and_pc_disallow_are_distinct() {
    let policy = owbn::builtin();
    let mut sheet = SheetEdit::default();
    sheet.identity.insert("Guild".into(), " mNeMoI ".into());
    let rules = policy.matching("Wraith", &sheet, &[], false);
    let mut proof = Evidence {
        references: vec![evidence(
            "wraith-mnemoi",
            Regulation::CoordinatorNotify,
            "notice-001",
        )],
        ..Evidence::default()
    };
    assert_eq!(
        policy
            .check_regulations(&rules, false, &proof)
            .unwrap_err()
            .code,
        "OWBN_REGULATION_EVIDENCE"
    );
    let npc_rules = policy.matching("Wraith", &sheet, &[], true);
    assert!(policy.check_regulations(&npc_rules, true, &proof).is_ok());
    proof.references[0] = evidence(
        "wraith-mnemoi",
        Regulation::CoordinatorApproval,
        "approval-001",
    );
    assert!(policy.check_regulations(&rules, false, &proof).is_ok());
    sheet.identity.insert("Guild".into(), "Ferrymen".into());
    let rules = policy.matching("Wraith", &sheet, &[], false);
    proof.references[0] = evidence(
        "wraith-ferrymen",
        Regulation::CoordinatorApproval,
        "approval-002",
    );
    assert_eq!(
        policy
            .check_regulations(&rules, false, &proof)
            .unwrap_err()
            .code,
        "OWBN_REGULATION_EVIDENCE"
    );
    proof.references[0] = evidence("wraith-ferrymen", Regulation::CouncilVote, "   ");
    assert!(policy.check_regulations(&rules, false, &proof).is_err());
    proof.references[0].owbn_coordinator_reference = "vote-001".into();
    assert!(policy.check_regulations(&rules, false, &proof).is_ok());
    let sparagmoi = [SelectedPower {
        names: &["Sparagmoi"],
        level: Some(4),
    }];
    let pc = policy.matching("Wraith", &sheet, &sparagmoi, false);
    assert_eq!(
        policy
            .check_regulations(&pc, false, &proof)
            .unwrap_err()
            .code,
        "OWBN_PC_DISALLOWED"
    );
    let npc = policy.matching("Wraith", &sheet, &sparagmoi, true);
    assert!(policy.check_regulations(&npc, true, &proof).is_ok());
    let tier_five = [SelectedPower {
        names: &["Sparagmoi"],
        level: Some(5),
    }];
    let npc = policy.matching("Wraith", &sheet, &tier_five, true);
    assert!(policy.check_regulations(&npc, true, &proof).is_err());
    proof.references.push(evidence(
        "wraith-sparagmoi-tier-5",
        Regulation::CoordinatorApproval,
        "approval-003",
    ));
    proof.validate().unwrap();
    assert!(policy.check_regulations(&npc, true, &proof).is_ok());
    proof.references[1].owbn_coordinator_reference = "vote-001".into();
    assert_eq!(proof.validate().unwrap_err().code, "OWBN_EVIDENCE_INVALID");
}

#[test]
fn binding_and_bylaw_rules_take_precedence_over_local_and_book_rules() {
    let mut policy = owbn::builtin().clone();
    let mut binding = policy
        .rules
        .iter()
        .find(|rule| rule.id == "wraith-mnemoi")
        .unwrap()
        .clone();
    binding.id = "synthetic-binding".into();
    binding.authority = PacketAuthority::BindingPacket;
    binding.pc = Regulation::CoordinatorNotify;
    let mut book = binding.clone();
    book.id = "synthetic-book".into();
    book.authority = PacketAuthority::NonBindingOrBook;
    book.pc = Regulation::Unregulated;
    let mut local = book.clone();
    local.id = "synthetic-local".into();
    local.authority = PacketAuthority::CustomHouseRule;
    policy.rules.extend([binding, book, local]);
    let mut sheet = SheetEdit::default();
    sheet.identity.insert("Guild".into(), "Mnemoi".into());
    let rules = policy.matching("Wraith", &sheet, &[], false);
    assert_eq!(rules[0].authority, PacketAuthority::BylawMandated);
    policy.rules.retain(|rule| rule.id != "wraith-mnemoi");
    let rules = policy.matching("Wraith", &sheet, &[], false);
    assert_eq!(rules[0].authority, PacketAuthority::BindingPacket);
    assert_eq!(rules[0].regulation(false), Regulation::CoordinatorNotify);
}

#[test]
fn creation_requires_the_specific_risen_reference_and_a_reviewed_start() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("synthetic.gvlib"), "Synthetic").unwrap();
    let id = lib
        .create_character("Risen", "Synthetic Risen", 0, ST)
        .unwrap();
    let mut request = lib.vampire_creation_context(&id, ST).unwrap().request;
    enable(&mut lib, true);
    let rev = lib.revision().unwrap();
    for npc in [false, true] {
        request.create_as_npc = npc;
        request.owbn_evidence.references.clear();
        let q = lib.quote_vampire_creation(&id, &request, rev, ST).unwrap();
        assert!(!q.ready);
        assert!(q.issues.iter().any(|issue| issue.starts_with("risen:")));
        request.owbn_evidence.references.push(evidence(
            "risen",
            Regulation::CoordinatorApproval,
            "synthetic-risen-approval",
        ));
        let q = lib.quote_vampire_creation(&id, &request, rev, ST).unwrap();
        assert!(!q.ready);
        assert!(!q.issues.iter().any(|issue| issue.starts_with("risen:")));
        assert!(
            q.issues
                .iter()
                .any(|issue| issue.starts_with("Confirm the Risen origin"))
        );
    }
}

#[test]
fn chronicle_flags_survive_backup_and_reminders_are_opt_in() {
    let (dir, mut lib, id) = library_with_vampire(8);
    enable(&mut lib, true);
    let backup = dir.path().join("backup.gvlib");
    lib.backup(&backup, ST).unwrap();
    let mut profile = lib.chronicle_profile(ST).unwrap();
    profile.profile.owbn_bylaws_enabled = false;
    lib.save_chronicle_profile(&profile.profile, profile.revision, ST)
        .unwrap();
    lib.restore(&backup, lib.revision().unwrap(), ST).unwrap();
    let profile = lib.public_chronicle_profile().unwrap();
    assert!(profile.profile.owbn_bylaws_enabled && profile.profile.allow_non_binding_sources);
    let mut options = Options::default();
    let mut sheet = lib
        .print_document(&id, &options, profile.revision, Role::Player)
        .unwrap();
    assert!(
        !sheet
            .sections
            .iter()
            .any(|section| section.title.contains("OWbN"))
    );
    for language in [DocLanguage::English, DocLanguage::Portuguese] {
        options.include_owbn_mechanics = true;
        options.language = language;
        sheet = lib
            .print_document(&id, &options, profile.revision, Role::Player)
            .unwrap();
        let reminders = sheet
            .sections
            .iter()
            .find(|section| section.title.contains("OWbN"))
            .unwrap();
        for (row, number) in reminders.rows.iter().zip(["30", "10", "", "9"]) {
            assert!(row.value.contains(number));
        }
        assert_eq!(reminders.rows.len(), 4);
    }
}

#[test]
fn numeric_ceilings_and_twentieth_conversion_are_separate() {
    for (kind, limit) in [
        (LevelKind::Discipline, 5),
        (LevelKind::SorceryPath, 6),
        (LevelKind::NuminaPath, 5),
    ] {
        assert!(owbn::check_power_level(false, kind, limit).is_ok());
        assert_eq!(
            owbn::check_power_level(false, kind, limit + 1)
                .unwrap_err()
                .code,
            "OWBN_PC_POWER_CEILING"
        );
        assert!(owbn::check_power_level(true, kind, limit + 1).is_ok());
    }
    let mapping = TierMappingBasis::convert_20th_level;
    assert_eq!(
        (1..=5).map(mapping).collect::<Vec<_>>(),
        [
            MetTier::Basic,
            MetTier::Basic,
            MetTier::Intermediate,
            MetTier::Intermediate,
            MetTier::Advanced
        ]
        .map(Some)
    );
    assert_eq!(mapping(6), None);
}
