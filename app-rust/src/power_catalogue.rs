//! Stable-ID catalogue metadata for source-reviewed Vampire Discipline powers.
//! Prices remain previews; this module does not authorize a purchase or grant a trait.
use crate::{
    model::{Result, error},
    pricing::{
        self, ClanRelation, CostPreview, MetTier, PricingProfile, PurchaseFamily, SourceCitation,
    },
    purchase_preview::AnimalismPower,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewStatus {
    Reviewed,
    Unresolved,
    Unreviewed,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewMetadata {
    /// Whether the source identity for this record was checked.
    pub source_identity: ReviewStatus,
    /// Local/global prerequisites can remain unresolved while a book price is previewed.
    pub prerequisites: ReviewStatus,
    pub translation: ReviewStatus,
    pub rights: ReviewStatus,
    pub approval: ReviewStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerSourceReference {
    /// The existing profile identifies the source edition; these pages identify the power.
    pub profile: PricingProfile,
    pub printed_page: u32,
    pub pdf_page_1based: u32,
}

/// Generic record shape for reviewed entries and synthetic catalogue fixtures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogueEntry {
    pub stable_id: String,
    pub target_list: Option<String>,
    pub fixed_xp_cost: Option<i64>,
    pub name: String,
    /// Power-family label (for example, Animalism or a synthetic mixed-family label).
    pub family: String,
    pub tier: MetTier,
    pub profile: Option<PricingProfile>,
    pub template_scope: String,
    pub branch: Option<String>,
    pub purchase_family: PurchaseFamily,
    pub source: Option<PowerSourceReference>,
    pub legacy_power: Option<AnimalismPower>,
    pub review: ReviewMetadata,
}

impl CatalogueEntry {
    /// Return a price preview only. Unresolved local prerequisites and approval remain
    /// metadata and are not converted into purchase authorization by this calculation.
    pub fn price_preview(
        &self,
        character_kind: &str,
        profile: Option<PricingProfile>,
        relation: Option<ClanRelation>,
    ) -> Result<CostPreview> {
        if self.review.source_identity != ReviewStatus::Reviewed {
            return Err(error(
                "CATALOGUE_REVIEW_REQUIRED",
                "Review the source identity before previewing this catalogue entry.",
            ));
        }
        if !crate::sheet::is_character_kind(character_kind) || character_kind != self.template_scope
        {
            return Err(error(
                "UNSUPPORTED_PRICING_SCOPE",
                "This table is a Vampire discipline preview; other templates and ritual/path-specific cases require their own reviewed rules.",
            ));
        }
        let selected_profile = profile.ok_or_else(|| {
            error(
                "PRICING_PROFILE_REQUIRED",
                "Select an applicable rules profile; no default prices are assumed.",
            )
        })?;
        if self.profile != Some(selected_profile) {
            return Err(error(
                "UNSUPPORTED_PRICING_PROFILE",
                "This catalogue entry has no reviewed price for the selected profile.",
            ));
        }
        let source = self.source.ok_or_else(|| {
            error(
                "CATALOGUE_SOURCE_REQUIRED",
                "A reviewed source page is required before previewing this catalogue entry.",
            )
        })?;
        if source.printed_page == 0 || source.pdf_page_1based == 0 {
            return Err(error(
                "CATALOGUE_SOURCE_REQUIRED",
                "A reviewed source page is required before previewing this catalogue entry.",
            ));
        }
        if source.profile != selected_profile {
            return Err(error(
                "CATALOGUE_SOURCE_PROFILE_MISMATCH",
                "The source page and requested price profile do not match.",
            ));
        }
        match selected_profile {
            PricingProfile::MetLawsOfTheNightRevised => pricing::discipline_preview(
                character_kind,
                self.purchase_family,
                Some(selected_profile),
                Some(self.tier),
                relation,
            ),
            PricingProfile::ConfiguredCreature => {
                let rules = crate::creature_profiles::profile(character_kind).ok_or_else(|| {
                    error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile.")
                })?;
                pricing::configured_power_price(
                    character_kind,
                    self.purchase_family,
                    self.tier,
                    relation.ok_or_else(|| {
                        error(
                            "PRICING_AFFILIATION_UNKNOWN",
                            "Establish affiliation before quoting.",
                        )
                    })?,
                    rules,
                    self.target_list().unwrap_or(&rules.primary_power_list),
                    self.fixed_xp_cost,
                )
            }
        }
    }

    /// Reuse the price citation's edition and document identity with this power's pages.
    pub fn power_citation(&self, price: &CostPreview) -> Option<SourceCitation> {
        let source = self.source?;
        if source.profile != price.profile
            || source.printed_page == 0
            || source.pdf_page_1based == 0
        {
            return None;
        }
        let mut citation = price.citation.clone();
        citation.printed_page = source.printed_page;
        citation.pdf_page_1based = source.pdf_page_1based;
        Some(citation)
    }
}

/// Portable JSON definition; no creature-specific Rust power variants are needed.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub stable_id: String,
    pub template_scope: String,
    pub target_list: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_xp_cost: Option<i64>,
    pub family: String,
    pub branch: Option<String>,
    pub name: String,
    pub tier: MetTier,
    pub purchase_family: PurchaseFamily,
    pub profile: PricingProfile,
    pub printed_page: u32,
    pub pdf_page_1based: u32,
    pub original_pdf_sha256: String,
    pub prerequisite_note: String,
    #[serde(default)]
    pub source_reviewed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_power: Option<AnimalismPower>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_preview: Option<crate::purchase_preview::AnimalismPurchasePreview>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub version: u32,
    pub entries: Vec<Definition>,
}
impl Catalogue {
    pub fn from_json(text: &str) -> Result<Self> {
        let invalid = || {
            error(
                "MET_CATALOGUE_INVALID",
                "MET power definitions need review; no character or XP was changed.",
            )
        };
        if text.len() > 128 * 1024 * 1024 {
            return Err(invalid());
        }
        let value: Self = serde_json::from_str(text).map_err(|_| invalid())?;
        if value.version != 1 || value.entries.len() > 100_000 {
            return Err(invalid());
        }
        let mut ids = std::collections::HashSet::new();
        for p in &value.entries {
            let profile =
                crate::creature_profiles::profile(&p.template_scope).ok_or_else(invalid)?;
            if p.stable_id.is_empty()
                || p.stable_id.len() > 256
                || !p
                    .stable_id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._:-".contains(&b))
                || !ids.insert(&p.stable_id)
                || p.fixed_xp_cost.is_some_and(|cost| {
                    !(1..=4096).contains(&cost)
                        || p.profile != PricingProfile::ConfiguredCreature
                        || p.legacy_power.is_some()
                })
                || !profile.owns_power_list(&p.target_list)
                || !crate::sheet::LIST_FIELDS.contains(&p.target_list.as_str())
                || [&p.name, &p.family].iter().any(|s| {
                    s.trim().is_empty() || s.len() > 1000 || s.chars().any(char::is_control)
                })
                || p.branch
                    .as_ref()
                    .is_some_and(|s| s.len() > 1000 || s.chars().any(char::is_control))
                || p.prerequisite_note.len() > 16000
                || (p.source_reviewed
                    && (p.printed_page == 0
                        || p.pdf_page_1based == 0
                        || p.original_pdf_sha256.len() != 64
                        || !p.original_pdf_sha256.bytes().all(|b| b.is_ascii_hexdigit())))
            {
                return Err(invalid());
            }
            if let Some(legacy) = p.legacy_power {
                if legacy.stable_id() != p.stable_id
                    || p.template_scope != "Vampire"
                    || p.target_list != "DisciplineList"
                {
                    return Err(invalid());
                }
                if p.legacy_preview.as_ref().is_none_or(|v| {
                    v.power_id != p.stable_id
                        || v.name != p.name
                        || v.tier != p.tier
                        || v.prerequisite_note != p.prerequisite_note
                }) {
                    return Err(invalid());
                }
            } else if p.legacy_preview.is_some() {
                return Err(invalid());
            }
        }
        Ok(value)
    }
    pub fn lookup(&self, id: &str) -> Option<&Definition> {
        self.entries.iter().find(|p| p.stable_id == id)
    }
}
impl Definition {
    pub fn entry(&self) -> CatalogueEntry {
        CatalogueEntry {
            stable_id: self.stable_id.clone(),
            target_list: Some(self.target_list.clone()),
            fixed_xp_cost: self.fixed_xp_cost,
            name: self.name.clone(),
            family: self.family.clone(),
            tier: self.tier,
            profile: Some(self.profile),
            template_scope: self.template_scope.clone(),
            branch: self.branch.clone(),
            purchase_family: self.purchase_family,
            source: self.source_reviewed.then_some(PowerSourceReference {
                profile: self.profile,
                printed_page: self.printed_page,
                pdf_page_1based: self.pdf_page_1based,
            }),
            legacy_power: self.legacy_power,
            review: ReviewMetadata {
                source_identity: if self.source_reviewed {
                    ReviewStatus::Reviewed
                } else {
                    ReviewStatus::Unreviewed
                },
                prerequisites: ReviewStatus::Unresolved,
                translation: ReviewStatus::Unreviewed,
                rights: ReviewStatus::Unreviewed,
                approval: ReviewStatus::Unresolved,
            },
        }
    }
}
pub fn builtin() -> &'static Catalogue {
    static VALUE: std::sync::OnceLock<Catalogue> = std::sync::OnceLock::new();
    VALUE.get_or_init(|| {
        Catalogue::from_json(include_str!("../data/met_powers.json"))
            .expect("validated bundled MET powers")
    })
}
/// Stable entry order comes from JSON. The legacy enum is only a compatibility binding.
pub fn entries() -> Vec<CatalogueEntry> {
    builtin().entries.iter().map(Definition::entry).collect()
}
pub fn lookup(stable_id: &str) -> Option<CatalogueEntry> {
    builtin().lookup(stable_id).map(Definition::entry)
}
impl CatalogueEntry {
    pub fn target_list(&self) -> Option<&str> {
        self.target_list.as_deref().or_else(|| {
            builtin()
                .lookup(&self.stable_id)
                .map(|p| p.target_list.as_str())
        })
    }
}

impl Definition {
    pub fn source_citation(&self) -> Result<SourceCitation> {
        if let Some(preview) = &self.legacy_preview {
            return Ok(preview.power_source.citation.clone());
        }
        let mut citation = crate::creature_profiles::profile(&self.template_scope)
            .and_then(|p| p.cost_source.clone())
            .filter(|s| s.original_pdf_sha256 == self.original_pdf_sha256)
            .ok_or_else(|| {
                error(
                    "CATALOGUE_SOURCE_REQUIRED",
                    "Review the creature source-book identity before mapping this power.",
                )
            })?;
        citation.printed_page = self.printed_page;
        citation.pdf_page_1based = self.pdf_page_1based;
        Ok(citation)
    }
}
