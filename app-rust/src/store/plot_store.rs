use super::*;
use crate::plots::{self, Plot, Snapshot};

fn require_plot(record: &Record) -> Result<()> {
    if record.kind == "Plot" && record.build.is_none() {
        Ok(())
    } else {
        Err(error("PLOT_REQUIRED", "Select a live plot."))
    }
}

impl Library {
    pub fn plot_snapshot(&self, id: &str, role: Role) -> Result<Snapshot> {
        require_st(role)?;
        let revision = self.revision()?;
        let record = self.stored_record(id)?;
        require_plot(&record)?;
        let (plot, legacy, retained) = plots::read(&record)?;
        if revision != self.revision()? {
            return Err(error(
                "STALE_REVISION",
                "The library changed while reading the plot. Reopen it.",
            ));
        }
        Ok(Snapshot {
            revision,
            plot,
            legacy,
            retained,
        })
    }

    pub fn create_plot(&mut self, plot: &Plot, expected: i64, role: Role) -> Result<String> {
        require_st(role)?;
        plots::validate(plot)?;
        let id = Uuid::new_v4().to_string();
        let mut record = Record {
            id: id.clone(),
            kind: "Plot".into(),
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
        apply_plot(&mut record, plot)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        insert(&tx, &record)?;
        audit(&tx, "create_plot", Some(&id), expected + 1)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn save_plot(&mut self, id: &str, plot: &Plot, expected: i64, role: Role) -> Result<i64> {
        require_st(role)?;
        plots::validate(plot)?;
        let mut record = self.stored_record(id)?;
        require_plot(&record)?;
        apply_plot(&mut record, plot)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET name=?1,data=?2 WHERE id=?3",
            params![record.name, serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "edit_plot", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
}

fn apply_plot(record: &mut Record, plot: &Plot) -> Result<()> {
    let mut plot = plot.clone();
    plot.name = plot.name.trim().to_owned();
    plot.order_timeline();
    record.name.clone_from(&plot.name);
    record
        .private_fields
        .insert(plots::FIELD.into(), serde_json::to_value(plot)?);
    Ok(())
}
