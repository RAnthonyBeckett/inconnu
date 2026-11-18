use super::*;
use crate::gex_export::{self, Options, Preview, Target};

impl Library {
    /// Reads a single revision without altering original records, XP, receipts or visibility.
    pub fn gex_preview(
        &self,
        target: Target,
        character_id: Option<&str>,
        options: &Options,
        expected: i64,
        role: Role,
    ) -> Result<Preview> {
        if options.scope == crate::printing::PrintScope::PrivateStoryteller
            || target == Target::Chronicle
        {
            require_st(role)?;
        }
        let tx = self.conn.unchecked_transaction()?;
        if self.revision()? != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; preview XML again. / Biblioteca alterada; atualize a prévia XML.",
            ));
        }
        let mut skipped_builds = false;
        let records = if target == Target::Character {
            let selected = character_id.ok_or_else(|| {
                error(
                    "GEX_CHARACTER_REQUIRED",
                    "Choose a character. / Escolha um personagem.",
                )
            })?;
            let bytes: i64 = tx.query_row(
                "SELECT length(CAST(data AS BLOB)) FROM records WHERE id=?1",
                [selected],
                |r| r.get(0),
            )?;
            if bytes > 32 * 1024 * 1024 {
                return Err(error(
                    "GEX_LIMIT",
                    "The character exceeds the legacy export size limit. / O personagem excede o limite da exportação legada.",
                ));
            }
            let record = self.stored_record(selected)?;
            if !is_character(&record.kind) {
                return Err(error(
                    "GEX_CHARACTER_REQUIRED",
                    "Choose a character. / Escolha um personagem.",
                ));
            }
            vec![record]
        } else {
            let (count, bytes): (i64, i64) = tx.query_row(
                "SELECT COUNT(*),COALESCE(SUM(length(CAST(data AS BLOB))),0) FROM records",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if count > gex_export::MAX_RECORDS as i64 || bytes > 32 * 1024 * 1024 {
                return Err(error(
                    "GEX_LIMIT",
                    "The legacy export exceeds the supported size. / A exportação excede o tamanho suportado.",
                ));
            }
            self.records()?
                .into_iter()
                .filter(|r| {
                    if r.build.is_some() {
                        skipped_builds = true;
                        return false;
                    }
                    target == Target::Chronicle || is_character(&r.kind)
                })
                .collect()
        };
        let profile = if target == Target::Chronicle {
            Some(chronicle_store::read_profile(&tx)?.0)
        } else {
            None
        };
        let mut document = gex_export::serialize(&records, profile.as_ref(), options, role)?;
        if target == Target::Chronicle {
            document.warnings.push("Chronicle projection includes supported records and profile details. Retained calendar/APR sources, scheduled sessions and application-only workflows are not included; keep a backup for those records. / A projeção inclui registros suportados e dados da crônica. Fontes de calendário/APR, sessões agendadas e fluxos exclusivos do aplicativo não são incluídos; mantenha um backup desses dados.".into());
        }
        if skipped_builds {
            document
                .warnings
                .push("Build copies were excluded. / Cópias de construção foram excluídas.".into());
        }
        tx.commit()?;
        Ok(Preview {
            document,
            revision: expected,
            options: options.clone(),
            target,
            character_id: character_id.map(str::to_owned),
        })
    }

    /// Rebuilds the projection after the save dialog; cached XML is never trusted for publication.
    pub fn write_gex(&self, preview: &Preview, path: &Path, role: Role) -> Result<()> {
        let fresh = self.gex_preview(
            preview.target,
            preview.character_id.as_deref(),
            &preview.options,
            preview.revision,
            role,
        )?;
        if fresh.document.xml != preview.document.xml
            || fresh.document.warnings != preview.document.warnings
        {
            return Err(error(
                "GEX_PREVIEW_CHANGED",
                "Preview XML again before exporting. / Atualize a prévia XML antes de exportar.",
            ));
        }
        crate::safe_files::write_new(path, fresh.document.xml.as_bytes())
    }
}
