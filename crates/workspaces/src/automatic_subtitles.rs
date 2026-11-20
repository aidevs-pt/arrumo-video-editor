use std::{error::Error, io, path::Path};

use rusqlite::params;

use crate::{WorkspaceStore, delivery_directory, delivery_verification::managed_delivery_file};

impl WorkspaceStore {
    pub fn process_next_automatic_subtitles(
        &self,
        database: &Path,
    ) -> Result<bool, Box<dyn Error>> {
        self.process_next_automatic_subtitles_with(database, |workspace, run, format| {
            self.generate_delivery_subtitles(database, workspace, run, format)
        })
    }

    pub(crate) fn process_next_automatic_subtitles_with(
        &self,
        database: &Path,
        generate: impl FnOnce(i64, i64, i64) -> Result<usize, Box<dyn Error>>,
    ) -> Result<bool, Box<dyn Error>> {
        let mut statement = self.0.prepare(
            "SELECT run.workspace_id, run.id, json_extract(format.value, '$.format.id')
             FROM generation_runs AS run, json_each(run.snapshot_json, '$.formats') AS format
             WHERE run.state = 'queued'
               AND json_extract(run.snapshot_json, '$.options.generate_delivery_subtitles') = 1
               AND json_extract(run.plan_json, '$.steps[' || run.stage_index || '].stage') = 'verify_delivery'
               AND json_type(format.value, '$.format.id') = 'integer'
               AND NOT EXISTS (
                   SELECT 1 FROM automatic_delivery_subtitles AS job
                   WHERE job.run_id = run.id AND job.format_id = json_extract(format.value, '$.format.id')
               )
             ORDER BY run.id, format.key",
        )?;
        let candidates = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<(i64, i64, i64)>>>()?;
        drop(statement);

        for (workspace, run, format) in candidates {
            if managed_delivery_file(database, workspace, run, &format!("{format}.mp4")).is_err()
                || !matches!(
                    delivery_directory(database, workspace, run)
                        .join(format!("{format}.srt"))
                        .symlink_metadata(),
                    Err(error) if error.kind() == io::ErrorKind::NotFound
                )
            {
                continue;
            }

            // Reserve the attempt durably before any external transcription can occur.
            // A restarted worker cannot tell whether an interrupted provider call was charged.
            let claimed = self.0.execute(
                "INSERT INTO automatic_delivery_subtitles (run_id, format_id)
                 SELECT id, ?3 FROM generation_runs
                 WHERE id = ?2 AND workspace_id = ?1 AND state = 'queued'
                   AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = 'verify_delivery'
                 ON CONFLICT (run_id, format_id) DO NOTHING",
                params![workspace, run, format],
            )?;
            if claimed == 0 {
                continue;
            }
            let state = if generate(workspace, run, format).is_ok() {
                "completed"
            } else {
                "failed"
            };
            self.0.execute(
                "UPDATE automatic_delivery_subtitles SET state = ?3 WHERE run_id = ?1 AND format_id = ?2",
                params![run, format, state],
            )?;
            return Ok(true);
        }
        Ok(false)
    }
}
