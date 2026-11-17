//! Reusable, explicitly loaded local purchase-policy choices.
//!
//! A template does not carry per-purchase approval or eligibility evidence and
//! does not establish organizational authority.
use crate::{
    model::{Result, error},
    pricing::PricingProfile,
    purchases::{PurchaseRequest, TeacherRule},
};
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;
const MAX_TEMPLATE_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyTemplate {
    pub format_version: u32,
    pub name: String,
    pub profile: PricingProfile,
    pub policy_version: String,
    pub policy_reference: String,
    pub teacher_rule: TeacherRule,
    pub standard_cost_confirmed: bool,
}

fn required(value: &str, max_len: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_len
}

impl PolicyTemplate {
    fn validate(&self) -> Result<()> {
        if self.format_version != FORMAT_VERSION {
            return Err(error(
                "POLICY_TEMPLATE_VERSION",
                "This local policy template format is not supported.",
            ));
        }
        if !required(&self.name, 200)
            || !required(&self.policy_version, 16000)
            || !required(&self.policy_reference, 16000)
        {
            return Err(error(
                "POLICY_TEMPLATE_FIELDS",
                "Enter a name of at most 200 characters and nonblank policy version and reference strings of at most 16,000 characters.",
            ));
        }
        if !matches!(
            self.profile,
            PricingProfile::MetLawsOfTheNightRevised | PricingProfile::ConfiguredCreature
        ) || (self.profile == PricingProfile::MetLawsOfTheNightRevised
            && !self.standard_cost_confirmed)
        {
            return Err(error(
                "POLICY_TEMPLATE_RULES",
                "A reusable template requires the supported Laws of the Night Revised profile and an explicit standard-cost confirmation.",
            ));
        }
        if self.profile == PricingProfile::ConfiguredCreature && !self.standard_cost_confirmed {
            return Err(error(
                "POLICY_TEMPLATE_RULES",
                "A reusable configured template requires an explicit standard-cost confirmation.",
            ));
        }
        Ok(())
    }

    pub fn from_request(name: &str, request: &PurchaseRequest) -> Result<Self> {
        if !required(name, 200)
            || !required(&request.policy_version, 16000)
            || !required(&request.policy_reference, 16000)
        {
            return Err(error(
                "POLICY_TEMPLATE_FIELDS",
                "Enter a name of at most 200 characters and nonblank policy version and reference strings of at most 16,000 characters.",
            ));
        }
        let profile = request.profile.ok_or_else(|| {
            error(
                "POLICY_TEMPLATE_SOURCE",
                "Select the supported exact source profile before saving a policy template.",
            )
        })?;
        if profile != PricingProfile::MetLawsOfTheNightRevised {
            return Err(error(
                "POLICY_TEMPLATE_RULES",
                "A reusable template requires the supported Laws of the Night Revised profile and an explicit standard-cost confirmation.",
            ));
        }
        let teacher_rule = request.teacher_rule.ok_or_else(|| {
            error(
                "POLICY_TEMPLATE_RULES",
                "Record an explicit local teacher ruling before saving a policy template.",
            )
        })?;
        if !request.standard_cost_confirmed {
            return Err(error(
                "POLICY_TEMPLATE_RULES",
                "Confirm the standard cost against the supported exact source before saving a policy template.",
            ));
        }
        let template = Self {
            format_version: FORMAT_VERSION,
            name: name.into(),
            profile,
            policy_version: request.policy_version.clone(),
            policy_reference: request.policy_reference.clone(),
            teacher_rule,
            standard_cost_confirmed: true,
        };
        template.validate()?;
        Ok(template)
    }

    /// Construct a reusable policy from the profile actually selected for this kind.
    /// Vampire keeps the original API and validation path; other kinds require a valid,
    /// source-reviewed configured profile before the declaration can be saved.
    pub fn from_request_for_kind(
        name: &str,
        kind: &str,
        request: &PurchaseRequest,
        rules: &crate::creature_profiles::Profile,
    ) -> Result<Self> {
        if kind == "Vampire" && request.profile != Some(PricingProfile::ConfiguredCreature) {
            return Self::from_request(name, request);
        }
        if !crate::sheet::is_character_kind(kind)
            || rules.kind != kind
            || request.profile != Some(PricingProfile::ConfiguredCreature)
        {
            return Err(error(
                "POLICY_TEMPLATE_SCOPE",
                "Choose the configured profile for this supported creature kind.",
            ));
        }
        rules.validate_snapshot()?;
        if !rules.rules_reviewed {
            return Err(error(
                "CREATURE_RULE_REVIEW_REQUIRED",
                "Review this creature's configured rules before saving a reusable policy.",
            ));
        }
        rules.checked_cost_source()?;
        if !required(name, 200)
            || !required(&request.policy_version, 16000)
            || !required(&request.policy_reference, 16000)
        {
            return Err(error(
                "POLICY_TEMPLATE_FIELDS",
                "Enter a name of at most 200 characters and nonblank policy version and reference strings of at most 16,000 characters.",
            ));
        }
        let teacher_rule = request.teacher_rule.ok_or_else(|| {
            error(
                "POLICY_TEMPLATE_RULES",
                "Record an explicit local teacher ruling before saving a policy template.",
            )
        })?;
        if !request.standard_cost_confirmed {
            return Err(error(
                "POLICY_TEMPLATE_RULES",
                "Confirm the standard cost against the supported exact source before saving a policy template.",
            ));
        }
        let template = Self {
            format_version: FORMAT_VERSION,
            name: name.into(),
            profile: PricingProfile::ConfiguredCreature,
            policy_version: request.policy_version.clone(),
            policy_reference: request.policy_reference.clone(),
            teacher_rule,
            standard_cost_confirmed: true,
        };
        template.validate()?;
        Ok(template)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_TEMPLATE_BYTES {
            return Err(error(
                "POLICY_TEMPLATE_TOO_LARGE",
                "A policy template file may not exceed 128 KiB.",
            ));
        }
        let template: Self = serde_json::from_slice(bytes).map_err(|_| {
            error(
                "POLICY_TEMPLATE_INVALID",
                "The policy template is malformed or contains unsupported fields.",
            )
        })?;
        template.validate()?;
        Ok(template)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        serde_json::to_vec_pretty(self).map_err(|_| {
            error(
                "POLICY_TEMPLATE_INVALID",
                "The policy template could not be encoded as JSON.",
            )
        })
    }

    pub fn apply(&self, request: &mut PurchaseRequest) -> Result<()> {
        self.validate()?;
        if self.profile != PricingProfile::MetLawsOfTheNightRevised {
            return Err(error(
                "POLICY_TEMPLATE_SCOPE",
                "Apply this configured policy with its character kind.",
            ));
        }
        self.apply_fields(request);
        Ok(())
    }

    /// Apply a previously reviewed policy to a matching character kind.
    pub fn apply_for_kind(&self, request: &mut PurchaseRequest, kind: &str) -> Result<()> {
        self.validate()?;
        let scope_matches = match self.profile {
            PricingProfile::MetLawsOfTheNightRevised => kind == "Vampire",
            PricingProfile::ConfiguredCreature => crate::sheet::is_character_kind(kind),
        };
        if !scope_matches {
            return Err(error(
                "POLICY_TEMPLATE_SCOPE",
                "This policy profile does not match the character kind.",
            ));
        }
        self.apply_fields(request);
        Ok(())
    }

    fn apply_fields(&self, request: &mut PurchaseRequest) {
        request.review_id = None;
        request.profile = Some(self.profile);
        request.policy_version = self.policy_version.clone();
        request.policy_reference = self.policy_reference.clone();
        request.teacher_rule = Some(self.teacher_rule);
        request.standard_cost_confirmed = self.standard_cost_confirmed;

        // These references describe this individual purchase and must be reviewed anew.
        request.approval_reference.clear();
        request.eligibility_evidence.clear();
        request.teacher_evidence.clear();
    }
}
