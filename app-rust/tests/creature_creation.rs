use grapevine_workbench::{
    creature_profiles, model::Role, store::Library, vampire_creation as creation,
};
use serde_json::json;
use tempfile::TempDir;

const ST: Role = Role::Storyteller;
const NON_VAMPIRE_KINDS: &[&str] = &[
    "Werewolf", "Mortal", "Wraith", "Mage", "Fera", "Various", "Mummy", "KueiJin", "Hunter",
    "Demon", "Risen",
];

fn reviewed_request(lib: &Library, id: &str) -> creation::Request {
    let mut request = lib.vampire_creation_context(id, ST).unwrap().request;
    request.standard_pools = true;
    request.review_confirmed = true;
    request.actor = "Synthetic reviewer".into();
    request.day = "2026-11-16".into();
    request.reason = "Synthetic creation review".into();
    request.policy_version = "Synthetic profile v1".into();
    request.review_reference = "Synthetic local eligibility review".into();
    request.manual_power_reference = "Synthetic local starting-power review".into();
    request
}

#[test]
fn every_non_vampire_profile_has_a_blocked_configured_preview() {
    let directory = TempDir::new().unwrap();
    let mut lib = Library::create(&directory.path().join("creatures.gvlib"), "Creatures").unwrap();

    for (index, kind) in NON_VAMPIRE_KINDS.iter().enumerate() {
        let id = lib
            .create_character(
                kind,
                &format!("Synthetic {kind}"),
                lib.revision().unwrap(),
                ST,
            )
            .unwrap();
        let request = reviewed_request(&lib, &id);
        let quote = lib
            .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
            .unwrap();
        let profile = creature_profiles::profile(kind).unwrap();

        assert!(quote.is_configured());
        assert_eq!(quote.configured_profile.as_ref().unwrap().kind, *kind);
        assert!(
            !quote.ready,
            "{kind} must remain blocked pending source review"
        );
        assert!(
            quote
                .issues
                .iter()
                .any(|issue| issue.contains("not reviewed"))
        );
        assert_eq!(quote.generation.generation, 0);
        assert!(
            quote
                .budget
                .issues
                .iter()
                .any(|issue| issue.contains("do not apply"))
        );
        assert!(
            quote
                .effective_sheet
                .lists
                .contains_key(&profile.primary_power_list)
        );
        assert!(
            quote
                .effective_sheet
                .lists
                .contains_key(&profile.secondary_power_list)
        );
        assert!(quote.effective_sheet.resources.is_empty());

        let revision = lib.revision().unwrap();
        assert!(lib.commit_vampire_creation(&quote, ST).is_err());
        assert_eq!(
            lib.revision().unwrap(),
            revision,
            "{kind} must not be applied"
        );
        assert_eq!(index + 1, lib.revision().unwrap() as usize);
    }
}

#[test]
fn configured_pool_arithmetic_uses_profile_bases_and_blocks_vampire_options() {
    let directory = TempDir::new().unwrap();
    let mut lib =
        Library::create(&directory.path().join("configured.gvlib"), "Configured").unwrap();
    let id = lib
        .create_character("Werewolf", "Synthetic Werewolf", 0, ST)
        .unwrap();
    let mut request = reviewed_request(&lib, &id);
    for (key, rows) in [
        ("PhysicalList", json!([{"Name":"P1","Value":"7"}])),
        ("SocialList", json!([{"Name":"S1","Value":"5"}])),
        ("MentalList", json!([{"Name":"M1","Value":"3"}])),
        ("AbilityList", json!([{"Name":"A1","Value":"5"}])),
        ("BackgroundList", json!([{"Name":"B1","Value":"5"}])),
        ("PhysicalNegList", json!([{"Name":"N1","Value":"1"}])),
        ("FlawList", json!([{"Name":"F1","Value":"2"}])),
        (
            "GiftList",
            json!([
                {"Name":"Gift 1","Value":"basic"},
                {"Name":"Gift 2","Value":"basic"},
                {"Name":"Gift 3","Value":"basic"}
            ]),
        ),
    ] {
        request.sheet.lists.get_mut(key).unwrap()["Traits"] = rows;
    }
    let baseline = lib
        .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    let configured = baseline.configured_budget.as_ref().unwrap();
    assert_eq!(configured.attribute_bases, [7, 5, 3]);
    assert_eq!(configured.attribute_remaining, [0; 3]);
    assert_eq!(configured.ability_remaining, 0);
    assert_eq!(configured.background_remaining, 0);
    assert_eq!(configured.primary_power_remaining, 0);
    assert_eq!(configured.negative_credit, 1);
    assert_eq!(configured.flaw_credit, 2);
    assert_eq!(configured.free_available, 8);

    request.budget.sabbat = true;
    request.budget.optional_derangement = true;
    request.budget.lower_morality = true;
    request.budget.virtue_allocated = [3, 2, 2];
    request.budget.virtue_free = [1, 0, 0];
    request.budget.conviction = true;
    request.budget.instinct = true;
    request.budget.morality_free = 1;
    request.budget.willpower_free = 1;
    let blocked = lib
        .quote_vampire_creation(&id, &request, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(!blocked.ready);
    assert!(
        blocked
            .issues
            .iter()
            .any(|issue| issue.contains("Vampire-only"))
    );
    assert_eq!(
        blocked.configured_budget.unwrap().free_available,
        configured.free_available
    );
    assert!(blocked.effective_sheet.resources.is_empty());
}

#[test]
fn configured_signature_covers_profile_lists_while_legacy_helper_is_unchanged() {
    let directory = TempDir::new().unwrap();
    let mut lib =
        Library::create(&directory.path().join("signatures.gvlib"), "Signatures").unwrap();
    let id = lib
        .create_character("Werewolf", "Synthetic Werewolf", 0, ST)
        .unwrap();
    let mut request = reviewed_request(&lib, &id);
    let baseline = creation::signature_for("Werewolf", &request.sheet).unwrap();
    request.sheet.lists.get_mut("GiftList").unwrap()["Traits"] =
        json!([{"Name":"Gift","Value":"basic"}]);
    assert_ne!(
        baseline,
        creation::signature_for("Werewolf", &request.sheet).unwrap()
    );

    let vampire_id = lib
        .create_character("Vampire", "Synthetic Vampire", lib.revision().unwrap(), ST)
        .unwrap();
    let vampire = lib
        .vampire_creation_context(&vampire_id, ST)
        .unwrap()
        .request;
    assert_eq!(
        creation::signature_for("Vampire", &vampire.sheet).unwrap(),
        creation::signature(&vampire.sheet).unwrap()
    );
}

#[test]
fn configured_signature_binds_unmanaged_identity_lists_and_sharing_and_blocks_resource_disclosure()
{
    let dir = TempDir::new().unwrap();
    let mut lib = Library::create(&dir.path().join("scope.gvlib"), "Synthetic").unwrap();
    let id = lib.create_character("Wraith", "Synthetic", 0, ST).unwrap();
    let mut original = lib.sheet_edit(&id, ST).unwrap();
    original.resources.insert("Pathos".into(), 0);
    original.visibility.insert("Pathos".into(), false);
    lib.save_sheet(&id, &original, 1, ST).unwrap();
    let request = reviewed_request(&lib, &id);
    let signature = creation::signature_for("Wraith", &request.sheet).unwrap();
    let mut changed = request.sheet.clone();
    changed.identity.insert("Title".into(), "Changed".into());
    assert_ne!(
        signature,
        creation::signature_for("Wraith", &changed).unwrap()
    );
    let mut changed = request.sheet.clone();
    changed.lists.insert(
        "EquipmentList".into(),
        json!({"Name":"Equipment","Traits":[]}),
    );
    assert_ne!(
        signature,
        creation::signature_for("Wraith", &changed).unwrap()
    );
    let mut changed = request;
    changed.sheet.visibility.insert("Pathos".into(), true);
    assert_ne!(
        signature,
        creation::signature_for("Wraith", &changed.sheet).unwrap()
    );
    let quote = lib
        .quote_vampire_creation(&id, &changed, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(!quote.ready);
    assert!(
        quote
            .issues
            .iter()
            .any(|issue| issue.contains("resource visibility"))
    );
}
