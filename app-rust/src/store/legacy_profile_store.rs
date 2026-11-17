use super::chronicle_store::{merged_data, read_profile, validate_profile};
use super::*;
use crate::legacy_profile as legacy;
fn context(conn: &Connection, hash: &str) -> Result<legacy::Context> {
    let (library_id, revision): (String, i64) =
        conn.query_row("SELECT id,revision FROM library", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    let (name, data, bytes): (String, String, Vec<u8>) = conn.query_row(
        "SELECT source_name,report,source_bytes FROM imports WHERE hash=?1",
        [hash],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    use sha2::{Digest, Sha256};
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != hash {
        return Err(error(
            "LEGACY_PROFILE_SOURCE_CHANGED",
            "Retained source bytes no longer match their import hash. Review a trusted source copy.",
        ));
    }
    let (profile, raw) = read_profile(conn)?;
    let report: Value = serde_json::from_str(&data)?;
    let (fields, source_metadata_hash) = legacy::candidates(&report, &profile)?;
    Ok(legacy::Context {
        library_id,
        revision,
        source_hash: hash.into(),
        source_name: name,
        source_metadata_hash,
        profile_before_hash: legacy::hash(&(&profile, &raw))?,
        profile,
        fields,
        history: legacy::history(raw.as_ref())?,
    })
}
impl Library {
    pub fn legacy_profile_sources(&self, role: Role) -> Result<Vec<legacy::Source>> {
        require_st(role)?;
        let mut statement = self
            .conn
            .prepare("SELECT hash,source_name,report FROM imports ORDER BY source_name")?;
        let mut sources = vec![];
        for row in statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (hash, name, data) = row?;
            let report: Value = serde_json::from_str(&data)?;
            let result = legacy::candidates(&report, &Default::default());
            let (available_fields, issue) = match result {
                Ok((fields, _)) => (fields.iter().filter(|c| c.proposed.is_some()).count(), None),
                Err(e) => (0, Some(e.message)),
            };
            sources.push(legacy::Source {
                hash,
                name,
                available_fields,
                issue,
            });
        }
        Ok(sources)
    }
    pub fn legacy_profile_context(&self, hash: &str, role: Role) -> Result<legacy::Context> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let c = context(&tx, hash)?;
        tx.commit()?;
        Ok(c)
    }
    pub fn quote_legacy_profile(
        &self,
        request: &legacy::Request,
        expected: i64,
        role: Role,
    ) -> Result<legacy::Quote> {
        require_st(role)?;
        let tx = self.conn.unchecked_transaction()?;
        let c = context(&tx, &request.source_hash)?;
        if c.revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing legacy Chronicle values.",
            ));
        }
        let q = legacy::quote(&c, request)?;
        validate_profile(&q.proposed)?;
        tx.commit()?;
        Ok(q)
    }
    pub fn commit_legacy_profile(
        &mut self,
        q: &legacy::Quote,
        role: Role,
    ) -> Result<legacy::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (current, raw) = read_profile(&tx)?;
        let mut history = legacy::history(raw.as_ref())?;
        let library_id: String = tx.query_row("SELECT id FROM library", [], |r| r.get(0))?;
        if library_id != q.library_id {
            return Err(error(
                "LEGACY_PROFILE_LIBRARY",
                "This review belongs to another library.",
            ));
        }
        if let Some(saved) = history.iter().find(|r| r.request.id == q.request.id) {
            if saved.quote_hash != legacy::hash(q)? {
                return Err(error(
                    "LEGACY_PROFILE_ID",
                    "This review ID already identifies another operation.",
                ));
            }
            let saved = saved.clone();
            tx.commit()?;
            self.title = current.title;
            return Ok(saved);
        }
        let c = context(&tx, &q.request.source_hash)?;
        if c.revision != q.revision || legacy::quote(&c, &q.request)? != *q {
            return Err(error(
                "STALE_REVISION",
                "Source, profile or review changed; preview again.",
            ));
        }
        validate_profile(&q.proposed)?;
        let committed_revision = q
            .revision
            .checked_add(1)
            .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?;
        let receipt = legacy::Receipt {
            request: q.request.clone(),
            source_name: q.source_name.clone(),
            source_metadata_hash: q.source_metadata_hash.clone(),
            profile_before_hash: q.profile_before_hash.clone(),
            quote_hash: legacy::hash(q)?,
            changes: q.changes.clone(),
            committed_revision,
        };
        history.push(receipt.clone());
        let mut data: Value = serde_json::from_str(&merged_data(&q.proposed, raw)?)?;
        data[legacy::FIELD] = serde_json::to_value(history)?;
        legacy::history(Some(&data))?;
        advance(&tx, q.revision)?;
        tx.execute("UPDATE library SET title=?1", [&q.proposed.title])?;
        tx.execute("INSERT INTO chronicle_profile(singleton,data) VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET data=excluded.data",[serde_json::to_string(&data)?])?;
        audit(
            &tx,
            "legacy_chronicle_reconcile",
            Some(&q.request.source_hash),
            committed_revision,
        )?;
        tx.commit()?;
        self.title = q.proposed.title.clone();
        Ok(receipt)
    }
}
