use grapevine_workbench::{
    power_catalogue::{self, CatalogueEntry, PowerSourceReference, ReviewMetadata, ReviewStatus},
    pricing::{ClanRelation, MetTier, PricingProfile, PurchaseFamily},
    purchase_preview::AnimalismPower,
};

fn reviewed_synthetic(tier: MetTier, template_scope: &str) -> CatalogueEntry {
    CatalogueEntry {
        stable_id: format!("synthetic.{template_scope}.{tier:?}"),
        target_list: None,
        fixed_xp_cost: None,
        name: "Synthetic fixture".into(),
        family: "Mixed family fixture".into(),
        tier,
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        template_scope: template_scope.into(),
        branch: Some("synthetic test branch".into()),
        purchase_family: PurchaseFamily::Discipline,
        source: Some(PowerSourceReference {
            profile: PricingProfile::MetLawsOfTheNightRevised,
            // Synthetic fixtures use clearly synthetic page coordinates.
            printed_page: 1,
            pdf_page_1based: 2,
        }),
        legacy_power: None,
        review: ReviewMetadata {
            source_identity: ReviewStatus::Reviewed,
            prerequisites: ReviewStatus::Unresolved,
            translation: ReviewStatus::Unreviewed,
            rights: ReviewStatus::Unreviewed,
            approval: ReviewStatus::Unresolved,
        },
    }
}

#[test]
fn catalogue_wraps_all_legacy_entries_without_changing_the_enum_wire_names() {
    let entries: Vec<_> = power_catalogue::entries()
        .into_iter()
        .filter(|entry| entry.legacy_power.is_some())
        .collect();
    assert_eq!(entries.len(), AnimalismPower::ALL.len());

    for (entry, power) in entries.iter().zip(AnimalismPower::ALL) {
        assert_eq!(entry.stable_id, power.stable_id());
        assert_eq!(entry.name, power.name());
        assert_eq!(entry.family, power.family());
        assert_eq!(entry.tier, power.tier());
        assert_eq!(entry.legacy_power, Some(power));
        let source = entry.source.unwrap();
        assert_eq!(source.profile, PricingProfile::MetLawsOfTheNightRevised);
        assert_eq!(source.printed_page, power.printed_page());
        assert_eq!(source.pdf_page_1based, power.printed_page() + 1);
        assert_eq!(entry.review.source_identity, ReviewStatus::Reviewed);
        assert_eq!(entry.review.prerequisites, ReviewStatus::Unresolved);
        assert_eq!(entry.review.translation, ReviewStatus::Unreviewed);
        assert_eq!(entry.review.rights, ReviewStatus::Unreviewed);
        assert_eq!(entry.review.approval, ReviewStatus::Unresolved);
    }

    assert_eq!(
        power_catalogue::lookup(AnimalismPower::FeralWhispers.stable_id())
            .unwrap()
            .legacy_power,
        Some(AnimalismPower::FeralWhispers)
    );
    assert!(power_catalogue::lookup("unknown.power.id").is_none());
    assert_eq!(
        serde_json::to_string(&AnimalismPower::FeralWhispers).unwrap(),
        "\"FeralWhispers\""
    );
    assert_eq!(
        serde_json::to_string(&AnimalismPower::Beckoning).unwrap(),
        "\"Beckoning\""
    );
}

#[test]
fn legacy_entry_preview_uses_shared_pricing_and_keeps_power_citation_pages() {
    let entry = power_catalogue::lookup(AnimalismPower::Beckoning.stable_id()).unwrap();
    let in_clan = entry
        .price_preview(
            "Vampire",
            Some(PricingProfile::MetLawsOfTheNightRevised),
            Some(ClanRelation::InClan),
        )
        .unwrap();
    let out_of_clan = entry
        .price_preview(
            "Vampire",
            Some(PricingProfile::MetLawsOfTheNightRevised),
            Some(ClanRelation::OutOfClan),
        )
        .unwrap();

    assert_eq!((in_clan.base.as_str(), in_clan.total.as_str()), ("3", "3"));
    assert_eq!(
        (out_of_clan.base.as_str(), out_of_clan.total.as_str()),
        ("3", "4")
    );
    assert!(!in_clan.purchase_enabled);
    let citation = entry.power_citation(&in_clan).unwrap();
    assert_eq!(
        citation.printed_page,
        AnimalismPower::Beckoning.printed_page()
    );
    assert_eq!(
        citation.pdf_page_1based,
        AnimalismPower::Beckoning.printed_page() + 1
    );
}

#[test]
fn generic_records_cover_synthetic_tiers_and_mixed_families_without_authorizing_them() {
    for (tier, expected) in [(MetTier::Intermediate, "6"), (MetTier::Advanced, "9")] {
        let entry = reviewed_synthetic(tier, "Vampire");
        let preview = entry
            .price_preview(
                "Vampire",
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(ClanRelation::InClan),
            )
            .unwrap();
        assert_eq!(preview.base, expected);
        assert!(!preview.purchase_enabled);
        assert_eq!(entry.family, "Mixed family fixture");
    }

    let risen = reviewed_synthetic(MetTier::Intermediate, "Risen");
    assert_eq!(
        risen
            .price_preview(
                "Risen",
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(ClanRelation::InClan),
            )
            .unwrap_err()
            .code,
        "UNSUPPORTED_PRICING_SCOPE"
    );
}

#[test]
fn missing_profile_and_unknown_source_review_are_rejected() {
    let entry = power_catalogue::lookup(AnimalismPower::FeralWhispers.stable_id()).unwrap();
    assert_eq!(
        entry
            .price_preview("Vampire", None, Some(ClanRelation::InClan))
            .unwrap_err()
            .code,
        "PRICING_PROFILE_REQUIRED"
    );

    let mut unknown_review = entry;
    unknown_review.review.source_identity = ReviewStatus::Unknown;
    assert_eq!(
        unknown_review
            .price_preview(
                "Vampire",
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(ClanRelation::InClan),
            )
            .unwrap_err()
            .code,
        "CATALOGUE_REVIEW_REQUIRED"
    );

    let mut unsupported_profile =
        power_catalogue::lookup(AnimalismPower::FeralWhispers.stable_id()).unwrap();
    unsupported_profile.profile = None;
    assert_eq!(
        unsupported_profile
            .price_preview(
                "Vampire",
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(ClanRelation::InClan),
            )
            .unwrap_err()
            .code,
        "UNSUPPORTED_PRICING_PROFILE"
    );
}

#[test]
fn missing_or_invalid_source_pages_cannot_be_priced_or_cited() {
    let mut entry = power_catalogue::lookup(AnimalismPower::FeralWhispers.stable_id()).unwrap();
    entry.source = None;
    assert_eq!(
        entry
            .price_preview(
                "Vampire",
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(ClanRelation::InClan),
            )
            .unwrap_err()
            .code,
        "CATALOGUE_SOURCE_REQUIRED"
    );

    let mut entry = power_catalogue::lookup(AnimalismPower::FeralWhispers.stable_id()).unwrap();
    let valid_price = entry
        .price_preview(
            "Vampire",
            Some(PricingProfile::MetLawsOfTheNightRevised),
            Some(ClanRelation::InClan),
        )
        .unwrap();
    entry.source.as_mut().unwrap().printed_page = 0;
    assert_eq!(
        entry
            .price_preview(
                "Vampire",
                Some(PricingProfile::MetLawsOfTheNightRevised),
                Some(ClanRelation::InClan),
            )
            .unwrap_err()
            .code,
        "CATALOGUE_SOURCE_REQUIRED"
    );
    assert!(entry.power_citation(&valid_price).is_none());
}
