//! Fetching over HTTP, through `curl`, as omahelm and omawind do.
//!
//! Everything omatide downloads is a small JSON document from NOAA's
//! CO-OPS service, so the limits here are tight: a station list is about
//! four megabytes, and everything else is a few kilobytes.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

const USER_AGENT: &str = concat!(
    "omatide/",
    env!("CARGO_PKG_VERSION"),
    " (+https://omahoy.org)"
);
/// The most any one answer may weigh. NOAA's whole current-station list
/// is under four megabytes; nothing else comes close.
pub const LIMIT: u64 = 16 << 20;
/// curl gives up after 120 seconds; this is in case curl itself hangs.
const DEADLINE: Duration = Duration::from_secs(150);

/// curl is stopped after `DEADLINE` whatever it's doing. Both of its pipes
/// are read at once, so it never waits on one while omatide waits on the
/// other.
pub fn get(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut child = Command::new("curl")
        .args([
            "--http1.1",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
        ])
        .args(["--connect-timeout", "20", "--max-time", "120"])
        .args([
            "--user-agent",
            USER_AGENT,
            "--max-filesize",
            &limit.to_string(),
        ])
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("can't run curl: {e}"))?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        stop(&mut child);
        return Err("curl: no pipes".into());
    };
    let over = Arc::new(AtomicBool::new(false));
    let (body_tx, body_rx) = mpsc::channel();
    {
        let over = over.clone();
        std::thread::spawn(move || {
            let mut body = Vec::new();
            let read = stdout.take(limit + 1).read_to_end(&mut body);
            if body.len() as u64 > limit {
                over.store(true, Ordering::SeqCst);
            }
            let _ = body_tx.send(read.ok().map(|_| body));
        });
    }
    // Everything curl says is read; the first 64 KB is kept.
    let (why_tx, why_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stderr, mut kept, mut chunk) = (stderr, Vec::new(), [0u8; 4096]);
        while let Ok(n) = stderr.read(&mut chunk) {
            if n == 0 {
                break;
            }
            if kept.len() < 64 * 1024 {
                kept.extend_from_slice(&chunk[..n]);
            }
        }
        let _ = why_tx.send(String::from_utf8_lossy(&kept).trim().to_string());
    });
    let started = Instant::now();
    let left = || DEADLINE.saturating_sub(started.elapsed());
    let status = loop {
        let too_big = over.load(Ordering::SeqCst);
        if too_big || left().is_zero() {
            stop(&mut child);
            return Err(if too_big {
                format!("{url}: more than {} MB", limit >> 20)
            } else {
                format!("{url}: no answer in {} s", DEADLINE.as_secs())
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                stop(&mut child);
                return Err(format!("curl: {e}"));
            }
        }
    };
    // curl has gone, but something it started may still hold its pipes:
    // the rest of the answer is waited for only until the deadline.
    let (Ok(body), Ok(why)) = (body_rx.recv_timeout(left()), why_rx.recv_timeout(left())) else {
        // curl is reaped; the readers finish when whatever holds the pipes
        // lets go.
        return Err(format!("{url}: no answer in {} s", DEADLINE.as_secs()));
    };
    if !status.success() {
        return Err(if why.is_empty() {
            format!("curl failed on {url}")
        } else {
            why
        });
    }
    match body {
        Some(b) if b.len() as u64 <= limit => Ok(b),
        Some(_) => Err(format!("{url}: more than {} MB", limit >> 20)),
        None => Err(format!("{url}: couldn't read the answer")),
    }
}

/// Kills curl and reaps it. curl stays in omatide's process group, so
/// Ctrl-C reaches it too, and if omatide exits mid-download curl fails its
/// next write to the closed pipe, or stops at its own --max-time.
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// The body as text, for the JSON everything here actually is.
pub fn get_text(url: &str) -> Result<String, String> {
    let bytes = get(url, LIMIT)?;
    String::from_utf8(bytes).map_err(|_| format!("{url}: the answer wasn't text"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_that_goes_nowhere_fails_rather_than_hangs() {
        // Reserved by RFC 2606, so this can never reach a real server.
        let e = get_text("https://omatide.invalid/nothing").unwrap_err();
        assert!(!e.is_empty());
    }

    #[test]
    fn a_limit_of_nothing_is_refused_not_truncated() {
        // file:// keeps this off the network. curl's --max-filesize
        // stops it, and a short read must be reported, never returned
        // as if it were the whole answer.
        let path = std::env::current_dir().unwrap().join("Cargo.toml");
        let url = format!("file://{}", path.display());
        assert!(get(&url, 1).is_err());
        assert!(get(&url, LIMIT).unwrap().len() > 1);
    }
}
