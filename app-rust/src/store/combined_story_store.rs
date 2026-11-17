use super::*;
use crate::{
    combined_story::{self, Request, RumorRow, Snapshot},
    story_work::{Draft, Snapshot as StorySnapshot},
};

const COMBINED_ROWS: usize = 500;
const COMBINED_BYTES: usize = 8 * 1024 * 1024;

fn stale_revision() -> AppError {
    error(
        "STALE_REVISION",
        "The library changed while preparing the combined story review. Run it again.",
    )
}

fn combined_limit() -> AppError {
    error(
        "COMBINED_STORY_LIMIT",
        "The combined story review exceeds its row or size limit. Narrow the dates.",
    )
}

impl Library {
    /// Combines exact-ID Action history and date-scoped Rumor snapshots for ST review.
    pub fn combined_story_snapshot(&self, request: &Request, role: Role) -> Result<Snapshot> {
        require_st(role)?;
        let revision = self.revision()?;

        let actions =
            self.action_timeline(&request.character_id, &request.from, &request.through, role)?;
        if actions.revision != revision {
            return Err(stale_revision());
        }

        let queue = self.rumor_audience_queue(&request.from, &request.through, role)?;
        if queue.revision != revision {
            return Err(stale_revision());
        }

        let mut rumors = Vec::new();
        let mut unassigned_rumors_in_range = 0usize;
        let mut serialized_bytes = serde_json::to_vec(&actions)?.len();
        for row in queue.rows {
            if row.recipient_count == 0 {
                unassigned_rumors_in_range = unassigned_rumors_in_range.saturating_add(1);
                continue;
            }

            let rumor = self.rumor_audience_snapshot(&row.rumor_id, role)?;
            if rumor.revision != revision {
                return Err(stale_revision());
            }
            if !rumor
                .recipients
                .iter()
                .any(|recipient| recipient.binding.character_id == request.character_id)
            {
                continue;
            }

            let rumor_row = RumorRow {
                record_id: rumor.rumor_id,
                snapshot: StorySnapshot {
                    revision,
                    draft: Draft::Rumor(rumor.rumor),
                    legacy: rumor.legacy,
                    retained: rumor.retained,
                },
            };
            if actions.rows.len() + rumors.len() >= COMBINED_ROWS {
                return Err(combined_limit());
            }
            serialized_bytes =
                serialized_bytes.saturating_add(serde_json::to_vec(&rumor_row)?.len());
            if serialized_bytes > COMBINED_BYTES {
                return Err(combined_limit());
            }
            rumors.push(rumor_row);
        }

        rumors.sort_by(|left, right| {
            left.snapshot
                .draft
                .day()
                .cmp(right.snapshot.draft.day())
                .then(left.record_id.cmp(&right.record_id))
        });

        if self.revision()? != revision {
            return Err(stale_revision());
        }
        let snapshot = Snapshot {
            request: combined_story::Request {
                character_id: request.character_id.clone(),
                from: request.from.clone(),
                through: request.through.clone(),
            },
            actions,
            rumors,
            undated_rumors: queue.undated_omitted,
            unassigned_rumors_in_range,
        };
        if snapshot.actions.rows.len() + snapshot.rumors.len() > COMBINED_ROWS
            || serde_json::to_vec(&snapshot)?.len() > COMBINED_BYTES
        {
            return Err(combined_limit());
        }
        Ok(snapshot)
    }
}
