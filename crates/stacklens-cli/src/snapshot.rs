use std::io::Write;

use crate::capture::current_snapshot;

pub fn run(stdout: &mut dyn Write) -> anyhow::Result<()> {
    let snapshot = current_snapshot();
    writeln!(stdout, "{}", snapshot.to_json_string()?)?;
    Ok(())
}
