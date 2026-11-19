use std::collections::BTreeMap;

use rusqlite::{Transaction, TransactionBehavior, params};
use serde::Serialize;

use crate::WorkspaceStore;

#[derive(Debug, Serialize)]
pub struct ScheduledOutput {
    pub run_id: i64,
    pub format_id: i64,
    pub scheduled_date: String,
    pub title: String,
    pub profile_id: Option<i64>,
    pub profile_color: Option<String>,
    pub format_name: String,
    pub width: u32,
    pub height: u32,
}

impl WorkspaceStore {
    pub fn set_output_schedule(
        &self,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
        date: Option<&str>,
    ) -> rusqlite::Result<()> {
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        if !self
            .verified_delivery(workspace_id, run_id)?
            .verified_outputs
            .iter()
            .any(|output| output.format_id == format_id)
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if let Some(date) = date {
            validate_date(&tx, date)?;
            tx.execute(
                "INSERT INTO output_schedules (run_id, format_id, scheduled_date) VALUES (?1, ?2, ?3)
                 ON CONFLICT(run_id, format_id) DO UPDATE SET scheduled_date = excluded.scheduled_date",
                params![run_id, format_id, date],
            )?;
        } else {
            tx.execute(
                "DELETE FROM output_schedules WHERE run_id = ?1 AND format_id = ?2",
                params![run_id, format_id],
            )?;
        }
        tx.commit()
    }

    pub(crate) fn run_output_dates(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<BTreeMap<i64, String>> {
        self.0.prepare(
            "SELECT schedule.format_id, schedule.scheduled_date FROM output_schedules AS schedule
             JOIN generation_runs AS run ON run.id = schedule.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND run.state = 'completed'",
        )?.query_map(params![workspace_id, run_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect()
    }

    pub fn scheduled_outputs(
        &self,
        workspace_id: i64,
        profile_id: Option<i64>,
        start: &str,
        end: &str,
    ) -> rusqlite::Result<Vec<ScheduledOutput>> {
        validate_date(&self.0, start)?;
        validate_date(&self.0, end)?;
        if start > end {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0
            .prepare(
                "SELECT run.id, schedule.format_id, schedule.scheduled_date,
                    COALESCE(run.display_title, json_extract(run.snapshot_json, '$.draft.title')),
                    json_extract(run.snapshot_json, '$.draft.profile_id'),
                    json_extract(run.snapshot_json, '$.profile.color'),
                    json_extract(format.value, '$.format.name'),
                    json_extract(format.value, '$.format.width'),
                    json_extract(format.value, '$.format.height')
             FROM output_schedules AS schedule
             JOIN generation_runs AS run ON run.id = schedule.run_id
             JOIN json_each(run.snapshot_json, '$.formats') AS format
               ON json_extract(format.value, '$.format.id') = schedule.format_id
             WHERE run.workspace_id = ?1 AND run.state = 'completed'
               AND (?2 IS NULL OR json_extract(run.snapshot_json, '$.draft.profile_id') = ?2)
               AND schedule.scheduled_date BETWEEN ?3 AND ?4
             ORDER BY schedule.scheduled_date, run.id, schedule.format_id",
            )?
            .query_map(params![workspace_id, profile_id, start, end], |row| {
                Ok(ScheduledOutput {
                    run_id: row.get(0)?,
                    format_id: row.get(1)?,
                    scheduled_date: row.get(2)?,
                    title: row.get(3)?,
                    profile_id: row.get(4)?,
                    profile_color: row.get(5)?,
                    format_name: row.get(6)?,
                    width: row.get(7)?,
                    height: row.get(8)?,
                })
            })?
            .collect()
    }
}

fn validate_date(connection: &rusqlite::Connection, date: &str) -> rusqlite::Result<()> {
    if date.len() != 10 || !("0001-01-01"..="9999-12-31").contains(&date) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let normalized: Option<String> =
        connection.query_row("SELECT date(?1, '+0 days')", [date], |row| row.get(0))?;
    if normalized.as_deref() != Some(date) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}
