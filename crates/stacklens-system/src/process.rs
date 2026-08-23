use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_CAPTURE_BYTES: usize = 8 * 1024;

pub struct CommandOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub not_found: bool,
    pub spawn_detail: Option<String>,
}

pub fn run_with_timeout(
    mut command: Command,
    timeout: Duration,
    max_bytes: usize,
) -> CommandOutcome {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.stdin(Stdio::null());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return CommandOutcome {
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
                not_found: err.kind() == std::io::ErrorKind::NotFound,
                spawn_detail: Some(err.to_string()),
            };
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_handle = thread::spawn(move || read_bounded(stdout, max_bytes));
    let stderr_handle = thread::spawn(move || read_bounded(stderr, max_bytes));

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut spawn_detail = None;
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(err) => {
                spawn_detail = Some(err.to_string());
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();

    CommandOutcome {
        exit_code,
        stdout,
        stderr,
        timed_out,
        not_found: false,
        spawn_detail,
    }
}

fn read_bounded<R: Read>(reader: Option<R>, max_bytes: usize) -> String {
    let Some(mut reader) = reader else {
        return String::new();
    };
    let mut buf = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() < max_bytes {
                    let take = n.min(max_bytes - buf.len());
                    buf.extend_from_slice(&chunk[..take]);
                }
            }
            Err(_) => break,
        }
    }
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if buf.len() >= max_bytes {
        text.push_str("\n...[truncated]");
    }
    text
}

pub fn truncate_detail(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 512 {
        return trimmed.to_owned();
    }
    let mut end = 512;
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &trimmed[..end])
}

#[cfg(test)]
mod tests {
    use super::{run_with_timeout, truncate_detail, MAX_CAPTURE_BYTES};
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn missing_binary_is_not_found() {
        let outcome = run_with_timeout(
            Command::new("stacklens-process-does-not-exist"),
            Duration::from_secs(2),
            MAX_CAPTURE_BYTES,
        );
        assert!(outcome.not_found);
        assert!(!outcome.timed_out);
    }

    #[test]
    fn truncate_does_not_split_utf8() {
        let text = "é".repeat(400);
        let truncated = truncate_detail(&text);
        assert!(truncated.ends_with('…'));
        assert!(truncated.is_char_boundary(truncated.len() - '…'.len_utf8()));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_sleep() {
        let mut command = Command::new("sleep");
        command.arg("10");
        let outcome = run_with_timeout(command, Duration::from_millis(250), MAX_CAPTURE_BYTES);
        assert!(outcome.timed_out);
    }

    #[cfg(windows)]
    #[test]
    fn timeout_kills_ping() {
        let mut command = Command::new("ping");
        command.args(["-n", "20", "127.0.0.1"]);
        let outcome = run_with_timeout(command, Duration::from_millis(400), MAX_CAPTURE_BYTES);
        assert!(outcome.timed_out);
    }
}
