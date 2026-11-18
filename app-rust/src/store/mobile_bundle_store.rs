use super::*;
use crate::{
    mobile_bundle::Bundle,
    printing::{DocLanguage, Options, PrintScope},
};

impl Library {
    pub(super) fn projection_markers(&self) -> Result<Vec<(String, String)>> {
        let mut statement = self.conn.prepare(
            "SELECT DISTINCT json_extract(data, '$.st_markers') AS markers FROM records WHERE json_type(data, '$.st_markers') = 'array' ORDER BY markers LIMIT 65",
        )?;
        let values = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut markers = Vec::new();
        for value in values {
            let pair: (String, String) = serde_json::from_str(&value?)?;
            if markers.len() >= 64 || pair.0.len() > 4096 || pair.1.len() > 4096 {
                return Err(error(
                    "REDACTION_LIMIT",
                    "Private marker definitions exceed the supported projection limit.",
                ));
            }
            markers.push(pair);
        }
        Ok(markers)
    }

    /// The export always resolves its own Player view, including when launched from staff UI.
    pub fn mobile_sheet(&self, id: &str, language: DocLanguage, expected: i64) -> Result<Bundle> {
        let markers = self.projection_markers()?;
        let profile = self.public_chronicle_profile()?;
        if profile.revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Reload the sheet before exporting.",
            ));
        }
        let projection = self.print_projection(
            id,
            &Options {
                language,
                scope: PrintScope::Public,
                include_notes: false,
                include_temporary: false,
                include_references: true,
                include_simple_mechanics: true,
                include_challenge_math: true,
                ..Options::default()
            },
            expected,
            Role::Player,
            &markers,
        )?;
        let mechanics = projection.document.sections[projection.mechanics_range].to_vec();
        Bundle::project(
            &projection.view,
            projection.document,
            mechanics,
            profile.profile,
            &markers,
        )
    }

    pub fn write_mobile_sheet(
        &self,
        id: &str,
        language: DocLanguage,
        expected: i64,
        path: &Path,
    ) -> Result<()> {
        let bytes = self.mobile_sheet(id, language, expected)?.encode()?;
        if self.revision()? != expected {
            return Err(error(
                "STALE_REVISION",
                "Reload the sheet before exporting.",
            ));
        }
        crate::safe_files::write_new(path, &bytes)
    }
}
