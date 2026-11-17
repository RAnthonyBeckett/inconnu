use super::chronicle_store::{merged_data, read_profile};
use super::*;
use crate::{legacy_calendar as calendar, legacy_profile::hash};
fn context(conn: &Connection, source_hash: &str) -> Result<calendar::Context> {
    let (library_id, revision): (String, i64) =
        conn.query_row("SELECT id,revision FROM library", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    let (name, data, bytes): (String, String, Vec<u8>) = conn.query_row(
        "SELECT source_name,report,source_bytes FROM imports WHERE hash=?1",
        [source_hash],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    use sha2::{Digest, Sha256};
    let actual = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if actual != source_hash {
        return Err(error(
            "LEGACY_CALENDAR_SOURCE_CHANGED",
            "Retained source bytes no longer match their import hash. Review a trusted copy.",
        ));
    }
    let report: Value = serde_json::from_str(&data).map_err(|_| {
        error(
            "LEGACY_CALENDAR_SOURCE",
            "Retained calendar metadata is malformed; original evidence is preserved.",
        )
    })?;
    let (entries, source_metadata_hash) = calendar::candidates(&report)?;
    let (profile, raw) = read_profile(conn)?;
    let history = calendar::history(raw.as_ref())?;
    let mut statement=conn.prepare("SELECT json_extract(data,'$.request.day') FROM scheduled_sessions WHERE json_extract(data,'$.cancelled')=0 UNION ALL SELECT json_extract(data,'$.quote.request.day') FROM game_sessions LIMIT 10001")?;
    let existing_days = statement
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if existing_days.len() > 10000 {
        return Err(error(
            "LEGACY_CALENDAR_LIMIT",
            "Too many calendar destinations for one bounded source review.",
        ));
    }
    Ok(calendar::Context {
        library_id,
        revision,
        source_hash: source_hash.into(),
        source_name: name,
        source_metadata_hash,
        profile_hash: hash(&(&profile, &raw))?,
        entries,
        existing_days,
        history,
    })
}
impl Library {
    pub fn legacy_calendar_sources(&self, role: Role) -> Result<Vec<calendar::Source>> {
        require_st(role)?;
        let mut statement = self.conn.prepare(
            "SELECT hash,source_name,report FROM imports ORDER BY source_name LIMIT 1001",
        )?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if rows.len() > 1000 {
            return Err(error(
                "LEGACY_CALENDAR_LIMIT",
                "Too many retained sources for one bounded calendar review.",
            ));
        }
        let mut sources = vec![];
        for (hash, name, data) in rows {
            let report: Value = serde_json::from_str(&data).map_err(|_| {
                error(
                    "LEGACY_CALENDAR_SOURCE",
                    "Retained source report is malformed.",
                )
            })?;
            if report
                .get("metadata")
                .is_none_or(|m| m.get("Calendar").is_none() && m.get("calendar").is_none())
            {
                continue;
            }
            let (entries, issue) = match calendar::candidates(&report) {
                Ok((entries, _)) => (entries.len(), None),
                Err(e) => (0, Some(e.message)),
            };
            sources.push(calendar::Source {
                hash,
                name,
                entries,
                issue,
            });
        }
        Ok(sources)
    }
    pub fn legacy_calendar_origin(
        &self,
        plan_id: &str,
        role: Role,
    ) -> Result<Option<calendar::Origin>> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let (_, raw) = read_profile(&tx)?;
        let history = calendar::history(raw.as_ref())?;
        let found = history.iter().find_map(|receipt| {
            receipt
                .plans
                .iter()
                .find(|plan| plan.request.id == plan_id)
                .map(|plan| (receipt, plan))
        });
        let origin = if let Some((receipt, plan)) = found {
            let c = context(&tx, &receipt.request.source_hash)?;
            if c.source_metadata_hash != receipt.source_metadata_hash {
                return Err(error(
                    "LEGACY_CALENDAR_SOURCE_CHANGED",
                    "Retained source details changed after this calendar import. Review a trusted copy.",
                ));
            }
            let entry = c
                .entries
                .into_iter()
                .find(|entry| entry.index == plan.entry_index)
                .ok_or_else(|| {
                    error(
                        "LEGACY_CALENDAR_SOURCE_CHANGED",
                        "The retained source entry is no longer available.",
                    )
                })?;
            Some(calendar::Origin {
                source_name: c.source_name,
                entry,
                imported_day: plan.request.day.clone(),
                imported_title: plan.request.title.clone(),
            })
        } else {
            None
        };
        tx.commit()?;
        Ok(origin)
    }
    pub fn legacy_calendar_context(
        &self,
        source_hash: &str,
        role: Role,
    ) -> Result<calendar::Context> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let c = context(&tx, source_hash)?;
        tx.commit()?;
        Ok(c)
    }
    pub fn quote_legacy_calendar(
        &self,
        request: &calendar::Request,
        expected: i64,
        role: Role,
    ) -> Result<calendar::Quote> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let c = context(&tx, &request.source_hash)?;
        if c.revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; review calendar entries again.",
            ));
        }
        let q = calendar::quote(&c, request)?;
        tx.commit()?;
        Ok(q)
    }
    pub fn commit_legacy_calendar(
        &mut self,
        q: &calendar::Quote,
        role: Role,
    ) -> Result<calendar::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let library_id: String = tx.query_row("SELECT id FROM library", [], |r| r.get(0))?;
        if q.library_id != library_id {
            return Err(error(
                "LEGACY_CALENDAR_LIBRARY",
                "This calendar review belongs to another library.",
            ));
        }
        let (profile, raw) = read_profile(&tx)?;
        let mut history = calendar::history(raw.as_ref())?;
        if let Some(saved) = history.iter().find(|r| r.request.id == q.request.id) {
            if saved.quote_hash != hash(q)? {
                return Err(error(
                    "LEGACY_CALENDAR_ID",
                    "This calendar review ID already identifies another operation.",
                ));
            }
            let saved = saved.clone();
            tx.commit()?;
            self.title = profile.title;
            return Ok(saved);
        }
        let c = context(&tx, &q.request.source_hash)?;
        if c.revision != q.revision || calendar::quote(&c, &q.request)? != *q {
            return Err(error(
                "STALE_REVISION",
                "Calendar source, destination, profile or review changed; preview again.",
            ));
        }
        let next = q
            .revision
            .checked_add(1)
            .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?;
        let receipt = calendar::Receipt {
            library_id,
            request: q.request.clone(),
            source_name: q.source_name.clone(),
            source_metadata_hash: q.source_metadata_hash.clone(),
            quote_hash: hash(q)?,
            plans: q.plans.clone(),
            committed_revision: next,
        };
        history.push(receipt.clone());
        let mut data: Value = serde_json::from_str(&merged_data(&profile, raw)?)?;
        data[calendar::FIELD] = serde_json::to_value(history)?;
        calendar::history(Some(&data))?;
        advance(&tx, q.revision)?;
        super::schedules_store::insert_reviewed_plans(
            &tx,
            &q.library_id,
            q.revision,
            next,
            &q.plans
                .iter()
                .map(|p| p.request.clone())
                .collect::<Vec<_>>(),
        )?;
        tx.execute("INSERT INTO chronicle_profile(singleton,data) VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET data=excluded.data",[serde_json::to_string(&data)?])?;
        audit(
            &tx,
            "legacy_calendar_import",
            Some(&q.request.source_hash),
            next,
        )?;
        tx.commit()?;
        self.title = profile.title;
        Ok(receipt)
    }
}
