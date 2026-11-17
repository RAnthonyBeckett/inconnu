use super::*;
use crate::story_work::{self, Draft, Kind, Master, Snapshot};

pub(super) fn read_draft(record: &Record) -> Result<(Draft, bool, bool)> {
    match (record.kind.as_str(), record.build.is_none()) {
        ("Action", true) => {
            let (d, legacy, retained) = crate::actions::read(record)?;
            Ok((Draft::Action(d), legacy, retained))
        }
        ("Rumor", true) => {
            let (d, legacy, retained) = crate::rumors::read(record)?;
            Ok((Draft::Rumor(d), legacy, retained))
        }
        _ => Err(error(
            "STORY_RECORD_REQUIRED",
            "Select a live Action or Rumor record.",
        )),
    }
}

pub(super) fn apply_draft(record: &mut Record, draft: &Draft) -> Result<()> {
    let value = match draft {
        Draft::Action(a) => {
            record.name = format!("{} · {}", a.day, a.character);
            serde_json::to_value(a)?
        }
        Draft::Rumor(r) => {
            record.name.clone_from(&r.title);
            serde_json::to_value(r)?
        }
    };
    record
        .private_fields
        .insert(draft.kind().field().into(), value);
    Ok(())
}

pub(super) fn new_story_record(id: String, draft: &Draft) -> Result<Record> {
    let mut record = Record {
        id,
        kind: draft.kind().record_kind().into(),
        name: String::new(),
        player_id: None,
        legacy_id: None,
        public_fields: Map::new(),
        private_fields: Map::new(),
        original: json!({"created_in_app":true}),
        source_sha256: String::new(),
        st_markers: None,
        build: None,
        xp_ledger: None,
    };
    apply_draft(&mut record, draft)?;
    Ok(record)
}

impl Library {
    pub fn story_snapshot(&self, id: &str, role: Role) -> Result<Snapshot> {
        require_st(role)?;
        let revision = self.revision()?;
        let (draft, legacy, retained) = read_draft(&self.stored_record(id)?)?;
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed while reading this record. Reopen it.",
            ));
        }
        Ok(Snapshot {
            revision,
            draft,
            legacy,
            retained,
        })
    }

    pub fn create_story(&mut self, draft: &Draft, expected: i64, role: Role) -> Result<String> {
        require_st(role)?;
        draft.validate()?;
        let id = Uuid::new_v4().to_string();
        let record = new_story_record(id.clone(), draft)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        insert(&tx, &record)?;
        audit(&tx, "create_story_draft", Some(&id), expected + 1)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn save_story(
        &mut self,
        id: &str,
        draft: &Draft,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        draft.validate()?;
        let mut record = self.stored_record(id)?;
        if record.kind != draft.kind().record_kind() || record.build.is_some() {
            return Err(error(
                "STORY_KIND_MISMATCH",
                "The draft type must match the saved live record.",
            ));
        }
        apply_draft(&mut record, draft)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET name=?1,data=?2 WHERE id=?3",
            params![record.name, serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "edit_story_draft", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }

    pub fn master_story(
        &self,
        kind: Kind,
        from: &str,
        through: &str,
        incomplete_only: bool,
        role: Role,
    ) -> Result<Master> {
        require_st(role)?;
        story_work::validate_range(from, through)?;
        let revision = self.revision()?;
        let mut out = Master {
            revision,
            kind,
            from: from.into(),
            through: through.into(),
            incomplete_only,
            undated_omitted: 0,
            records: Vec::new(),
        };
        {
            let mut stmt = self
                .conn
                .prepare("SELECT data FROM records WHERE kind=?1 ORDER BY id")?;
            let mut rows = stmt.query([kind.record_kind()])?;
            let mut draft_bytes = 0usize;
            while let Some(row) = rows.next()? {
                let record: Record = serde_json::from_str(&row.get::<_, String>(0)?)?;
                if record.build.is_some() {
                    continue;
                }
                let (draft, legacy, retained) = read_draft(&record)?;
                let day = draft.day();
                if !crate::xp::day_valid(day) {
                    out.undated_omitted += 1;
                    continue;
                }
                if day < from || day > through || (incomplete_only && draft.ready()) {
                    continue;
                }
                draft_bytes = draft_bytes.saturating_add(match &draft {
                    Draft::Action(a) => serde_json::to_vec(a)?.len(),
                    Draft::Rumor(r) => serde_json::to_vec(r)?.len(),
                });
                if out.records.len() >= 500 || draft_bytes > 8 * 1024 * 1024 {
                    return Err(error(
                        "STORY_REPORT_LIMIT",
                        "Too many matching drafts. Narrow the report dates.",
                    ));
                }
                out.records.push(Snapshot {
                    revision,
                    draft,
                    legacy,
                    retained,
                });
            }
        }
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed during the report. Generate it again.",
            ));
        }
        out.records.sort_by(|a, b| {
            a.draft
                .day()
                .cmp(b.draft.day())
                .then(a.draft.label().cmp(b.draft.label()))
        });
        Ok(out)
    }
}
