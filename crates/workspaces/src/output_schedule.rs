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
    /// The number of portrait cuts this output makes the way the requested profile makes its
    /// videos, when it is listed for that profile only because of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portrait_cuts: Option<u32>,
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
                    json_extract(format.value, '$.format.height'),
                    CASE WHEN ?2 IS NOT NULL
                          AND json_extract(run.snapshot_json, '$.draft.profile_id') IS NOT ?2
                         THEN json_extract(format.value, '$.delivery.supplemental.portrait_derivatives.count') END
             FROM output_schedules AS schedule
             JOIN generation_runs AS run ON run.id = schedule.run_id
             JOIN json_each(run.snapshot_json, '$.formats') AS format
               ON json_extract(format.value, '$.format.id') = schedule.format_id
             WHERE run.workspace_id = ?1 AND run.state = 'completed'
               AND (?2 IS NULL OR json_extract(run.snapshot_json, '$.draft.profile_id') = ?2
                    OR (json_extract(format.value, '$.portrait_source.profile_id') = ?2
                        AND json_extract(format.value, '$.delivery.supplemental.portrait_derivatives.count') > 0))
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
                    portrait_cuts: row.get(9)?,
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

#[cfg(test)]
mod tests {
    use crate::{DraftSettings, RunOptions, WorkspaceStore};

    #[test]
    fn a_profile_calendar_includes_outputs_whose_portrait_cuts_follow_it() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let reviews = store
            .create_profile(workspace, "Reviews", "", None)
            .unwrap()
            .id;
        let shorts = store
            .create_profile(workspace, "Shorts", "", None)
            .unwrap()
            .id;
        let draft = store
            .create_video_draft(
                workspace,
                DraftSettings {
                    title: "Launch".into(),
                    intent: "Fixture".into(),
                    profile_id: Some(reviews),
                    source_ids: Vec::new(),
                },
            )
            .unwrap();
        let run = store
            .enqueue_video_draft(workspace, draft.id, "fixture", &RunOptions::default())
            .unwrap()
            .id;
        let formats = serde_json::json!([
            {"format": {"id": 5, "name": "Landscape", "width": 1920, "height": 1080},
             "portrait_source": {"profile_id": shorts},
             "delivery": {"supplemental": {"portrait_derivatives": {"count": 2}}}},
            {"format": {"id": 6, "name": "Square", "width": 1080, "height": 1080}}
        ]);
        store
            .0
            .execute(
                "UPDATE generation_runs SET state = 'completed',
                     snapshot_json = json_set(snapshot_json, '$.formats', json(?2)) WHERE id = ?1",
                rusqlite::params![run, formats.to_string()],
            )
            .unwrap();
        store.0.execute_batch(&format!(
            "INSERT INTO output_schedules VALUES ({run}, 5, '2026-12-01'), ({run}, 6, '2026-12-02');"
        )).unwrap();

        let own = store
            .scheduled_outputs(workspace, Some(reviews), "2026-12-01", "2026-12-31")
            .unwrap();
        assert_eq!(own.len(), 2);
        assert!(own.iter().all(|output| output.portrait_cuts.is_none()));
        let source = store
            .scheduled_outputs(workspace, Some(shorts), "2026-12-01", "2026-12-31")
            .unwrap();
        assert_eq!(source.len(), 1);
        assert_eq!((source[0].format_id, source[0].portrait_cuts), (5, Some(2)));
        assert_eq!(
            store
                .scheduled_outputs(workspace, None, "2026-12-01", "2026-12-31")
                .unwrap()
                .len(),
            2
        );
    }
}
