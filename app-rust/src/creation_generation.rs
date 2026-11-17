//! MET Revised Vampire creation limits from the ordinary, uncompressed Generation table.
use crate::model::{Result, error};
use crate::pricing::SourceCitation;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationLimits {
    pub generation: i64,
    pub attribute_max: i64,
    pub ability_max: i64,
    pub blood_capacity: i64,
    pub blood_per_turn: i64,
    pub starting_willpower: i64,
    pub willpower_max: i64,
}

/// Resolve a Generation Background rating without assuming a Flaw or elder exception.
pub fn from_background(points: i64) -> Result<GenerationLimits> {
    if !(0..=5).contains(&points) {
        return Err(error(
            "GENERATION_BACKGROUND_OUT_OF_RANGE",
            "Generation Background points must be between 0 and 5.",
        ));
    }
    limits(13 - points)
}

/// Return ordinary, uncompressed Generation-table limits for generations 8 through 13.
pub fn limits(generation: i64) -> Result<GenerationLimits> {
    let (
        attribute_max,
        ability_max,
        blood_capacity,
        blood_per_turn,
        starting_willpower,
        willpower_max,
    ) = match generation {
        13 => (10, 5, 10, 1, 2, 6),
        12 => (10, 5, 11, 1, 2, 8),
        11 => (11, 5, 12, 1, 4, 8),
        10 => (12, 5, 13, 1, 4, 10),
        9 => (13, 5, 14, 2, 6, 10),
        8 => (14, 5, 15, 3, 6, 12),
        _ => {
            return Err(error(
                "GENERATION_OUT_OF_RANGE",
                "This table helper covers ordinary generations 8 through 13 only.",
            ));
        }
    };
    Ok(GenerationLimits {
        generation,
        attribute_max,
        ability_max,
        blood_capacity,
        blood_per_turn,
        starting_willpower,
        willpower_max,
    })
}

fn citation(printed_page: u32, pdf_page_1based: u32) -> SourceCitation {
    SourceCitation {
        book: "Laws of the Night".into(),
        edition: "Revised".into(),
        system: "Mind’s Eye Theatre".into(),
        printed_page,
        pdf_page_1based,
        original_pdf_sha256: "f6eae9fda388c51e23bfe73338c867123655d5efa7957020857da7c370bac574"
            .into(),
    }
}

pub fn source_citations() -> [SourceCitation; 2] {
    [citation(95, 96), citation(96, 97)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_generation_rows_match() {
        for (
            generation,
            attribute_max,
            ability_max,
            blood_capacity,
            blood_per_turn,
            starting_willpower,
            willpower_max,
        ) in [
            (13, 10, 5, 10, 1, 2, 6),
            (12, 10, 5, 11, 1, 2, 8),
            (11, 11, 5, 12, 1, 4, 8),
            (10, 12, 5, 13, 1, 4, 10),
            (9, 13, 5, 14, 2, 6, 10),
            (8, 14, 5, 15, 3, 6, 12),
        ] {
            assert_eq!(
                limits(generation).unwrap(),
                GenerationLimits {
                    generation,
                    attribute_max,
                    ability_max,
                    blood_capacity,
                    blood_per_turn,
                    starting_willpower,
                    willpower_max
                }
            );
        }
    }

    #[test]
    fn background_mapping_and_bounds() {
        for (points, generation) in [(0, 13), (1, 12), (2, 11), (3, 10), (4, 9), (5, 8)] {
            assert_eq!(from_background(points).unwrap().generation, generation);
        }
        assert!(from_background(-1).is_err());
        assert!(from_background(6).is_err());
        assert!(limits(7).is_err());
        assert!(limits(14).is_err());
    }
}
