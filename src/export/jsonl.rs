use anyhow::Result;

use crate::{db::Database, event::EventRecord};

pub fn export_events(database: &Database) -> Result<String> {
    let events = database.all_events()?;
    render_jsonl(&events)
}

fn render_jsonl(events: &[EventRecord]) -> Result<String> {
    let mut lines = Vec::with_capacity(events.len());
    for event in events {
        lines.push(serde_json::to_string(event)?);
    }
    Ok(lines.join("\n"))
}
