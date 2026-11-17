use super::*;
use crate::{
    action_links,
    actions::Action,
    story_work::{self, Draft, Snapshot as StorySnapshot},
};

const TIMELINE_LIMIT: usize = 500;
const TIMELINE_BYTES: usize = 8 * 1024 * 1024;

fn live_character(record: &Record) -> bool {
    crate::sheet::is_character_kind(&record.kind) && record.build.is_none()
}

fn require_live_character(record: &Record) -> Result<()> {
    if live_character(record) {
        Ok(())
    } else {
        Err(error(
            "ACTION_LINK_TARGET_REQUIRED",
            "Select a live character record. Names are not character links.",
        ))
    }
}

fn read_record(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<Record> {
    let data: Option<String> = tx
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    serde_json::from_str(&data.ok_or_else(|| error("NOT_FOUND", "Record no longer exists."))?)
        .map_err(Into::into)
}

fn read_action(record: &Record) -> Result<(Action, bool, bool)> {
    if record.kind != "Action" || record.build.is_some() {
        return Err(error(
            "ACTION_LINK_ACTION_REQUIRED",
            "Select a saved live Action record.",
        ));
    }
    match super::story_store::read_draft(record)? {
        (Draft::Action(action), legacy, retained) => Ok((action, legacy, retained)),
        (Draft::Rumor(_), _, _) => Err(error(
            "ACTION_LINK_ACTION_REQUIRED",
            "Select a saved live Action record.",
        )),
    }
}

fn action_snapshot(record: &Record, revision: i64) -> Result<StorySnapshot> {
    let (action, legacy, retained) = read_action(record)?;
    Ok(StorySnapshot {
        revision,
        draft: Draft::Action(action),
        legacy,
        retained,
    })
}

fn binding_for_target(
    tx: &rusqlite::Transaction<'_>,
    character_id: &str,
    previous: Option<&action_links::Binding>,
) -> Result<action_links::Binding> {
    if character_id.trim().is_empty() || character_id.len() > 128 || character_id.contains('\0') {
        return Err(error(
            "ACTION_LINK_TARGET_REQUIRED",
            "Select a live character record.",
        ));
    }
    match read_record(tx, character_id) {
        Ok(character) => {
            require_live_character(&character)?;
            if let Some(previous) = previous.filter(|saved| saved.character_id == character_id) {
                Ok(previous.clone())
            } else {
                Ok(action_links::Binding::new(character.id, character.name))
            }
        }
        Err(err)
            if err.code == "NOT_FOUND"
                && previous.is_some_and(|saved| saved.character_id == character_id) =>
        {
            // Keeping the same missing target is allowed; callers may also explicitly clear it.
            Ok(previous.expect("matched existing binding").clone())
        }
        Err(err) if err.code == "NOT_FOUND" => Err(error(
            "ACTION_LINK_TARGET_REQUIRED",
            "Choose an existing live character for this link.",
        )),
        Err(err) => Err(err),
    }
}

fn write_record(
    tx: &rusqlite::Transaction<'_>,
    record: &Record,
    operation: &str,
    expected: i64,
) -> Result<i64> {
    advance(tx, expected)?;
    if tx.execute(
        "UPDATE records SET name=?1,data=?2 WHERE id=?3",
        params![record.name, serde_json::to_string(record)?, record.id],
    )? != 1
    {
        return Err(error("NOT_FOUND", "The Action no longer exists."));
    }
    audit(tx, operation, Some(&record.id), expected + 1)?;
    Ok(expected + 1)
}

fn current_name(library: &Library, binding: &action_links::Binding) -> Result<Option<String>> {
    match library.stored_record(&binding.character_id) {
        Ok(record) if live_character(&record) => Ok(Some(record.name)),
        Ok(_) => Ok(None),
        Err(err) if err.code == "NOT_FOUND" => Ok(None),
        Err(err) => Err(err),
    }
}

impl Library {
    pub fn linked_action_snapshot(
        &self,
        action_id: &str,
        role: Role,
    ) -> Result<action_links::LinkedSnapshot> {
        require_st(role)?;
        let revision = self.revision()?;
        let record = self.stored_record(action_id)?;
        let mut story = action_snapshot(&record, revision)?;
        let binding = if let Some(binding) = action_links::read(&record)? {
            let current_name = current_name(self, &binding)?;
            if let Draft::Action(action) = &mut story.draft {
                action.character = current_name
                    .clone()
                    .unwrap_or_else(|| binding.label_at_binding.clone());
            }
            Some(action_links::ResolvedBinding {
                current_name,
                binding,
            })
        } else {
            None
        };
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed while reading this Action. Reopen it.",
            ));
        }
        Ok(action_links::LinkedSnapshot { story, binding })
    }

    pub fn create_linked_action(
        &mut self,
        action: &Action,
        character_id: Option<&str>,
        expected: i64,
        role: Role,
    ) -> Result<String> {
        require_st(role)?;
        crate::actions::validate(action)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let binding = character_id
            .map(|id| binding_for_target(&tx, id, None))
            .transpose()?;
        advance(&tx, expected)?;
        let id = Uuid::new_v4().to_string();
        let draft = Draft::Action(action.clone());
        let mut record = super::story_store::new_story_record(id.clone(), &draft)?;
        if let Some(binding) = binding {
            record
                .private_fields
                .insert(action_links::FIELD.into(), serde_json::to_value(binding)?);
        }
        insert(&tx, &record)?;
        audit(&tx, "create_linked_action", Some(&id), expected + 1)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn save_linked_action(
        &mut self,
        id: &str,
        action: &Action,
        character_id: Option<&str>,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        crate::actions::validate(action)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut record = read_record(&tx, id)?;
        if record.kind != "Action" || record.build.is_some() {
            return Err(error(
                "ACTION_LINK_ACTION_REQUIRED",
                "Select a saved live Action record.",
            ));
        }
        let binding = if let Some(character_id) = character_id {
            let previous = action_links::read(&record)?;
            Some(binding_for_target(&tx, character_id, previous.as_ref())?)
        } else {
            None
        };
        super::story_store::apply_draft(&mut record, &Draft::Action(action.clone()))?;
        if let Some(binding) = binding {
            record
                .private_fields
                .insert(action_links::FIELD.into(), serde_json::to_value(binding)?);
        } else {
            record.private_fields.remove(action_links::FIELD);
        }
        let revision = write_record(&tx, &record, "edit_linked_action", expected)?;
        tx.commit()?;
        Ok(revision)
    }

    pub fn action_timeline(
        &self,
        character_id: &str,
        from: &str,
        through: &str,
        role: Role,
    ) -> Result<action_links::Timeline> {
        require_st(role)?;
        story_work::validate_range(from, through)?;
        let revision = self.revision()?;
        let character = self.stored_record(character_id)?;
        require_live_character(&character)?;
        let mut rows = Vec::new();
        let mut bytes = 0usize;
        let mut undated_omitted = 0usize;
        let mut unlinked_in_library = 0usize;
        for record in self
            .records()?
            .into_iter()
            .filter(|record| record.kind == "Action" && record.build.is_none())
        {
            let Some(binding) = action_links::read(&record)? else {
                unlinked_in_library += 1;
                continue;
            };
            if binding.character_id != character_id {
                continue;
            }
            let mut story = action_snapshot(&record, revision)?;
            let Draft::Action(action) = &mut story.draft else {
                unreachable!("action_snapshot always contains an Action")
            };
            action.character.clone_from(&character.name);
            if !crate::xp::day_valid(&action.day) {
                undated_omitted += 1;
                continue;
            }
            if action.day.as_str() < from || action.day.as_str() > through {
                continue;
            }
            bytes = bytes.saturating_add(serde_json::to_vec(action)?.len());
            if rows.len() >= TIMELINE_LIMIT || bytes > TIMELINE_BYTES {
                return Err(error(
                    "ACTION_TIMELINE_LIMIT",
                    "The character's Action timeline exceeds the report limit. Narrow the dates.",
                ));
            }
            rows.push(action_links::TimelineRow {
                action_id: record.id,
                snapshot: story,
            });
        }
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed while building this Action timeline. Run it again.",
            ));
        }
        rows.sort_by(|left, right| {
            left.snapshot
                .draft
                .day()
                .cmp(right.snapshot.draft.day())
                .then(left.action_id.cmp(&right.action_id))
        });
        Ok(action_links::Timeline {
            revision,
            character_id: character.id,
            character_name: character.name,
            from: from.into(),
            through: through.into(),
            rows,
            undated_omitted,
            unlinked_in_library,
        })
    }

    pub fn preview_action_copy(
        &self,
        character_id: &str,
        source_action_id: &str,
        new_day: &str,
        options: &action_links::CopyOptions,
        role: Role,
    ) -> Result<action_links::CopyPreview> {
        require_st(role)?;
        if !crate::xp::day_valid(new_day) {
            return Err(error(
                "ACTION_COPY_DATE_INVALID",
                "Enter a valid YYYY-MM-DD date for the new Action.",
            ));
        }
        let revision = self.revision()?;
        let character = self.stored_record(character_id)?;
        require_live_character(&character)?;
        let source_record = self.stored_record(source_action_id)?;
        let (source, _, _) = read_action(&source_record)?;
        let binding = action_links::read(&source_record)?.ok_or_else(|| {
            error(
                "ACTION_COPY_SOURCE_UNLINKED",
                "Choose a source Action explicitly linked to this character.",
            )
        })?;
        if binding.character_id != character_id {
            return Err(error(
                "ACTION_COPY_SOURCE_MISMATCH",
                "The source Action is linked to a different character.",
            ));
        }
        if !crate::xp::day_valid(&source.day) {
            return Err(error(
                "ACTION_COPY_SOURCE_UNDATED",
                "The source Action has no valid date and cannot be carried forward.",
            ));
        }
        if source.day.as_str() >= new_day {
            return Err(error(
                "ACTION_COPY_DATE_ORDER",
                "The new Action date must be later than the selected source Action.",
            ));
        }
        let draft = Action {
            character: character.name.clone(),
            day: new_day.into(),
            slots: source
                .slots
                .iter()
                .map(|slot| crate::actions::ActivitySlot {
                    name: slot.name.clone(),
                    level: slot.level.clone(),
                    unused: if options.carry_unused {
                        slot.unused.clone()
                    } else {
                        "0".into()
                    },
                    total: slot.total.clone(),
                    growth: if options.carry_growth {
                        slot.growth.clone()
                    } else {
                        "0".into()
                    },
                    action: String::new(),
                    result: String::new(),
                })
                .collect(),
        };
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed while preparing this Action copy. Reopen it.",
            ));
        }
        Ok(action_links::CopyPreview {
            revision,
            character_id: character.id,
            source_id: source_action_id.into(),
            source_day: source.day,
            day: new_day.into(),
            options: options.clone(),
            draft,
        })
    }
}
