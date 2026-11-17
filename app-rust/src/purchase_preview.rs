//! Read-only source-linked previews for source-reviewed Vampire Discipline powers.
//! This module never writes XP, grants traits, or authorizes a purchase.
use crate::model::{Result, error};
use crate::pricing::{ClanRelation, CostPreview, MetTier, PricingProfile, SourceCitation};
use serde::{Deserialize, Serialize};

/// Source-scoped IDs; they do not assert identity with powers in other editions.
pub const ANIMALISM_FERAL_WHISPERS_ID: &str = "met.lotn-revised.animalism.basic.feral_whispers";
pub const ANIMALISM_BECKONING_ID: &str = "met.lotn-revised.animalism.basic.beckoning";
pub const AUSPEX_HEIGHTENED_SENSES_ID: &str = "met.lotn-revised.auspex.basic.heightened_senses";
pub const AUSPEX_AURA_PERCEPTION_ID: &str = "met.lotn-revised.auspex.basic.aura_perception";
pub const CELERITY_ALACRITY_ID: &str = "met.lotn-revised.celerity.basic.alacrity";
pub const CELERITY_SWIFTNESS_ID: &str = "met.lotn-revised.celerity.basic.swiftness";
pub const DOMINATE_COMMAND_ID: &str = "met.lotn-revised.dominate.basic.command";
pub const DOMINATE_MESMERISM_ID: &str = "met.lotn-revised.dominate.basic.mesmerism";
pub const FORTITUDE_ENDURANCE_ID: &str = "met.lotn-revised.fortitude.basic.endurance";
pub const FORTITUDE_METTLE_ID: &str = "met.lotn-revised.fortitude.basic.mettle";
pub const OBFUSCATE_CLOAK_OF_SHADOWS_ID: &str = "met.lotn-revised.obfuscate.basic.cloak_of_shadows";
pub const OBFUSCATE_UNSEEN_PRESENCE_ID: &str = "met.lotn-revised.obfuscate.basic.unseen_presence";
pub const POTENCE_PROWESS_ID: &str = "met.lotn-revised.potence.basic.prowess";
pub const POTENCE_MIGHT_ID: &str = "met.lotn-revised.potence.basic.might";
pub const PRESENCE_AWE_ID: &str = "met.lotn-revised.presence.basic.awe";
pub const PRESENCE_DREAD_GAZE_ID: &str = "met.lotn-revised.presence.basic.dread_gaze";
pub const ANIMALISM_QUELL_THE_BEAST_ID: &str =
    "met.lotn-revised.animalism.intermediate.quell_the_beast";
pub const ANIMALISM_SUBSUME_THE_SPIRIT_ID: &str =
    "met.lotn-revised.animalism.intermediate.subsume_the_spirit";
pub const ANIMALISM_DRAWING_OUT_THE_BEAST_ID: &str =
    "met.lotn-revised.animalism.advanced.drawing_out_the_beast";
pub const AUSPEX_THE_SPIRITS_TOUCH_ID: &str =
    "met.lotn-revised.auspex.intermediate.the_spirits_touch";
pub const AUSPEX_TELEPATHY_ID: &str = "met.lotn-revised.auspex.intermediate.telepathy";
pub const AUSPEX_PSYCHIC_PROJECTION_ID: &str =
    "met.lotn-revised.auspex.advanced.psychic_projection";
pub const CELERITY_RAPIDITY_ID: &str = "met.lotn-revised.celerity.intermediate.rapidity";
pub const CELERITY_LEGERITY_ID: &str = "met.lotn-revised.celerity.intermediate.legerity";
pub const CELERITY_FLEETNESS_ID: &str = "met.lotn-revised.celerity.advanced.fleetness";
pub const DOMINATE_FORGETFUL_MIND_ID: &str =
    "met.lotn-revised.dominate.intermediate.forgetful_mind";
pub const DOMINATE_CONDITIONING_ID: &str = "met.lotn-revised.dominate.intermediate.conditioning";
pub const DOMINATE_POSSESSION_ID: &str = "met.lotn-revised.dominate.advanced.possession";
pub const FORTITUDE_RESILIENCE_ID: &str = "met.lotn-revised.fortitude.intermediate.resilience";
pub const FORTITUDE_RESISTANCE_ID: &str = "met.lotn-revised.fortitude.intermediate.resistance";
pub const FORTITUDE_AEGIS_ID: &str = "met.lotn-revised.fortitude.advanced.aegis";
pub const OBFUSCATE_MASK_OF_A_THOUSAND_FACES_ID: &str =
    "met.lotn-revised.obfuscate.intermediate.mask_of_a_thousand_faces";
pub const OBFUSCATE_VANISH_FROM_THE_MINDS_EYE_ID: &str =
    "met.lotn-revised.obfuscate.intermediate.vanish_from_the_minds_eye";
pub const OBFUSCATE_CLOAK_THE_GATHERING_ID: &str =
    "met.lotn-revised.obfuscate.advanced.cloak_the_gathering";
pub const POTENCE_VIGOR_ID: &str = "met.lotn-revised.potence.intermediate.vigor";
pub const POTENCE_INTENSITY_ID: &str = "met.lotn-revised.potence.intermediate.intensity";
pub const POTENCE_PUISSANCE_ID: &str = "met.lotn-revised.potence.advanced.puissance";
pub const PRESENCE_ENTRANCEMENT_ID: &str = "met.lotn-revised.presence.intermediate.entrancement";
pub const PRESENCE_SUMMON_ID: &str = "met.lotn-revised.presence.intermediate.summon";
pub const PRESENCE_MAJESTY_ID: &str = "met.lotn-revised.presence.advanced.majesty";

/// The legacy type name and first sixteen variants remain stable for saved preview
/// compatibility. New reviewed tiers are appended without changing existing enum names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimalismPower {
    FeralWhispers,
    Beckoning,
    HeightenedSenses,
    AuraPerception,
    Alacrity,
    Swiftness,
    Command,
    Mesmerism,
    Endurance,
    Mettle,
    CloakOfShadows,
    UnseenPresence,
    Prowess,
    Might,
    Awe,
    DreadGaze,
    QuellTheBeast,
    SubsumeTheSpirit,
    DrawingOutTheBeast,
    TheSpiritsTouch,
    Telepathy,
    PsychicProjection,
    Rapidity,
    Legerity,
    Fleetness,
    ForgetfulMind,
    Conditioning,
    Possession,
    Resilience,
    Resistance,
    Aegis,
    MaskOfAThousandFaces,
    VanishFromTheMindsEye,
    CloakTheGathering,
    Vigor,
    Intensity,
    Puissance,
    Entrancement,
    Summon,
    Majesty,
}

impl AnimalismPower {
    /// Stable ordering: preserve the original sixteen Basic entries first, then append
    /// the source-reviewed Intermediate and Advanced entries. This is display order only.
    pub const ALL: [Self; 40] = [
        Self::FeralWhispers,
        Self::Beckoning,
        Self::HeightenedSenses,
        Self::AuraPerception,
        Self::Alacrity,
        Self::Swiftness,
        Self::Command,
        Self::Mesmerism,
        Self::Endurance,
        Self::Mettle,
        Self::CloakOfShadows,
        Self::UnseenPresence,
        Self::Prowess,
        Self::Might,
        Self::Awe,
        Self::DreadGaze,
        Self::QuellTheBeast,
        Self::SubsumeTheSpirit,
        Self::DrawingOutTheBeast,
        Self::TheSpiritsTouch,
        Self::Telepathy,
        Self::PsychicProjection,
        Self::Rapidity,
        Self::Legerity,
        Self::Fleetness,
        Self::ForgetfulMind,
        Self::Conditioning,
        Self::Possession,
        Self::Resilience,
        Self::Resistance,
        Self::Aegis,
        Self::MaskOfAThousandFaces,
        Self::VanishFromTheMindsEye,
        Self::CloakTheGathering,
        Self::Vigor,
        Self::Intensity,
        Self::Puissance,
        Self::Entrancement,
        Self::Summon,
        Self::Majesty,
    ];

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::FeralWhispers => ANIMALISM_FERAL_WHISPERS_ID,
            Self::Beckoning => ANIMALISM_BECKONING_ID,
            Self::HeightenedSenses => AUSPEX_HEIGHTENED_SENSES_ID,
            Self::AuraPerception => AUSPEX_AURA_PERCEPTION_ID,
            Self::Alacrity => CELERITY_ALACRITY_ID,
            Self::Swiftness => CELERITY_SWIFTNESS_ID,
            Self::Command => DOMINATE_COMMAND_ID,
            Self::Mesmerism => DOMINATE_MESMERISM_ID,
            Self::Endurance => FORTITUDE_ENDURANCE_ID,
            Self::Mettle => FORTITUDE_METTLE_ID,
            Self::CloakOfShadows => OBFUSCATE_CLOAK_OF_SHADOWS_ID,
            Self::UnseenPresence => OBFUSCATE_UNSEEN_PRESENCE_ID,
            Self::Prowess => POTENCE_PROWESS_ID,
            Self::Might => POTENCE_MIGHT_ID,
            Self::Awe => PRESENCE_AWE_ID,
            Self::DreadGaze => PRESENCE_DREAD_GAZE_ID,
            Self::QuellTheBeast => ANIMALISM_QUELL_THE_BEAST_ID,
            Self::SubsumeTheSpirit => ANIMALISM_SUBSUME_THE_SPIRIT_ID,
            Self::DrawingOutTheBeast => ANIMALISM_DRAWING_OUT_THE_BEAST_ID,
            Self::TheSpiritsTouch => AUSPEX_THE_SPIRITS_TOUCH_ID,
            Self::Telepathy => AUSPEX_TELEPATHY_ID,
            Self::PsychicProjection => AUSPEX_PSYCHIC_PROJECTION_ID,
            Self::Rapidity => CELERITY_RAPIDITY_ID,
            Self::Legerity => CELERITY_LEGERITY_ID,
            Self::Fleetness => CELERITY_FLEETNESS_ID,
            Self::ForgetfulMind => DOMINATE_FORGETFUL_MIND_ID,
            Self::Conditioning => DOMINATE_CONDITIONING_ID,
            Self::Possession => DOMINATE_POSSESSION_ID,
            Self::Resilience => FORTITUDE_RESILIENCE_ID,
            Self::Resistance => FORTITUDE_RESISTANCE_ID,
            Self::Aegis => FORTITUDE_AEGIS_ID,
            Self::MaskOfAThousandFaces => OBFUSCATE_MASK_OF_A_THOUSAND_FACES_ID,
            Self::VanishFromTheMindsEye => OBFUSCATE_VANISH_FROM_THE_MINDS_EYE_ID,
            Self::CloakTheGathering => OBFUSCATE_CLOAK_THE_GATHERING_ID,
            Self::Vigor => POTENCE_VIGOR_ID,
            Self::Intensity => POTENCE_INTENSITY_ID,
            Self::Puissance => POTENCE_PUISSANCE_ID,
            Self::Entrancement => PRESENCE_ENTRANCEMENT_ID,
            Self::Summon => PRESENCE_SUMMON_ID,
            Self::Majesty => PRESENCE_MAJESTY_ID,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::FeralWhispers => "Feral Whispers",
            Self::Beckoning => "Beckoning",
            Self::HeightenedSenses => "Heightened Senses",
            Self::AuraPerception => "Aura Perception",
            Self::Alacrity => "Alacrity",
            Self::Swiftness => "Swiftness",
            Self::Command => "Command",
            Self::Mesmerism => "Mesmerism",
            Self::Endurance => "Endurance",
            Self::Mettle => "Mettle",
            Self::CloakOfShadows => "Cloak of Shadows",
            Self::UnseenPresence => "Unseen Presence",
            Self::Prowess => "Prowess",
            Self::Might => "Might",
            Self::Awe => "Awe",
            Self::DreadGaze => "Dread Gaze",
            Self::QuellTheBeast => "Quell the Beast",
            Self::SubsumeTheSpirit => "Subsume the Spirit",
            Self::DrawingOutTheBeast => "Drawing Out the Beast",
            Self::TheSpiritsTouch => "The Spirit's Touch",
            Self::Telepathy => "Telepathy",
            Self::PsychicProjection => "Psychic Projection",
            Self::Rapidity => "Rapidity",
            Self::Legerity => "Legerity",
            Self::Fleetness => "Fleetness",
            Self::ForgetfulMind => "Forgetful Mind",
            Self::Conditioning => "Conditioning",
            Self::Possession => "Possession",
            Self::Resilience => "Resilience",
            Self::Resistance => "Resistance",
            Self::Aegis => "Aegis",
            Self::MaskOfAThousandFaces => "Mask of a Thousand Faces",
            Self::VanishFromTheMindsEye => "Vanish from the Mind's Eye",
            Self::CloakTheGathering => "Cloak the Gathering",
            Self::Vigor => "Vigor",
            Self::Intensity => "Intensity",
            Self::Puissance => "Puissance",
            Self::Entrancement => "Entrancement",
            Self::Summon => "Summon",
            Self::Majesty => "Majesty",
        }
    }

    pub const fn family(self) -> &'static str {
        match self {
            Self::FeralWhispers
            | Self::Beckoning
            | Self::QuellTheBeast
            | Self::SubsumeTheSpirit
            | Self::DrawingOutTheBeast => "Animalism",
            Self::HeightenedSenses
            | Self::AuraPerception
            | Self::TheSpiritsTouch
            | Self::Telepathy
            | Self::PsychicProjection => "Auspex",
            Self::Alacrity
            | Self::Swiftness
            | Self::Rapidity
            | Self::Legerity
            | Self::Fleetness => "Celerity",
            Self::Command
            | Self::Mesmerism
            | Self::ForgetfulMind
            | Self::Conditioning
            | Self::Possession => "Dominate",
            Self::Endurance | Self::Mettle | Self::Resilience | Self::Resistance | Self::Aegis => {
                "Fortitude"
            }
            Self::CloakOfShadows
            | Self::UnseenPresence
            | Self::MaskOfAThousandFaces
            | Self::VanishFromTheMindsEye
            | Self::CloakTheGathering => "Obfuscate",
            Self::Prowess | Self::Might | Self::Vigor | Self::Intensity | Self::Puissance => {
                "Potence"
            }
            Self::Awe | Self::DreadGaze | Self::Entrancement | Self::Summon | Self::Majesty => {
                "Presence"
            }
        }
    }

    pub const fn tier(self) -> MetTier {
        match self {
            Self::FeralWhispers
            | Self::Beckoning
            | Self::HeightenedSenses
            | Self::AuraPerception
            | Self::Alacrity
            | Self::Swiftness
            | Self::Command
            | Self::Mesmerism
            | Self::Endurance
            | Self::Mettle
            | Self::CloakOfShadows
            | Self::UnseenPresence
            | Self::Prowess
            | Self::Might
            | Self::Awe
            | Self::DreadGaze => MetTier::Basic,
            Self::QuellTheBeast
            | Self::SubsumeTheSpirit
            | Self::TheSpiritsTouch
            | Self::Telepathy
            | Self::Rapidity
            | Self::Legerity
            | Self::ForgetfulMind
            | Self::Conditioning
            | Self::Resilience
            | Self::Resistance
            | Self::MaskOfAThousandFaces
            | Self::VanishFromTheMindsEye
            | Self::Vigor
            | Self::Intensity
            | Self::Entrancement
            | Self::Summon => MetTier::Intermediate,
            Self::DrawingOutTheBeast
            | Self::PsychicProjection
            | Self::Fleetness
            | Self::Possession
            | Self::Aegis
            | Self::CloakTheGathering
            | Self::Puissance
            | Self::Majesty => MetTier::Advanced,
        }
    }

    /// Printed page containing the named power, not merely its family introduction.
    pub const fn printed_page(self) -> u32 {
        match self {
            Self::FeralWhispers | Self::Beckoning => 134,
            Self::HeightenedSenses | Self::AuraPerception => 137,
            Self::Alacrity | Self::Swiftness => 141,
            Self::Command | Self::Mesmerism => 146,
            Self::Endurance | Self::Mettle => 149,
            Self::CloakOfShadows | Self::UnseenPresence => 161,
            Self::Prowess | Self::Might => 166,
            Self::Awe | Self::DreadGaze => 167,
            Self::QuellTheBeast | Self::SubsumeTheSpirit => 135,
            Self::DrawingOutTheBeast => 136,
            Self::TheSpiritsTouch | Self::Telepathy => 138,
            Self::PsychicProjection => 139,
            Self::Rapidity | Self::Legerity | Self::Fleetness => 141,
            Self::ForgetfulMind | Self::Conditioning => 147,
            Self::Possession => 148,
            Self::Resilience | Self::Resistance => 149,
            Self::Aegis => 150,
            Self::MaskOfAThousandFaces | Self::VanishFromTheMindsEye => 162,
            Self::CloakTheGathering => 163,
            Self::Vigor | Self::Intensity => 166,
            Self::Puissance => 167,
            Self::Entrancement | Self::Summon | Self::Majesty => 168,
        }
    }

    /// One-based display position in the reviewed catalogue; never a prerequisite.
    pub const fn position(self) -> u8 {
        match self {
            Self::FeralWhispers => 1,
            Self::Beckoning => 2,
            Self::HeightenedSenses => 3,
            Self::AuraPerception => 4,
            Self::Alacrity => 5,
            Self::Swiftness => 6,
            Self::Command => 7,
            Self::Mesmerism => 8,
            Self::Endurance => 9,
            Self::Mettle => 10,
            Self::CloakOfShadows => 11,
            Self::UnseenPresence => 12,
            Self::Prowess => 13,
            Self::Might => 14,
            Self::Awe => 15,
            Self::DreadGaze => 16,
            Self::QuellTheBeast => 17,
            Self::SubsumeTheSpirit => 18,
            Self::DrawingOutTheBeast => 19,
            Self::TheSpiritsTouch => 20,
            Self::Telepathy => 21,
            Self::PsychicProjection => 22,
            Self::Rapidity => 23,
            Self::Legerity => 24,
            Self::Fleetness => 25,
            Self::ForgetfulMind => 26,
            Self::Conditioning => 27,
            Self::Possession => 28,
            Self::Resilience => 29,
            Self::Resistance => 30,
            Self::Aegis => 31,
            Self::MaskOfAThousandFaces => 32,
            Self::VanishFromTheMindsEye => 33,
            Self::CloakTheGathering => 34,
            Self::Vigor => 35,
            Self::Intensity => 36,
            Self::Puissance => 37,
            Self::Entrancement => 38,
            Self::Summon => 39,
            Self::Majesty => 40,
        }
    }

    /// Legacy API name retained for saved preview compatibility; returns catalogue order.
    pub const fn position_in_basic_animalism(self) -> u8 {
        self.position()
    }

    pub fn parse(stable_id: &str) -> Option<Self> {
        match stable_id {
            ANIMALISM_FERAL_WHISPERS_ID => Some(Self::FeralWhispers),
            ANIMALISM_BECKONING_ID => Some(Self::Beckoning),
            AUSPEX_HEIGHTENED_SENSES_ID => Some(Self::HeightenedSenses),
            AUSPEX_AURA_PERCEPTION_ID => Some(Self::AuraPerception),
            CELERITY_ALACRITY_ID => Some(Self::Alacrity),
            CELERITY_SWIFTNESS_ID => Some(Self::Swiftness),
            DOMINATE_COMMAND_ID => Some(Self::Command),
            DOMINATE_MESMERISM_ID => Some(Self::Mesmerism),
            FORTITUDE_ENDURANCE_ID => Some(Self::Endurance),
            FORTITUDE_METTLE_ID => Some(Self::Mettle),
            OBFUSCATE_CLOAK_OF_SHADOWS_ID => Some(Self::CloakOfShadows),
            OBFUSCATE_UNSEEN_PRESENCE_ID => Some(Self::UnseenPresence),
            POTENCE_PROWESS_ID => Some(Self::Prowess),
            POTENCE_MIGHT_ID => Some(Self::Might),
            PRESENCE_AWE_ID => Some(Self::Awe),
            PRESENCE_DREAD_GAZE_ID => Some(Self::DreadGaze),
            ANIMALISM_QUELL_THE_BEAST_ID => Some(Self::QuellTheBeast),
            ANIMALISM_SUBSUME_THE_SPIRIT_ID => Some(Self::SubsumeTheSpirit),
            ANIMALISM_DRAWING_OUT_THE_BEAST_ID => Some(Self::DrawingOutTheBeast),
            AUSPEX_THE_SPIRITS_TOUCH_ID => Some(Self::TheSpiritsTouch),
            AUSPEX_TELEPATHY_ID => Some(Self::Telepathy),
            AUSPEX_PSYCHIC_PROJECTION_ID => Some(Self::PsychicProjection),
            CELERITY_RAPIDITY_ID => Some(Self::Rapidity),
            CELERITY_LEGERITY_ID => Some(Self::Legerity),
            CELERITY_FLEETNESS_ID => Some(Self::Fleetness),
            DOMINATE_FORGETFUL_MIND_ID => Some(Self::ForgetfulMind),
            DOMINATE_CONDITIONING_ID => Some(Self::Conditioning),
            DOMINATE_POSSESSION_ID => Some(Self::Possession),
            FORTITUDE_RESILIENCE_ID => Some(Self::Resilience),
            FORTITUDE_RESISTANCE_ID => Some(Self::Resistance),
            FORTITUDE_AEGIS_ID => Some(Self::Aegis),
            OBFUSCATE_MASK_OF_A_THOUSAND_FACES_ID => Some(Self::MaskOfAThousandFaces),
            OBFUSCATE_VANISH_FROM_THE_MINDS_EYE_ID => Some(Self::VanishFromTheMindsEye),
            OBFUSCATE_CLOAK_THE_GATHERING_ID => Some(Self::CloakTheGathering),
            POTENCE_VIGOR_ID => Some(Self::Vigor),
            POTENCE_INTENSITY_ID => Some(Self::Intensity),
            POTENCE_PUISSANCE_ID => Some(Self::Puissance),
            PRESENCE_ENTRANCEMENT_ID => Some(Self::Entrancement),
            PRESENCE_SUMMON_ID => Some(Self::Summon),
            PRESENCE_MAJESTY_ID => Some(Self::Majesty),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedSource {
    pub citation: SourceCitation,
    pub inspected_text_path: String,
    pub inspected_text_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedPurchaseGate {
    pub id: String,
    pub explanation: String,
}

/// Old enum wire names remain exact; new catalogue identities are validated stable IDs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PowerIdentity {
    Legacy(AnimalismPower),
    Catalogue(String),
}
impl PowerIdentity {
    pub fn family(&self) -> &str {
        match self {
            Self::Legacy(p) => p.family(),
            Self::Catalogue(id) => crate::power_catalogue::builtin()
                .lookup(id)
                .map(|p| p.family.as_str())
                .unwrap_or("Unresolved"),
        }
    }
}
impl PartialEq<AnimalismPower> for PowerIdentity {
    fn eq(&self, other: &AnimalismPower) -> bool {
        matches!(self,Self::Legacy(p) if p==other)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueBinding {
    pub template_scope: String,
    pub target_list: String,
    pub family: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimalismPurchasePreview {
    pub power: PowerIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalogue_binding: Option<CatalogueBinding>,
    pub power_id: String,
    pub name: String,
    pub tier: MetTier,
    /// Legacy field name: one-based order in the full reviewed catalogue.
    /// The number is display order only, never a purchase prerequisite.
    pub position_in_basic_animalism: u8,
    pub power_source: CheckedSource,
    pub learning_source: CheckedSource,
    /// Empty when no lower-power purchase prerequisite is explicitly stated.
    /// Use interactions are described separately in `prerequisite_note`; this field does
    /// not decide global or chronicle-specific prerequisites.
    pub stated_power_prerequisites: Vec<String>,
    pub prerequisite_note: String,
    pub price: CostPreview,
    pub unresolved_gates: Vec<UnresolvedPurchaseGate>,
    pub purchase_enabled: bool,
}

/// Quote a source-reviewed Vampire Discipline power using explicit profile and
/// affiliation inputs. The legacy function name remains available; results stay
/// previews until the listed local review gates are resolved.
pub fn animalism_purchase_preview(
    character_kind: &str,
    power_id: Option<&str>,
    profile: Option<PricingProfile>,
    relation: Option<ClanRelation>,
) -> Result<AnimalismPurchasePreview> {
    let stable_id = power_id.filter(|id| !id.trim().is_empty()).ok_or_else(|| {
        error(
            "ANIMALISM_POWER_REQUIRED",
            "Select a reviewed power before previewing its cost.",
        )
    })?;
    let definition = crate::power_catalogue::builtin()
        .lookup(stable_id)
        .ok_or_else(|| {
            error(
                "UNKNOWN_ANIMALISM_POWER",
                "The source-reviewed catalogue entry is unavailable.",
            )
        })?;
    let mut preview = definition.legacy_preview.clone().ok_or_else(|| {
        error(
            "UNKNOWN_ANIMALISM_POWER",
            "Use the shared catalogue preview for this power.",
        )
    })?;
    preview.price = definition
        .entry()
        .price_preview(character_kind, profile, relation)?;
    Ok(preview)
}

/// New shared path; legacy results retain byte-for-byte serialization.
pub fn power_purchase_preview(
    character_kind: &str,
    power_id: Option<&str>,
    profile: Option<PricingProfile>,
    relation: Option<ClanRelation>,
) -> Result<AnimalismPurchasePreview> {
    let id = power_id
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| error("ANIMALISM_POWER_REQUIRED", "Select a reviewed power."))?;
    let definition = crate::power_catalogue::builtin()
        .lookup(id)
        .ok_or_else(|| error("UNKNOWN_ANIMALISM_POWER", "Unknown catalogue power."))?;
    let rules = crate::creature_profiles::profile(character_kind)
        .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
    preview_from_definition(character_kind, definition, profile, relation, rules)
}

pub fn preview_from_definition(
    character_kind: &str,
    definition: &crate::power_catalogue::Definition,
    profile: Option<PricingProfile>,
    relation: Option<ClanRelation>,
    rules: &crate::creature_profiles::Profile,
) -> Result<AnimalismPurchasePreview> {
    if definition.legacy_power.is_some() {
        return animalism_purchase_preview(
            character_kind,
            Some(&definition.stable_id),
            profile,
            relation,
        );
    }
    let id = definition.stable_id.as_str();
    if definition.template_scope != character_kind
        || !rules.owns_power_list(&definition.target_list)
        || !definition.source_reviewed
    {
        return Err(error(
            "UNSUPPORTED_PRICING_SCOPE",
            "The reviewed power/target list does not belong to this creature.",
        ));
    }
    if profile != Some(definition.profile) {
        return Err(error(
            "UNSUPPORTED_PRICING_PROFILE",
            "Choose this power's configured rules profile.",
        ));
    }
    let price = match definition.profile {
        PricingProfile::MetLawsOfTheNightRevised => {
            definition
                .entry()
                .price_preview(character_kind, profile, relation)?
        }
        PricingProfile::ConfiguredCreature => crate::pricing::configured_preview(
            character_kind,
            definition.purchase_family,
            definition.tier,
            relation.ok_or_else(|| error("PRICING_AFFILIATION_UNKNOWN", "Review affiliation."))?,
            rules,
        )?,
    };
    if definition.original_pdf_sha256 != price.citation.original_pdf_sha256 {
        return Err(error(
            "CATALOGUE_SOURCE_PROFILE_MISMATCH",
            "Power and configured book identity differ; review the source binding.",
        ));
    }
    let mut citation = price.citation.clone();
    citation.printed_page = definition.printed_page;
    citation.pdf_page_1based = definition.pdf_page_1based;
    citation.original_pdf_sha256 = definition.original_pdf_sha256.clone();
    Ok(AnimalismPurchasePreview{power:PowerIdentity::Catalogue(id.into()),catalogue_binding:Some(CatalogueBinding{template_scope:definition.template_scope.clone(),target_list:definition.target_list.clone(),family:definition.family.clone()}),power_id:id.into(),name:definition.name.clone(),tier:definition.tier,position_in_basic_animalism:0,power_source:CheckedSource{citation,inspected_text_path:format!("source:{id}"),inspected_text_sha256:definition.original_pdf_sha256.clone()},learning_source:CheckedSource{citation:price.citation.clone(),inspected_text_path:format!("source:creature-profile.{character_kind}"),inspected_text_sha256:price.citation.original_pdf_sha256.clone()},stated_power_prerequisites:vec![],prerequisite_note:definition.prerequisite_note.clone(),price,unresolved_gates:vec![UnresolvedPurchaseGate{id:"local_review".into(),explanation:"Explicit policy, affiliation, teacher, eligibility and approval declarations remain required.".into()}],purchase_enabled:false})
}

impl AnimalismPurchasePreview {
    pub fn target_list(&self) -> &str {
        self.catalogue_binding
            .as_ref()
            .map(|b| b.target_list.as_str())
            .unwrap_or("DisciplineList")
    }
    pub fn cost_family_label(&self) -> String {
        match &self.catalogue_binding {
            Some(b) => format!("{} / {}", b.family, b.target_list.trim_end_matches("List")),
            None => format!("{} / Discipline", self.power.family()),
        }
    }
    pub fn validate_scope(&self, kind: &str) -> Result<()> {
        let valid = match &self.catalogue_binding {
            None => kind == "Vampire",
            Some(b) => {
                b.template_scope == kind
                    && crate::creature_profiles::profile(kind)
                        .is_some_and(|p| p.owns_power_list(&b.target_list))
            }
        };
        if !valid {
            return Err(error(
                "PURCHASE_SCOPE",
                "The power's recorded creature/list binding does not match this character.",
            ));
        }
        Ok(())
    }
}
