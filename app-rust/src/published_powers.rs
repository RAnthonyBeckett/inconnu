//! Edition-specific published names and levels. No MET tiers, prices or grants.
use crate::{
    model::{Result, error},
    owbn_bylaws::{PacketAuthority, TierMappingBasis},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, sync::OnceLock};

const MAX_EDITION_TAG_LEN: usize = 48;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Edition {
    bytes: [u8; MAX_EDITION_TAG_LEN],
    len: u8,
}

impl Edition {
    pub const V20: Self = Self::builtin("V20");
    pub const DA20: Self = Self::builtin("DA20");

    const fn builtin(tag: &str) -> Self {
        let source = tag.as_bytes();
        let mut bytes = [0; MAX_EDITION_TAG_LEN];
        let mut index = 0;
        while index < source.len() {
            bytes[index] = source[index];
            index += 1;
        }
        Self {
            bytes,
            len: source.len() as u8,
        }
    }

    /// Creates an edition tag containing 1 to 48 ASCII letters, digits, or underscores.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let source = tag.as_bytes();
        if source.is_empty()
            || source.len() > MAX_EDITION_TAG_LEN
            || !source
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            return None;
        }
        let mut bytes = [0; MAX_EDITION_TAG_LEN];
        bytes[..source.len()].copy_from_slice(source);
        Some(Self {
            bytes,
            len: source.len() as u8,
        })
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize])
            .expect("edition tags contain only validated ASCII")
    }
}

impl std::fmt::Debug for Edition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for Edition {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Edition {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct EditionVisitor;

        impl<'de> serde::de::Visitor<'de> for EditionVisitor {
            type Value = Edition;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .write_str("an edition tag of 1 to 48 ASCII letters, digits, or underscores")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Edition::from_tag(value).ok_or_else(|| E::custom("invalid edition tag"))
            }
        }

        deserializer.deserialize_str(EditionVisitor)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Coverage {
    Partial,
    Pending,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Discipline,
    Path,
    Gift,
    Rite,
    Arcanos,
    Thorn,
    Art,
    Realm,
    Sphere,
    Rote,
    Edge,
    Lore,
    Hekau,
    Spell,
    Ritual,
    Numina,
    Combination,
    Power,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    PageText,
    IndexOnly,
    LegacyMenu,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_scope: Option<String>,
    pub id: String,
    pub title: String,
    pub edition: Edition,
    pub pdf_sha256: String,
    pub coverage: Coverage,
    pub notes: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Power {
    #[serde(default, skip_serializing_if = "PacketAuthority::is_unknown")]
    pub packet_authority: PacketAuthority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier_mapping_basis: Option<TierMappingBasis>,
    pub id: String,
    pub book_id: String,
    pub name: String,
    pub discipline: String,
    pub path: Option<String>,
    pub kind: Kind,
    pub level: u8,
    pub printed_page: Option<u32>,
    pub pdf_page: u32,
    pub evidence: Evidence,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub version: u32,
    pub books: Vec<Book>,
    pub entries: Vec<Power>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub power: Power,
    pub book_id: String,
    pub book_title: String,
    pub edition: Edition,
    pub pdf_sha256: String,
    pub facts_hash: String,
}
impl Power {
    pub fn effective_authority(&self) -> PacketAuthority {
        if self.packet_authority.is_unknown()
            && matches!(self.book_id.as_str(), "v20-core" | "da20-core")
        {
            PacketAuthority::NonBindingOrBook
        } else {
            self.packet_authority
        }
    }
}
fn invalid() -> crate::model::AppError {
    error(
        "PUBLISHED_CATALOGUE_INVALID",
        "Published-source catalogue needs review; no character data was changed.",
    )
}
fn text_ok(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn id_ok(value: &str) -> bool {
    text_ok(value, 256)
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.:".contains(&b))
}
fn sha_ok(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Catalogue {
    pub fn from_json(source: &str) -> Result<Self> {
        if source.len() > 128 * 1024 * 1024 {
            return Err(invalid());
        }
        let catalogue: Self = serde_json::from_str(source).map_err(|_| invalid())?;
        if catalogue.version != 1
            || catalogue.books.is_empty()
            || catalogue.books.len() > 128
            || catalogue.entries.len() > 100_000
        {
            return Err(invalid());
        }
        let mut book_ids = HashSet::new();
        for book in &catalogue.books {
            if book
                .template_scope
                .as_ref()
                .is_some_and(|kind| !crate::sheet::is_character_kind(kind))
            {
                return Err(invalid());
            }
            if !id_ok(&book.id)
                || !text_ok(&book.title, 512)
                || !sha_ok(&book.pdf_sha256)
                || !book_ids.insert(&book.id)
                || book.notes.len() > 16
                || book.notes.iter().any(|note| !text_ok(note, 1024))
            {
                return Err(invalid());
            }
        }
        let mut ids = HashSet::new();
        for power in &catalogue.entries {
            let book = catalogue.book(&power.book_id).ok_or_else(invalid)?;
            if !id_ok(&power.id)
                || !(power.id.starts_with(&format!("{}.", book.id))
                    || power.id.starts_with(&format!("{}:", book.id)))
                || !ids.insert(&power.id)
                || book.coverage == Coverage::Pending
                || !text_ok(&power.name, 256)
                || !text_ok(&power.discipline, 128)
                || !(1..=9).contains(&power.level)
                || !(1..=10000).contains(&power.pdf_page)
                || power
                    .printed_page
                    .is_some_and(|page| !(1..=10000).contains(&page))
                || power.path.as_ref().is_some_and(|path| !text_ok(path, 256))
                || (power.kind == Kind::Path && power.path.is_none())
            {
                return Err(invalid());
            }
        }
        Ok(catalogue)
    }
    pub fn book(&self, id: &str) -> Option<&Book> {
        self.books.iter().find(|book| book.id == id)
    }
    pub fn power(&self, id: &str) -> Option<&Power> {
        self.entries.iter().find(|power| power.id == id)
    }
    pub fn search(&self, book_id: Option<&str>, query: &str) -> Vec<&Power> {
        let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
        self.entries
            .iter()
            .filter(|power| book_id.is_none_or(|id| power.book_id == id))
            .filter(|power| {
                let haystack = format!(
                    "{} {} {}",
                    power.name,
                    power.discipline,
                    power.path.as_deref().unwrap_or("")
                )
                .to_lowercase();
                words.iter().all(|word| haystack.contains(word))
            })
            .collect()
    }
    pub fn reference(&self, id: &str) -> Result<Reference> {
        let power = self.power(id).ok_or_else(|| {
            error(
                "PUBLISHED_POWER_UNKNOWN",
                "Choose an exact published-source identity.",
            )
        })?;
        let book = self.book(&power.book_id).ok_or_else(invalid)?;
        // Evidence/coverage may improve without invalidating unchanged source facts.
        let facts = (
            &book.id,
            &book.title,
            book.edition,
            &book.pdf_sha256,
            &power.id,
            &power.name,
            &power.discipline,
            &power.path,
            power.kind,
            power.level,
            power.printed_page,
            power.pdf_page,
        );
        let facts_hash = Sha256::digest(serde_json::to_vec(&facts)?)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Reference {
            power: power.clone(),
            book_id: book.id.clone(),
            book_title: book.title.clone(),
            edition: book.edition,
            pdf_sha256: book.pdf_sha256.clone(),
            facts_hash,
        })
    }
}

pub fn catalogue() -> Result<&'static Catalogue> {
    static CACHE: OnceLock<std::result::Result<Catalogue, String>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            Catalogue::from_json(include_str!("../data/published_powers.json"))
                .map_err(|_| "invalid".into())
        })
        .as_ref()
        .map_err(|_| invalid())
}

impl Book {
    pub fn applies_to(&self, kind: &str) -> bool {
        match &self.template_scope {
            Some(scope) => scope == kind,
            None => kind == "Vampire" && [Edition::V20, Edition::DA20].contains(&self.edition),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn source() -> serde_json::Value {
        json!({"version":1,"books":[{"id":"v20-core","title":"Synthetic V20","edition":"V20","pdf_sha256":"a".repeat(64),"coverage":"partial","notes":["Synthetic test data"]}],"entries":[{"id":"v20-core.discipline.example.3","book_id":"v20-core","name":"Example","discipline":"Animalism","path":null,"kind":"discipline","level":3,"printed_page":129,"pdf_page":138,"evidence":"index_only"}]})
    }
    #[test]
    fn published_level_and_dual_pages_do_not_become_met_costs() {
        let c = Catalogue::from_json(&source().to_string()).unwrap();
        let reference = c.reference("v20-core.discipline.example.3").unwrap();
        assert_eq!(reference.power.level, 3);
        assert_eq!(reference.power.printed_page, Some(129));
        assert_eq!(reference.power.pdf_page, 138);
        let text = serde_json::to_string(&reference).unwrap();
        assert!(!text.contains("tier"));
        assert!(!text.contains("cost"));
        assert!(!text.contains("body"));
        assert_eq!(c.search(None, "EXAMPLE animalism").len(), 1);
        assert!(c.search(Some("da20-core"), "Example").is_empty());
    }
    #[test]
    fn wrong_books_invalid_levels_duplicates_and_extra_prose_are_rejected() {
        let base = source();
        for field in ["tier", "price", "body", "source_file"] {
            let mut s = base.clone();
            s["entries"][0][field] = json!("unexpected");
            assert!(Catalogue::from_json(&s.to_string()).is_err());
        }
        for level in [0, 10] {
            let mut s = base.clone();
            s["entries"][0]["level"] = json!(level);
            assert!(Catalogue::from_json(&s.to_string()).is_err());
        }
        let mut s = base.clone();
        s["entries"][0]["book_id"] = json!("da20-core");
        assert!(Catalogue::from_json(&s.to_string()).is_err());
        let mut s = base;
        let duplicate = s["entries"][0].clone();
        s["entries"].as_array_mut().unwrap().push(duplicate);
        assert!(Catalogue::from_json(&s.to_string()).is_err());
    }
    #[test]
    fn changing_source_facts_invalidates_identity_but_coverage_notes_do_not() {
        let mut s = source();
        let original = Catalogue::from_json(&s.to_string())
            .unwrap()
            .reference("v20-core.discipline.example.3")
            .unwrap();
        s["books"][0]["notes"] = json!(["More source pages checked"]);
        assert_eq!(
            Catalogue::from_json(&s.to_string())
                .unwrap()
                .reference(&original.power.id)
                .unwrap()
                .facts_hash,
            original.facts_hash
        );
        s["entries"][0]["level"] = json!(4);
        assert_ne!(
            Catalogue::from_json(&s.to_string())
                .unwrap()
                .reference(&original.power.id)
                .unwrap()
                .facts_hash,
            original.facts_hash
        );
    }
}
