use grapevine_workbench::pricing::*;
fn quote(
    tier: Option<MetTier>,
    relation: Option<ClanRelation>,
) -> grapevine_workbench::model::Result<CostPreview> {
    discipline_preview(
        "Vampire",
        PurchaseFamily::Discipline,
        Some(PricingProfile::MetLawsOfTheNightRevised),
        tier,
        relation,
    )
}
#[test]
fn original_lotn_revised_printed124_costs_include_out_of_clan_adjustment() {
    // Expected values transcribed from the visually inspected original PDF page125.
    // This is not an OWbN adoption or prerequisite test.
    for (tier, base, out) in [
        (MetTier::Basic, "3", "4"),
        (MetTier::Intermediate, "6", "7"),
        (MetTier::Advanced, "9", "10"),
    ] {
        let inside = quote(Some(tier), Some(ClanRelation::InClan)).unwrap();
        assert_eq!(
            (
                inside.base.as_str(),
                inside.affiliation_adjustment.as_str(),
                inside.total.as_str()
            ),
            (base, "0", base)
        );
        let outside = quote(Some(tier), Some(ClanRelation::OutOfClan)).unwrap();
        assert_eq!(
            (
                outside.base.as_str(),
                outside.affiliation_adjustment.as_str(),
                outside.total.as_str()
            ),
            (base, "1", out)
        );
        assert_eq!(outside.citation.printed_page, 124);
        assert_eq!(outside.citation.pdf_page_1based, 125);
        assert_eq!(outside.citation.edition, "Revised");
        assert_eq!(outside.citation.original_pdf_sha256.len(), 64);
    }
}
#[test]
fn missing_scope_affiliation_tier_or_profile_never_receives_a_base_only_fallback() {
    assert_eq!(
        quote(Some(MetTier::Basic), None).unwrap_err().code,
        "PRICING_AFFILIATION_UNKNOWN"
    );
    assert_eq!(
        quote(None, Some(ClanRelation::InClan)).unwrap_err().code,
        "PRICING_TIER_UNKNOWN"
    );
    assert_eq!(
        discipline_preview(
            "Vampire",
            PurchaseFamily::Discipline,
            None,
            Some(MetTier::Basic),
            Some(ClanRelation::InClan)
        )
        .unwrap_err()
        .code,
        "PRICING_PROFILE_REQUIRED"
    );
    for kind in ["Werewolf", "Changeling", "Wraith", "Risen", "Mortal"] {
        assert_eq!(
            discipline_preview(
                kind,
                PurchaseFamily::Discipline,
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(MetTier::Basic),
                Some(ClanRelation::InClan)
            )
            .unwrap_err()
            .code,
            "UNSUPPORTED_PRICING_SCOPE"
        );
    }
    for family in [PurchaseFamily::Ritual, PurchaseFamily::Other] {
        assert!(
            discipline_preview(
                "Vampire",
                family,
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(MetTier::Basic),
                Some(ClanRelation::InClan)
            )
            .is_err()
        );
    }
}
#[test]
fn sourced_price_is_only_a_preview_not_a_purchase_or_reviewed_translation() {
    let preview = quote(Some(MetTier::Basic), Some(ClanRelation::OutOfClan)).unwrap();
    assert!(!preview.purchase_enabled);
    assert!(preview.reviewed_portuguese_equivalent.is_none());
    assert!(
        preview
            .library_prompt
            .contains("For more detailed info check your library for")
    );
    let roundtrip: CostPreview =
        serde_json::from_str(&serde_json::to_string(&preview).unwrap()).unwrap();
    assert_eq!(roundtrip, preview);
}
