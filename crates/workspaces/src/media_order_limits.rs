//! How much new media one asset plan or preview revision may order from the creator's
//! providers. Each order costs provider credits, so the app keeps a ceiling the creator can raise.

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::WorkspaceStore;

pub const DEFAULT_MAX_MEDIA: u32 = 12;
pub const DEFAULT_MAX_CLIPS: u32 = 6;
/// The highest either limit may be set to.
pub const MAX_MEDIA_LIMIT: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MediaOrderLimits {
    /// New pieces of media of any kind one plan or revision may order.
    pub max_media: u32,
    /// How many of them may be video clips, which cost the most.
    pub max_clips: u32,
    pub default_max_media: u32,
    pub default_max_clips: u32,
}

impl WorkspaceStore {
    pub fn media_order_limits(&self) -> rusqlite::Result<MediaOrderLimits> {
        let (media, clips): (Option<u32>, Option<u32>) = self
            .0
            .query_row(
                "SELECT max_media, max_clips FROM media_order_limits WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .unwrap_or_default();
        let max_media = media.unwrap_or(DEFAULT_MAX_MEDIA);
        Ok(MediaOrderLimits {
            max_media,
            // Clips are some of the media, so never more than it.
            max_clips: clips.unwrap_or(DEFAULT_MAX_CLIPS).min(max_media),
            default_max_media: DEFAULT_MAX_MEDIA,
            default_max_clips: DEFAULT_MAX_CLIPS,
        })
    }

    /// Sets both limits; `None` restores a default. Clips may not exceed the media limit.
    pub fn set_media_order_limits(
        &self,
        max_media: Option<u32>,
        max_clips: Option<u32>,
    ) -> rusqlite::Result<MediaOrderLimits> {
        let media = max_media.unwrap_or(DEFAULT_MAX_MEDIA);
        let valid = |value: Option<u32>| value.is_none_or(|value| value <= MAX_MEDIA_LIMIT);
        if !valid(max_media) || !valid(max_clips) || max_clips.is_some_and(|clips| clips > media) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0.execute(
            "INSERT INTO media_order_limits (id, max_media, max_clips) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET max_media = excluded.max_media, max_clips = excluded.max_clips",
            rusqlite::params![max_media, max_clips],
        )?;
        self.media_order_limits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_default_to_twelve_and_six_and_can_be_raised() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let limits = store.media_order_limits().unwrap();
        assert_eq!((limits.max_media, limits.max_clips), (12, 6));
        let raised = store.set_media_order_limits(Some(30), Some(20)).unwrap();
        assert_eq!((raised.max_media, raised.max_clips), (30, 20));
        assert!(store.set_media_order_limits(Some(51), None).is_err());
        assert!(
            store.set_media_order_limits(Some(4), Some(5)).is_err(),
            "clips are part of the media"
        );
        let none = store.set_media_order_limits(Some(0), Some(0)).unwrap();
        assert_eq!((none.max_media, none.max_clips), (0, 0));
        // A lower media limit with the default clips keeps clips within it.
        let low = store.set_media_order_limits(Some(2), None).unwrap();
        assert_eq!((low.max_media, low.max_clips), (2, 2));
        let reset = store.set_media_order_limits(None, None).unwrap();
        assert_eq!((reset.max_media, reset.max_clips), (12, 6));
    }
}
