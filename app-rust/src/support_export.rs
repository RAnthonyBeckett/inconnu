//! Read-only, privacy-bounded preparation of a local support file.
//!
//! The prepared packet contains only a small allowlist from local diagnostic
//! events. It is a file preview/export helper; it never sends data anywhere.

use crate::model::{Result, Role, error};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const FORMAT: &str = "grapevine-support-v1";
const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PACKET_BYTES: usize = 8 * 1024 * 1024;
const MAX_EVENTS: usize = 1_000;
const MAX_BACKTRACE_BYTES: usize = 64 * 1024;
const MAX_FIELD_BYTES: usize = 80;

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub format: String,
    pub app_version: String,
    pub prepared_unix_seconds: u64,
    pub include_backtraces: bool,
    pub source_missing: bool,
    pub source_tail_truncated: bool,
    pub malformed_lines: usize,
    pub omitted_events: usize,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub unix_seconds: u64,
    pub build: String,
    pub operation: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtrace: Option<String>,
    pub trace_truncated: bool,
}

#[derive(Deserialize)]
struct RawEvent {
    unix_seconds: u64,
    build: String,
    operation: String,
    code: String,
    #[serde(default)]
    backtrace: Option<String>,
}

fn denied() -> crate::model::AppError {
    error(
        "SUPPORT_EXPORT_DENIED",
        "Only a Storyteller can prepare a local support export.",
    )
}

fn unreadable() -> crate::model::AppError {
    error(
        "SUPPORT_LOG_UNREADABLE",
        "The local diagnostic log could not be read.",
    )
}

fn invalid_snapshot() -> crate::model::AppError {
    error(
        "SUPPORT_EXPORT_INVALID",
        "The prepared support snapshot is inconsistent or exceeds its size limit.",
    )
}

fn valid_ascii_field(value: &str, allow_dots_and_hyphens: bool) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FIELD_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || byte == b'_'
                || (allow_dots_and_hyphens && matches!(byte, b'.' | b'-'))
        })
}

fn truncate_backtrace(value: &str) -> (String, bool) {
    if value.len() <= MAX_BACKTRACE_BYTES {
        return (value.to_owned(), false);
    }
    let mut end = MAX_BACKTRACE_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), true)
}

fn parse_event(line: &[u8], include_backtraces: bool) -> Option<Event> {
    let raw: RawEvent = serde_json::from_slice(line).ok()?;
    if !valid_ascii_field(&raw.build, true)
        || !valid_ascii_field(&raw.operation, false)
        || !valid_ascii_field(&raw.code, false)
    {
        return None;
    }
    let (backtrace, trace_truncated) = if include_backtraces {
        raw.backtrace
            .map(|trace| {
                let (trace, truncated) = truncate_backtrace(&trace);
                (Some(trace), truncated)
            })
            .unwrap_or((None, false))
    } else {
        (None, false)
    };
    Some(Event {
        unix_seconds: raw.unix_seconds,
        build: raw.build,
        operation: raw.operation,
        code: raw.code,
        backtrace,
        trace_truncated,
    })
}

fn captured_tail(file: &mut File) -> Result<(Vec<u8>, bool)> {
    let length = file.metadata().map_err(|_| unreadable())?.len();
    let start = length.saturating_sub(MAX_SOURCE_BYTES);
    let truncated = start > 0;
    file.seek(SeekFrom::Start(start))
        .map_err(|_| unreadable())?;
    let captured_length = length - start;
    let mut bytes = Vec::with_capacity(captured_length as usize);
    (&mut *file)
        .take(captured_length)
        .read_to_end(&mut bytes)
        .map_err(|_| unreadable())?;

    if truncated && !bytes.is_empty() {
        let mut previous = [0u8; 1];
        file.seek(SeekFrom::Start(start - 1))
            .map_err(|_| unreadable())?;
        file.read_exact(&mut previous).map_err(|_| unreadable())?;
        if previous[0] != b'\n' {
            if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
                bytes.drain(..=newline);
            } else {
                bytes.clear();
            }
        }
    }
    Ok((bytes, truncated))
}

fn collect_events(bytes: &[u8], include_backtraces: bool) -> (Vec<Event>, usize, usize) {
    let mut events = VecDeque::with_capacity(MAX_EVENTS);
    let mut malformed_lines = 0usize;
    let mut omitted_events = 0usize;
    if bytes.is_empty() {
        return (Vec::new(), malformed_lines, omitted_events);
    }

    let mut line_start = 0usize;
    for newline in bytes
        .iter()
        .enumerate()
        .filter_map(|(index, byte)| (*byte == b'\n').then_some(index))
    {
        let line = &bytes[line_start..newline];
        if line.is_empty() {
            malformed_lines += 1;
        } else if let Some(event) = parse_event(line, include_backtraces) {
            if events.len() == MAX_EVENTS {
                events.pop_front();
                omitted_events += 1;
            }
            events.push_back(event);
        } else {
            malformed_lines += 1;
        }
        line_start = newline + 1;
    }
    if line_start < bytes.len() {
        let line = &bytes[line_start..];
        if let Some(event) = parse_event(line, include_backtraces) {
            if events.len() == MAX_EVENTS {
                events.pop_front();
                omitted_events += 1;
            }
            events.push_back(event);
        } else {
            malformed_lines += 1;
        }
    }
    (
        events.into_iter().collect(),
        malformed_lines,
        omitted_events,
    )
}

fn pretty_json(snapshot: &Snapshot) -> Result<String> {
    serde_json::to_string_pretty(snapshot).map_err(|_| invalid_snapshot())
}

fn cap_packet(snapshot: &mut Snapshot) -> Result<()> {
    loop {
        let encoded = pretty_json(snapshot)?;
        if encoded.len() <= MAX_PACKET_BYTES {
            return Ok(());
        }
        if snapshot.events.is_empty() {
            return Err(invalid_snapshot());
        }
        let excess = encoded.len() - MAX_PACKET_BYTES;
        let mut removed_bytes = 0usize;
        let mut remove_count = 0usize;
        while remove_count < snapshot.events.len() && removed_bytes < excess {
            let event_bytes = serde_json::to_vec(&snapshot.events[remove_count])
                .map_err(|_| invalid_snapshot())?
                .len();
            removed_bytes = removed_bytes.saturating_add(event_bytes);
            remove_count += 1;
        }
        if remove_count == 0 {
            return Err(invalid_snapshot());
        }
        snapshot.events.drain(..remove_count);
        snapshot.omitted_events += remove_count;
    }
}

/// Captures a bounded, allowlisted view of `events.jsonl` without changing it.
pub fn prepare(dir: &Path, include_backtraces: bool, role: Role) -> Result<Snapshot> {
    if role != Role::Storyteller {
        return Err(denied());
    }

    let path = dir.join("events.jsonl");
    let (events, source_missing, source_tail_truncated, malformed_lines, omitted_events) =
        match File::open(&path) {
            Ok(mut file) => {
                let (bytes, truncated) = captured_tail(&mut file)?;
                let (events, malformed, omitted) = collect_events(&bytes, include_backtraces);
                (events, false, truncated, malformed, omitted)
            }
            Err(problem) if problem.kind() == std::io::ErrorKind::NotFound => {
                (Vec::new(), true, false, 0, 0)
            }
            Err(_) => return Err(unreadable()),
        };

    let mut snapshot = Snapshot {
        format: FORMAT.into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        prepared_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        include_backtraces,
        source_missing,
        source_tail_truncated,
        malformed_lines,
        omitted_events,
        events,
    };
    cap_packet(&mut snapshot)?;
    Ok(snapshot)
}

/// Serializes the immutable prepared snapshot. This function never reads files.
pub fn json(snapshot: &Snapshot) -> Result<String> {
    if !snapshot.include_backtraces
        && snapshot
            .events
            .iter()
            .any(|event| event.backtrace.is_some() || event.trace_truncated)
        || snapshot
            .events
            .iter()
            .any(|event| event.trace_truncated && event.backtrace.is_none())
    {
        return Err(invalid_snapshot());
    }
    let encoded = pretty_json(snapshot)?;
    if encoded.len() > MAX_PACKET_BYTES {
        return Err(invalid_snapshot());
    }
    Ok(encoded)
}
