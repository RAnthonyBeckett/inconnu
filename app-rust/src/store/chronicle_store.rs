use super::*;
use crate::chronicle::{Count, Handover, Profile, PublicProfile, PublicSnapshot, Snapshot, Source};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const SOURCE_LIMIT: i64 = 1000;
const SOURCE_PROBE_LIMIT: i64 = SOURCE_LIMIT + 1;
const CHARACTER_KINDS: &[&str] = &[
    "Vampire",
    "Werewolf",
    "Mortal",
    "Changeling",
    "Wraith",
    "Mage",
    "Fera",
    "Various",
    "Mummy",
    "KueiJin",
    "Hunter",
    "Demon",
    "Risen",
];

fn profile_error() -> AppError {
    error(
        "CHRONICLE_PROFILE_INVALID",
        "Enter a Chronicle title and keep profile text within the editor limits.",
    )
}

fn corrupt_profile() -> AppError {
    error(
        "CHRONICLE_PROFILE_CORRUPT",
        "Saved Chronicle profile data is malformed. Restore a known-good backup.",
    )
}

pub(super) fn validate_profile(profile: &Profile) -> Result<()> {
    let other_text = [
        &profile.website,
        &profile.email,
        &profile.phone,
        &profile.meeting_place,
        &profile.meeting_time,
        &profile.description,
        &profile.house_rules,
        &profile.private_notes,
    ];
    if profile.title.trim().is_empty()
        || profile.title.len() > 512
        || profile.title.contains('\0')
        || other_text
            .iter()
            .any(|value| value.len() > 16_000 || value.contains('\0'))
    {
        return Err(profile_error());
    }
    if let Some(pack) = &profile.house_rules_pack {
        pack.validate()?;
    }
    Ok(())
}

fn current_title(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT title FROM library", [], |row| row.get(0))?)
}

/// Reads and validates the schema-8 profile while retaining its original JSON
/// object for forward-compatible edits and restore handling.
///
/// If no profile row exists, the returned default profile uses the current
/// library title and the raw value is `None`.
pub(super) fn read_profile(conn: &Connection) -> Result<(Profile, Option<Value>)> {
    let title = current_title(conn)?;
    let data: Option<String> = conn
        .query_row(
            "SELECT data FROM chronicle_profile WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(data) = data else {
        let profile = Profile {
            title,
            ..Profile::default()
        };
        return Ok((profile, None));
    };

    let raw: Value = serde_json::from_str(&data).map_err(|_| corrupt_profile())?;
    if !raw.is_object() {
        return Err(corrupt_profile());
    }
    crate::legacy_profile::history(Some(&raw))?;
    let calendar_history = crate::legacy_calendar::history(Some(&raw))?;
    let playtime_reviews = crate::playtime_review::history(Some(&raw))?;
    if !calendar_history.is_empty() || !playtime_reviews.is_empty() {
        let library_id: String = conn.query_row("SELECT id FROM library", [], |r| r.get(0))?;
        if calendar_history.iter().any(|h| h.library_id != library_id) {
            return Err(error(
                "LEGACY_CALENDAR_HISTORY",
                "Private calendar review history belongs to another library. Restore a trusted backup.",
            ));
        }
        if playtime_reviews
            .iter()
            .any(|review| review.library_id != library_id)
        {
            return Err(error(
                "PLAYTIME_REVIEW_LIBRARY",
                "Saved private award review history belongs to another library.",
            ));
        }
    }
    let profile: Profile = serde_json::from_value(raw.clone()).map_err(|_| corrupt_profile())?;
    validate_profile(&profile).map_err(|_| corrupt_profile())?;
    if profile.title != title {
        return Err(error(
            "CHRONICLE_PROFILE_TITLE_MISMATCH",
            "Saved Chronicle title does not match the library title. Restore a known-good backup.",
        ));
    }
    Ok((profile, Some(raw)))
}

pub(super) fn merged_data(profile: &Profile, raw: Option<Value>) -> Result<String> {
    let mut raw = raw.unwrap_or_else(|| Value::Object(Map::new()));
    let Some(object) = raw.as_object_mut() else {
        return Err(corrupt_profile());
    };
    for (key, value) in [
        ("title", &profile.title),
        ("website", &profile.website),
        ("email", &profile.email),
        ("phone", &profile.phone),
        ("meeting_place", &profile.meeting_place),
        ("meeting_time", &profile.meeting_time),
        ("description", &profile.description),
        ("house_rules", &profile.house_rules),
        ("private_notes", &profile.private_notes),
    ] {
        object.insert(key.to_owned(), Value::String(value.clone()));
    }
    object.insert(
        "publish_details".into(),
        Value::Bool(profile.publish_details),
    );
    object.insert(
        "publish_contacts".into(),
        Value::Bool(profile.publish_contacts),
    );
    object.insert(
        "owbn_bylaws_enabled".into(),
        Value::Bool(profile.owbn_bylaws_enabled),
    );
    object.insert(
        "allow_non_binding_sources".into(),
        Value::Bool(profile.allow_non_binding_sources),
    );
    if let Some(pack) = &profile.house_rules_pack {
        object.insert("house_rules_pack".into(), serde_json::to_value(pack)?);
    } else {
        object.remove("house_rules_pack");
    }
    Ok(serde_json::to_string(&raw)?)
}

fn read_revision(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT revision FROM library", [], |row| row.get(0))?)
}

fn stale_read() -> AppError {
    error(
        "STALE_REVISION",
        "The library changed while this Chronicle view was being prepared. Refresh it.",
    )
}

fn count_rows(conn: &Connection, table: &str) -> Result<u64> {
    // `table` is always a fixed internal name at each call site.
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    u64::try_from(count).map_err(|_| {
        error(
            "CHRONICLE_HANDOVER_CORRUPT",
            "A handover count is outside the supported range.",
        )
    })
}

fn increment(counts: &mut BTreeMap<String, u64>, key: impl Into<String>) {
    let value = counts.entry(key.into()).or_default();
    *value += 1;
}

fn source_basename(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn sources(conn: &Connection, include_names: bool) -> Result<Vec<Source>> {
    let mut statement = conn.prepare(
        "SELECT hash,source_name,
           CASE WHEN json_valid(report) THEN COALESCE(
             CASE WHEN json_type(report,'$.format')='text' THEN json_extract(report,'$.format') END,
             CASE WHEN json_type(report,'$.source_format')='text' THEN json_extract(report,'$.source_format') END,
             CASE WHEN json_type(report,'$.kind')='text' THEN json_extract(report,'$.kind') END,
             'retained source')
           ELSE 'retained source' END
         FROM imports ORDER BY hash LIMIT ?1",
    )?;
    let rows = statement.query_map([SOURCE_PROBE_LIMIT], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut sources = Vec::new();
    for row in rows {
        let (hash, source_name, report) = row?;
        sources.push(Source {
            hash,
            format: report,
            name: if include_names {
                source_basename(&source_name)
            } else {
                String::new()
            },
        });
        if sources.len() > SOURCE_LIMIT as usize {
            return Err(error(
                "CHRONICLE_HANDOVER_SOURCE_LIMIT",
                "Handover is limited to 1,000 retained imports.",
            ));
        }
    }
    Ok(sources)
}

fn public_profile(profile: &Profile, markers: &[(String, String)]) -> Result<PublicProfile> {
    let mut house_rules_pack = profile.house_rules_pack.as_ref().map(|pack| pack.summary());
    if let Some(summary) = &mut house_rules_pack {
        for field in [&mut summary.name, &mut summary.version] {
            let safe = importer::redact_projection(json!(&*field), markers)?;
            *field = safe.as_str().unwrap_or_default().to_owned();
        }
    }
    Ok(PublicProfile {
        owbn_bylaws_enabled: profile.owbn_bylaws_enabled,
        allow_non_binding_sources: profile.allow_non_binding_sources,
        house_rules_pack,
        title: profile.title.clone(),
        website: if profile.publish_details {
            profile.website.clone()
        } else {
            String::new()
        },
        email: if profile.publish_contacts {
            profile.email.clone()
        } else {
            String::new()
        },
        phone: if profile.publish_contacts {
            profile.phone.clone()
        } else {
            String::new()
        },
        meeting_place: if profile.publish_details {
            profile.meeting_place.clone()
        } else {
            String::new()
        },
        meeting_time: if profile.publish_details {
            profile.meeting_time.clone()
        } else {
            String::new()
        },
        description: if profile.publish_details {
            profile.description.clone()
        } else {
            String::new()
        },
        house_rules: if profile.publish_details {
            profile.house_rules.clone()
        } else {
            String::new()
        },
    })
}

impl Library {
    pub fn chronicle_profile(&self, role: Role) -> Result<Snapshot> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let revision = read_revision(&tx)?;
        let (profile, _) = read_profile(&tx)?;
        if read_revision(&tx)? != revision {
            return Err(stale_read());
        }
        tx.commit()?;
        Ok(Snapshot { revision, profile })
    }

    pub fn public_chronicle_profile(&self) -> Result<PublicSnapshot> {
        let tx = self.conn.unchecked_transaction()?;
        let revision = read_revision(&tx)?;
        let (profile, _) = read_profile(&tx)?;
        if read_revision(&tx)? != revision {
            return Err(stale_read());
        }
        let markers = if profile.house_rules_pack.is_some() {
            self.projection_markers()?
        } else {
            Vec::new()
        };
        let profile = public_profile(&profile, &markers)?;
        tx.commit()?;
        Ok(PublicSnapshot { revision, profile })
    }

    pub fn save_chronicle_profile(
        &mut self,
        profile: &Profile,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let mut profile = profile.clone();
        profile.title = profile.title.trim().to_owned();
        validate_profile(&profile)?;

        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        let (_, raw) = read_profile(&tx)?;
        let data = merged_data(&profile, raw)?;
        if tx.execute("UPDATE library SET title=?1", [&profile.title])? != 1 {
            return Err(error(
                "CHRONICLE_LIBRARY_MISSING",
                "The library title could not be updated.",
            ));
        }
        tx.execute(
            "INSERT INTO chronicle_profile(singleton,data) VALUES(1,?1)
             ON CONFLICT(singleton) DO UPDATE SET data=excluded.data",
            [&data],
        )?;
        let revision = expected + 1;
        audit(&tx, "edit_chronicle_profile", None, revision)?;
        tx.commit()?;
        self.title = profile.title;
        Ok(revision)
    }

    pub fn chronicle_handover(
        &self,
        include_contacts: bool,
        include_notes: bool,
        include_source_names: bool,
        role: Role,
    ) -> Result<Handover> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let revision = read_revision(&tx)?;
        let (mut profile, _) = read_profile(&tx)?;
        if !include_contacts {
            profile.email.clear();
            profile.phone.clear();
        }
        if !include_notes {
            profile.private_notes.clear();
        }

        let records = self.records()?;
        let mut counts = BTreeMap::<String, u64>::new();
        counts.insert("characters_total".into(), 0);
        counts.insert("players".into(), 0);
        counts.insert("draft_copies".into(), 0);
        counts.insert("test_copies".into(), 0);
        counts.insert("items".into(), 0);
        counts.insert("locations".into(), 0);
        counts.insert("rotes".into(), 0);
        counts.insert("actions".into(), 0);
        counts.insert("rumors".into(), 0);
        counts.insert("plots".into(), 0);
        counts.insert("characters_without_local_xp_opening".into(), 0);
        for kind in CHARACTER_KINDS {
            counts.insert(format!("characters_kind:{kind}"), 0);
        }
        for record in records {
            if let Some(build) = record.build.as_ref() {
                match build.kind {
                    BuildKind::Draft => increment(&mut counts, "draft_copies"),
                    BuildKind::Test => increment(&mut counts, "test_copies"),
                }
                continue;
            }
            if record.kind == "Player" {
                increment(&mut counts, "players");
            }
            if crate::sheet::is_character_kind(&record.kind) {
                increment(&mut counts, "characters_total");
                increment(&mut counts, format!("characters_kind:{}", record.kind));
                if record.xp_ledger.is_none() {
                    increment(&mut counts, "characters_without_local_xp_opening");
                }
            }
            match record.kind.as_str() {
                "Item" => increment(&mut counts, "items"),
                "Location" => increment(&mut counts, "locations"),
                "Rote" => increment(&mut counts, "rotes"),
                "Action" => increment(&mut counts, "actions"),
                "Rumor" => increment(&mut counts, "rumors"),
                "Plot" => increment(&mut counts, "plots"),
                _ => {}
            }
        }

        counts.insert("saved_searches".into(), count_rows(&tx, "saved_searches")?);
        let scheduled = self.scheduled_sessions(role)?;
        counts.insert(
            "scheduled_plans_active".into(),
            scheduled.iter().filter(|entry| !entry.cancelled).count() as u64,
        );
        counts.insert(
            "scheduled_plans_cancelled".into(),
            scheduled.iter().filter(|entry| entry.cancelled).count() as u64,
        );
        counts.insert(
            "committed_sessions".into(),
            self.session_receipts(role)?.len() as u64,
        );
        counts.insert("imports".into(), count_rows(&tx, "imports")?);
        counts.insert("audit_rows".into(), count_rows(&tx, "audit")?);
        let sources = sources(&tx, include_source_names)?;

        if read_revision(&tx)? != revision {
            return Err(stale_read());
        }
        tx.commit()?;
        Ok(Handover {
            revision,
            profile,
            include_contacts,
            include_notes,
            include_source_names,
            counts: counts
                .into_iter()
                .map(|(key, count)| Count { key, count })
                .collect(),
            sources,
        })
    }
}
