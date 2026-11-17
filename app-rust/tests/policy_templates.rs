use grapevine_workbench::{
    policy_templates::PolicyTemplate,
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
    purchases::{PurchaseRequest, TeacherRule},
};
use serde_json::Value;

fn request() -> PurchaseRequest {
    PurchaseRequest {
        review_id: None,
        id: uuid::Uuid::new_v4().to_string(),
        power_id: ANIMALISM_FERAL_WHISPERS_ID.into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(ClanRelation::InClan),
        affiliation_name: "Synthetic clan".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic next session".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic purchase reason".into(),
        policy_version: "Local policy 1".into(),
        policy_reference: "Synthetic policy source".into(),
        affiliation_evidence: "Per-purchase affiliation evidence".into(),
        approval_reference: "Per-purchase approval".into(),
        eligibility_evidence: "Per-purchase eligibility".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Per-purchase teacher evidence".into(),
        standard_cost_confirmed: true,
    }
}

fn template() -> PolicyTemplate {
    PolicyTemplate::from_request("Synthetic local policy", &request()).unwrap()
}

#[test]
fn policy_template_json_round_trips() {
    let source = template();
    let bytes = source.to_bytes().unwrap();
    let parsed = PolicyTemplate::parse(&bytes).unwrap();
    assert_eq!(parsed, source);
}

#[test]
fn malformed_missing_unknown_oversized_and_unsupported_templates_are_rejected() {
    let source = template();
    assert!(PolicyTemplate::parse(b"{").is_err());

    let mut value = serde_json::to_value(&source).unwrap();
    value.as_object_mut().unwrap().remove("policy_reference");
    assert!(PolicyTemplate::parse(&serde_json::to_vec(&value).unwrap()).is_err());

    let mut value = serde_json::to_value(&source).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unrecognized_rule".into(), Value::Bool(true));
    assert!(PolicyTemplate::parse(&serde_json::to_vec(&value).unwrap()).is_err());

    let mut value = serde_json::to_value(&source).unwrap();
    value["format_version"] = Value::from(2);
    assert!(PolicyTemplate::parse(&serde_json::to_vec(&value).unwrap()).is_err());

    let mut value = serde_json::to_value(&source).unwrap();
    value["standard_cost_confirmed"] = Value::Bool(false);
    assert!(PolicyTemplate::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(PolicyTemplate::parse(&vec![b' '; 128 * 1024 + 1]).is_err());
}

#[test]
fn creating_a_template_requires_source_ruling_and_nonblank_bounded_fields() {
    let mut source = request();
    source.profile = None;
    assert!(PolicyTemplate::from_request("Policy", &source).is_err());

    source = request();
    source.teacher_rule = None;
    assert!(PolicyTemplate::from_request("Policy", &source).is_err());

    source = request();
    source.standard_cost_confirmed = false;
    assert!(PolicyTemplate::from_request("Policy", &source).is_err());

    source = request();
    source.policy_version = " ".into();
    assert!(PolicyTemplate::from_request("Policy", &source).is_err());

    source = request();
    source.policy_reference = "x".repeat(16001);
    assert!(PolicyTemplate::from_request("Policy", &source).is_err());

    source = request();
    assert!(PolicyTemplate::from_request("", &source).is_err());
    assert!(PolicyTemplate::from_request(&"x".repeat(201), &source).is_err());

    source = request();
    source.power_id.clear();
    source.relation = None;
    assert!(PolicyTemplate::from_request("Policy", &source).is_ok());
}

#[test]
fn applying_a_template_changes_only_policy_fields_and_clears_per_purchase_evidence() {
    let mut source = request();
    source.policy_version = "Reused local version".into();
    source.policy_reference = "Reused local reference".into();
    source.teacher_rule = Some(TeacherRule::Required);
    let saved = PolicyTemplate::from_request("Reusable local policy", &source).unwrap();

    let mut target = request();
    target.profile = None;
    target.policy_version = "Old version".into();
    target.policy_reference = "Old reference".into();
    target.teacher_rule = Some(TeacherRule::NotRequiredByLocalRuling);
    target.standard_cost_confirmed = false;
    target.affiliation_name = "Target clan".into();
    target.approval_reference = "Old approval".into();
    target.eligibility_evidence = "Old eligibility".into();
    target.teacher_evidence = "Old teacher evidence".into();
    let before = target.clone();

    saved.apply(&mut target).unwrap();

    assert_eq!(target.profile, Some(saved.profile));
    assert_eq!(target.policy_version, saved.policy_version);
    assert_eq!(target.policy_reference, saved.policy_reference);
    assert_eq!(target.teacher_rule, Some(saved.teacher_rule));
    assert_eq!(
        target.standard_cost_confirmed,
        saved.standard_cost_confirmed
    );
    assert!(target.approval_reference.is_empty());
    assert!(target.eligibility_evidence.is_empty());
    assert!(target.teacher_evidence.is_empty());

    assert_eq!(target.id, before.id);
    assert_eq!(target.power_id, before.power_id);
    assert_eq!(target.relation, before.relation);
    assert_eq!(target.affiliation_name, before.affiliation_name);
    assert_eq!(target.day, before.day);
    assert_eq!(target.available_from, before.available_from);
    assert_eq!(target.next_session_reference, before.next_session_reference);
    assert_eq!(target.actor, before.actor);
    assert_eq!(target.reason, before.reason);
    assert_eq!(target.affiliation_evidence, before.affiliation_evidence);
}
