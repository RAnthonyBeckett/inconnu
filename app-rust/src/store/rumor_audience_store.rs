use super::*;
use crate::{
    rumor_audience::{
        self, AudienceState, Preview, Queue, QueueRow, Recipient, ResolvedRecipient, Snapshot,
    },
    rumors::Rumor,
    story_work::{self, Draft},
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const RECIPIENT_LIMIT: usize = 500;
const SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;

fn live_character(record: &Record) -> bool {
    record.build.is_none() && crate::sheet::is_character_kind(&record.kind)
}

fn canonical_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id)
}

fn read_revision(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT revision FROM library", [], |row| row.get(0))?)
}

fn read_record(conn: &Connection, id: &str) -> Result<Record> {
    let data: Option<String> = conn
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    serde_json::from_str(&data.ok_or_else(|| error("NOT_FOUND", "Record no longer exists."))?)
        .map_err(Into::into)
}

fn maybe_read_record(conn: &Connection, id: &str) -> Result<Option<Record>> {
    let data: Option<String> = conn
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    data.map(|encoded| serde_json::from_str(&encoded).map_err(Into::into))
        .transpose()
}

fn read_rumor(record: &Record) -> Result<(Rumor, bool, bool)> {
    if record.kind != "Rumor" || record.build.is_some() {
        return Err(error(
            "RUMOR_AUDIENCE_RUMOR_REQUIRED",
            "Select a saved live Rumor.",
        ));
    }
    match super::story_store::read_draft(record)? {
        (Draft::Rumor(rumor), legacy, retained) => Ok((rumor, legacy, retained)),
        (Draft::Action(_), _, _) => Err(error(
            "RUMOR_AUDIENCE_RUMOR_REQUIRED",
            "Select a saved live Rumor.",
        )),
    }
}

fn invalid_metadata() -> AppError {
    error(
        "RUMOR_AUDIENCE_CORRUPT",
        "Saved Rumor audience metadata is malformed. Restore a known-good backup.",
    )
}

fn validate_state(state: &AudienceState) -> Result<()> {
    if state.recipients.len() > RECIPIENT_LIMIT {
        return Err(invalid_metadata());
    }
    let mut ids = BTreeSet::new();
    for recipient in &state.recipients {
        if !canonical_id(&recipient.character_id)
            || !ids.insert(recipient.character_id.as_str())
            || recipient.label_at_binding.trim().is_empty()
            || recipient.label_at_binding.len() > 4096
            || recipient.label_at_binding.contains('\0')
        {
            return Err(invalid_metadata());
        }
    }
    if serde_json::to_vec(state)?.len() > SNAPSHOT_BYTES {
        return Err(invalid_metadata());
    }
    Ok(())
}

fn read_state(record: &Record) -> Result<AudienceState> {
    let Some(value) = record.private_fields.get(rumor_audience::FIELD) else {
        return Ok(AudienceState::default());
    };
    if !value.is_object() {
        return Err(invalid_metadata());
    }
    let state: AudienceState =
        serde_json::from_value(value.clone()).map_err(|_| invalid_metadata())?;
    validate_state(&state)?;
    Ok(state)
}

fn validate_ids(ids: &[String]) -> Result<Vec<String>> {
    if ids.len() > RECIPIENT_LIMIT {
        return Err(error(
            "RUMOR_AUDIENCE_LIMIT",
            "Choose no more than 500 live character records.",
        ));
    }
    let mut seen = BTreeSet::new();
    for id in ids {
        if !canonical_id(id) || !seen.insert(id.as_str()) {
            return Err(error(
                "RUMOR_AUDIENCE_SELECTION_INVALID",
                "Choose unique saved character IDs.",
            ));
        }
    }
    let mut sorted = ids.to_vec();
    sorted.sort();
    Ok(sorted)
}

fn resolve_existing(conn: &Connection, recipient: &Recipient) -> Result<ResolvedRecipient> {
    let current_name = maybe_read_record(conn, &recipient.character_id)?
        .filter(live_character)
        .map(|record| record.name);
    Ok(ResolvedRecipient {
        binding: recipient.clone(),
        current_name,
    })
}

fn choose_recipient(
    conn: &Connection,
    character_id: &str,
    previous: Option<&Recipient>,
) -> Result<ResolvedRecipient> {
    match maybe_read_record(conn, character_id)? {
        Some(record) if live_character(&record) => {
            let binding = previous.cloned().unwrap_or_else(|| Recipient {
                character_id: character_id.to_owned(),
                label_at_binding: record.name.clone(),
                extra: BTreeMap::new(),
            });
            Ok(ResolvedRecipient {
                binding,
                current_name: Some(record.name),
            })
        }
        Some(_) | None if previous.is_some() => Ok(ResolvedRecipient {
            binding: previous.expect("guarded existing binding").clone(),
            current_name: None,
        }),
        Some(_) => Err(error(
            "RUMOR_AUDIENCE_TARGET_REQUIRED",
            "New audience selections must identify a live supported character.",
        )),
        None => Err(error(
            "RUMOR_AUDIENCE_TARGET_REQUIRED",
            "Choose an existing live character for a new audience selection.",
        )),
    }
}

fn enforce_snapshot_size<T: serde::Serialize>(snapshot: &T) -> Result<()> {
    if serde_json::to_vec(snapshot)?.len() > SNAPSHOT_BYTES {
        return Err(error(
            "RUMOR_AUDIENCE_LIMIT",
            "The Rumor audience snapshot exceeds the 8 MiB limit.",
        ));
    }
    Ok(())
}

fn prepare_preview(
    conn: &Connection,
    rumor_id: &str,
    character_ids: &[String],
    revision: i64,
) -> Result<Preview> {
    if !canonical_id(rumor_id) {
        return Err(error(
            "RUMOR_AUDIENCE_RUMOR_REQUIRED",
            "Select a saved live Rumor.",
        ));
    }
    let character_ids = validate_ids(character_ids)?;
    let record = read_record(conn, rumor_id)?;
    let (rumor, legacy, retained) = read_rumor(&record)?;
    let state = read_state(&record)?;
    let previous = state
        .recipients
        .iter()
        .map(|entry| (entry.character_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut recipients = Vec::with_capacity(character_ids.len());
    for character_id in &character_ids {
        recipients.push(choose_recipient(
            conn,
            character_id,
            previous.get(character_id.as_str()).copied(),
        )?);
    }
    let preview = Preview {
        revision,
        rumor_id: rumor_id.to_owned(),
        rumor,
        legacy,
        retained,
        recipients,
        character_ids,
    };
    enforce_snapshot_size(&preview)?;
    Ok(preview)
}

fn snapshot_from_record(conn: &Connection, record: &Record, revision: i64) -> Result<Snapshot> {
    let (rumor, legacy, retained) = read_rumor(record)?;
    let state = read_state(record)?;
    let mut recipients = state
        .recipients
        .iter()
        .map(|recipient| resolve_existing(conn, recipient))
        .collect::<Result<Vec<_>>>()?;
    recipients.sort_by(|left, right| left.binding.character_id.cmp(&right.binding.character_id));
    let snapshot = Snapshot {
        revision,
        rumor_id: record.id.clone(),
        rumor,
        legacy,
        retained,
        recipients,
    };
    enforce_snapshot_size(&snapshot)?;
    Ok(snapshot)
}

fn queue_for_range(conn: &Connection, from: &str, through: &str, revision: i64) -> Result<Queue> {
    story_work::validate_range(from, through)?;
    let mut queue = Queue {
        revision,
        from: from.to_owned(),
        through: through.to_owned(),
        undated_omitted: 0,
        rows: Vec::new(),
    };
    let mut statement = conn.prepare("SELECT data FROM records WHERE kind='Rumor' ORDER BY id")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let record: Record = serde_json::from_str(&row.get::<_, String>(0)?)?;
        if record.build.is_some() {
            continue;
        }
        let (rumor, _, _) = read_rumor(&record)?;
        let state = read_state(&record)?;
        if !crate::xp::day_valid(&rumor.day) {
            queue.undated_omitted = queue.undated_omitted.saturating_add(1);
            continue;
        }
        if rumor.day.as_str() < from || rumor.day.as_str() > through {
            continue;
        }
        if queue.rows.len() >= RECIPIENT_LIMIT {
            return Err(error(
                "RUMOR_AUDIENCE_QUEUE_LIMIT",
                "The date-scoped Rumor queue exceeds 500 rows. Narrow the dates.",
            ));
        }
        queue.rows.push(QueueRow {
            rumor_id: record.id,
            title: rumor.title,
            day: rumor.day,
            reviewed: rumor.reviewed,
            recipient_count: state.recipients.len(),
        });
        if serde_json::to_vec(&queue)?.len() > SNAPSHOT_BYTES {
            return Err(error(
                "RUMOR_AUDIENCE_QUEUE_LIMIT",
                "The date-scoped Rumor queue exceeds 8 MiB. Narrow the dates.",
            ));
        }
    }
    queue.rows.sort_by(|left, right| {
        left.day
            .cmp(&right.day)
            .then(left.rumor_id.cmp(&right.rumor_id))
    });
    enforce_snapshot_size(&queue)?;
    Ok(queue)
}

fn stale_revision() -> AppError {
    error(
        "STALE_REVISION",
        "The library changed. Refresh the Rumor audience preview.",
    )
}

impl Library {
    pub fn rumor_audience_snapshot(&self, rumor_id: &str, role: Role) -> Result<Snapshot> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let revision = read_revision(&tx)?;
        let record = read_record(&tx, rumor_id)?;
        let snapshot = snapshot_from_record(&tx, &record, revision)?;
        if read_revision(&tx)? != revision {
            return Err(stale_revision());
        }
        tx.commit()?;
        Ok(snapshot)
    }

    pub fn preview_rumor_audience(
        &self,
        rumor_id: &str,
        character_ids: &[String],
        expected_revision: i64,
        role: Role,
    ) -> Result<Preview> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let revision = read_revision(&tx)?;
        if revision != expected_revision {
            return Err(stale_revision());
        }
        let preview = prepare_preview(&tx, rumor_id, character_ids, revision)?;
        if read_revision(&tx)? != revision {
            return Err(stale_revision());
        }
        tx.commit()?;
        Ok(preview)
    }

    pub fn commit_rumor_audience(&mut self, preview: &Preview, role: Role) -> Result<i64> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if read_revision(&tx)? != preview.revision {
            return Err(stale_revision());
        }
        let fresh = prepare_preview(
            &tx,
            &preview.rumor_id,
            &preview.character_ids,
            preview.revision,
        )?;
        if fresh != *preview {
            return Err(error(
                "RUMOR_AUDIENCE_PREVIEW_CHANGED",
                "The Rumor or audience preview changed. Prepare it again.",
            ));
        }

        let mut record = read_record(&tx, &preview.rumor_id)?;
        let mut state = read_state(&record)?;
        state.recipients = fresh
            .recipients
            .iter()
            .map(|recipient| recipient.binding.clone())
            .collect();
        record
            .private_fields
            .insert(rumor_audience::FIELD.into(), serde_json::to_value(state)?);

        advance(&tx, preview.revision)?;
        if tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, record.id],
        )? != 1
        {
            return Err(error(
                "RUMOR_AUDIENCE_RUMOR_REQUIRED",
                "The saved Rumor no longer exists.",
            ));
        }
        let next = preview.revision + 1;
        audit(&tx, "edit_rumor_audience", Some(&record.id), next)?;
        tx.commit()?;
        Ok(next)
    }

    pub fn rumor_audience_queue(&self, from: &str, through: &str, role: Role) -> Result<Queue> {
        require_st(role)?;
        story_work::validate_range(from, through)?;
        let tx = self.conn.unchecked_transaction()?;
        let revision = read_revision(&tx)?;
        let queue = queue_for_range(&tx, from, through, revision)?;
        if read_revision(&tx)? != revision {
            return Err(stale_revision());
        }
        tx.commit()?;
        Ok(queue)
    }
}
