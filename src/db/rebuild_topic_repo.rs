use rusqlite::Connection;

use crate::error::Result;
use crate::id::{RebuildId, TopicId};

pub fn add_topic_to_rebuild(
    conn: &Connection,
    rebuild_id: &RebuildId,
    topic_id: &TopicId,
    phase: i32,
    merge_order: i32,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO rebuild_topics (rebuild_id, topic_id, phase, merge_order) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![rebuild_id, topic_id, phase, merge_order],
    )?;
    Ok(())
}

pub fn get_topics_for_rebuild(
    conn: &Connection,
    rebuild_id: &RebuildId,
) -> Result<Vec<(TopicId, i32, i32)>> {
    let mut stmt = conn.prepare(
        "SELECT topic_id, phase, merge_order FROM rebuild_topics WHERE rebuild_id = ?1 ORDER BY phase, merge_order",
    )?;

    let rows = stmt.query_map([rebuild_id], |row| {
        Ok((row.get::<_, TopicId>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}
