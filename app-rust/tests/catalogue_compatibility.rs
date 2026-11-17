use grapevine_workbench::{
    pricing::{ClanRelation, PricingProfile},
    purchase_preview::{AnimalismPower, animalism_purchase_preview},
};
use sha2::{Digest, Sha256};
#[test]
fn legacy_preview_wire_snapshot() {
    let mut snapshots = Vec::new();
    for power in AnimalismPower::ALL.into_iter().take(16) {
        for relation in [ClanRelation::InClan, ClanRelation::OutOfClan] {
            snapshots.push(
                animalism_purchase_preview(
                    "Vampire",
                    Some(power.stable_id()),
                    Some(PricingProfile::MetLawsOfTheNightRevised),
                    Some(relation),
                )
                .unwrap(),
            );
        }
    }
    let wire = serde_json::to_vec(&snapshots).unwrap();
    let digest: String = Sha256::digest(&wire)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    // Portable wire snapshot: beta.2 replaces developer-path provenance with a logical source ID.
    // Pricing, IDs, field names and historical deserialization remain independently checked.
    assert_eq!(
        digest,
        "7b5126a8085d7f72b5db720a542762443611bd7e348dc10844943d041f11d6c4"
    );
    let restored: Vec<grapevine_workbench::purchase_preview::AnimalismPurchasePreview> =
        serde_json::from_slice(&wire).unwrap();
    assert_eq!(restored, snapshots);
}
