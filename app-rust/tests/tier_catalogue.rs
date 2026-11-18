use grapevine_workbench::{
    power_catalogue,
    pricing::{ClanRelation, MetTier, PricingProfile},
    purchase_preview::{AnimalismPower, animalism_purchase_preview},
};
use serde_json::Value;
use std::collections::HashSet;

#[test]
fn full_tier_catalogue_matches_independently_captured_source_headings_and_costs() {
    let matrix: Vec<Value> = serde_json::from_str(include_str!("tier_source_matrix.json")).unwrap();
    assert_eq!(matrix.len(), 24);
    let entries: Vec<_> = power_catalogue::entries()
        .into_iter()
        .filter(|e| {
            e.template_scope == "Vampire"
                && e.profile == Some(PricingProfile::MetLawsOfTheNightRevised)
                && e.legacy_power.is_some()
        })
        .collect();
    assert_eq!(entries.len(), 40);
    assert_eq!(
        entries
            .iter()
            .map(|e| e.stable_id.as_str())
            .collect::<Vec<_>>(),
        AnimalismPower::ALL
            .iter()
            .map(|p| p.stable_id())
            .collect::<Vec<_>>()
    );
    let ids: HashSet<_> = entries
        .iter()
        .map(|entry| entry.stable_id.as_str())
        .collect();
    assert_eq!(ids.len(), 40);
    assert_eq!(
        entries.iter().filter(|e| e.tier == MetTier::Basic).count(),
        16
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.tier == MetTier::Intermediate)
            .count(),
        16
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.tier == MetTier::Advanced)
            .count(),
        8
    );
    assert!(
        AnimalismPower::ALL[..16]
            .iter()
            .all(|power| power.tier() == MetTier::Basic)
    );
    for row in matrix {
        let id = row["power_id"].as_str().unwrap();
        let entry = power_catalogue::lookup(id).unwrap();
        assert_eq!(entry.name, row["name"].as_str().unwrap());
        assert_eq!(entry.family, row["family"].as_str().unwrap());
        let (tier, base, out) = match row["tier"].as_str().unwrap() {
            "Intermediate" => (MetTier::Intermediate, "6", "7"),
            "Advanced" => (MetTier::Advanced, "9", "10"),
            _ => panic!("Invalid source tier"),
        };
        assert_eq!(entry.tier, tier);
        for (relation, total) in [(ClanRelation::InClan, base), (ClanRelation::OutOfClan, out)] {
            let q = animalism_purchase_preview(
                "Vampire",
                Some(id),
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(relation),
            )
            .unwrap();
            assert_eq!(q.tier, tier);
            assert_eq!(
                q.power_source.citation.printed_page,
                u32::try_from(row["printed_page"].as_u64().unwrap()).unwrap()
            );
            assert_eq!(
                q.power_source.citation.pdf_page_1based,
                u32::try_from(row["pdf_page"].as_u64().unwrap()).unwrap()
            );
            assert_eq!(q.price.base, base);
            assert_eq!(q.price.total, total);
            assert_eq!(q.price.citation.printed_page, 124);
            assert_eq!(q.learning_source.citation.printed_page, 133);
            assert!(!q.purchase_enabled);
            assert!(
                q.unresolved_gates
                    .iter()
                    .any(|gate| gate.id == "teacher_interpretation")
            );
            assert!(q.price.reviewed_portuguese_equivalent.is_none());
        }
    }
}
