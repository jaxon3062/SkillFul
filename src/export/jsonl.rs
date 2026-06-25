use std::io::Write;

use anyhow::Result;

use crate::db::Database;

pub fn write_events<W: Write>(database: &Database, writer: &mut W) -> Result<()> {
    database.write_all_events_jsonl(writer)
}
