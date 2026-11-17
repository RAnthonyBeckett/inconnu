use grapevine_workbench::{model::Role, support_export};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::Write,
};

const ST: Role = Role::Storyteller;
const SOURCE_LIMIT: usize = 8 * 1024 * 1024;
const TRACE_LIMIT: usize = 64 * 1024;

fn event(seconds: u64) -> Value {
    json!({
        "unix_seconds": seconds,
        "build": "0.29.0",
        "operation": "save",
        "code": "STORAGE_ERROR"
    })
}

fn append_line(bytes: &mut Vec<u8>, value: &Value) {
    bytes.extend(serde_json::to_vec(value).unwrap());
    bytes.push(b'\n');
}

#[test]
fn default_export_uses_an_allowlist_and_backtraces_require_opt_in() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut source = event(42);
    source["backtrace"] = json!("TRACE_PRIVATE_SENTINEL\nframe");
    source["source_path"] = json!("SOURCE_PATH_SENTINEL");
    source["private_data"] = json!("PRIVATE_DATA_SENTINEL");
    fs::write(&path, serde_json::to_vec(&source).unwrap()).unwrap();
    let before = fs::read(&path).unwrap();

    let snapshot = support_export::prepare(dir.path(), false, ST).unwrap();
    assert_eq!(snapshot.format, "grapevine-support-v1");
    assert_eq!(snapshot.app_version, env!("CARGO_PKG_VERSION"));
    assert!(!snapshot.source_missing);
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.events[0].unix_seconds, 42);
    assert_eq!(snapshot.events[0].build, "0.29.0");
    assert_eq!(snapshot.events[0].operation, "save");
    assert_eq!(snapshot.events[0].code, "STORAGE_ERROR");
    assert_eq!(snapshot.events[0].backtrace, None);
    assert!(!snapshot.events[0].trace_truncated);
    let hidden_json = support_export::json(&snapshot).unwrap();
    for secret in [
        "TRACE_PRIVATE_SENTINEL",
        "SOURCE_PATH_SENTINEL",
        "PRIVATE_DATA_SENTINEL",
    ] {
        assert!(!hidden_json.contains(secret));
    }
    assert!(!hidden_json.contains("\"backtrace\""));

    let opted_in = support_export::prepare(dir.path(), true, ST).unwrap();
    assert_eq!(
        opted_in.events[0].backtrace.as_deref(),
        Some("TRACE_PRIVATE_SENTINEL\nframe")
    );
    assert!(
        support_export::json(&opted_in)
            .unwrap()
            .contains("TRACE_PRIVATE_SENTINEL")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn malformed_rows_are_counted_and_optional_traces_are_null_or_utf8_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut bytes = Vec::new();

    let mut null_trace = event(1);
    null_trace["backtrace"] = Value::Null;
    append_line(&mut bytes, &null_trace);
    append_line(&mut bytes, &event(2)); // Backtrace may be absent.

    let mut invalid_operation = event(3);
    invalid_operation["operation"] = json!("bad operation");
    append_line(&mut bytes, &invalid_operation);
    let mut missing_code = event(4);
    missing_code.as_object_mut().unwrap().remove("code");
    append_line(&mut bytes, &missing_code);
    bytes.extend_from_slice(&[0xff, b'\n']);
    let mut invalid_trace_type = event(5);
    invalid_trace_type["backtrace"] = json!(7);
    append_line(&mut bytes, &invalid_trace_type);

    let mut long_trace = event(6);
    long_trace["backtrace"] = json!(format!("{}é", "x".repeat(TRACE_LIMIT - 1)));
    append_line(&mut bytes, &long_trace);
    fs::write(&path, bytes).unwrap();

    let snapshot = support_export::prepare(dir.path(), true, ST).unwrap();
    assert_eq!(snapshot.malformed_lines, 4);
    assert_eq!(snapshot.events.len(), 3);
    assert_eq!(snapshot.events[0].backtrace, None);
    assert_eq!(snapshot.events[1].backtrace, None);
    assert_eq!(
        snapshot.events[2].backtrace.as_ref().unwrap().len(),
        TRACE_LIMIT - 1
    );
    assert!(snapshot.events[2].trace_truncated);
    assert!(
        snapshot.events[2]
            .backtrace
            .as_ref()
            .unwrap()
            .is_char_boundary(TRACE_LIMIT - 1)
    );
}

#[test]
fn missing_and_unreadable_sources_are_distinguished_without_path_leaks() {
    let dir = tempfile::tempdir().unwrap();
    let missing = support_export::prepare(dir.path(), false, ST).unwrap();
    assert!(missing.source_missing);
    assert!(missing.events.is_empty());
    assert!(!missing.source_tail_truncated);

    let denied = support_export::prepare(dir.path(), false, Role::Player).unwrap_err();
    assert_eq!(denied.code, "SUPPORT_EXPORT_DENIED");

    let blocked = dir.path().join("private-dir-name");
    fs::create_dir_all(blocked.join("events.jsonl")).unwrap();
    let unreadable = support_export::prepare(&blocked, false, ST).unwrap_err();
    assert_eq!(unreadable.code, "SUPPORT_LOG_UNREADABLE");
    assert!(!unreadable.message.contains("private-dir-name"));
    assert!(!unreadable.message.contains("events.jsonl"));
}

#[test]
fn event_limit_tail_capture_and_snapshot_immutability_preserve_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut bytes = Vec::new();
    for seconds in 1..=1_001 {
        append_line(&mut bytes, &event(seconds));
    }
    fs::write(&path, &bytes).unwrap();
    let before = fs::read(&path).unwrap();

    let snapshot = support_export::prepare(dir.path(), false, ST).unwrap();
    assert!(!snapshot.source_tail_truncated);
    assert_eq!(snapshot.events.len(), 1_000);
    assert_eq!(snapshot.omitted_events, 1);
    assert_eq!(snapshot.events.first().unwrap().unix_seconds, 2);
    assert_eq!(snapshot.events.last().unwrap().unix_seconds, 1_001);
    assert_eq!(fs::read(&path).unwrap(), before);

    let captured = support_export::json(&snapshot).unwrap();
    let captured_value: Value = serde_json::from_str(&captured).unwrap();
    assert_eq!(
        captured_value["events"].as_array().unwrap().last().unwrap()["unix_seconds"],
        1_001
    );
    let mut appended = File::options().append(true).open(&path).unwrap();
    writeln!(
        appended,
        "{}",
        serde_json::to_string(&event(1_002)).unwrap()
    )
    .unwrap();
    assert_eq!(support_export::json(&snapshot).unwrap(), captured);

    let tail_dir = tempfile::tempdir().unwrap();
    let tail_path = tail_dir.path().join("events.jsonl");
    let mut oversized = vec![b'x'; SOURCE_LIMIT];
    oversized.push(b'\n');
    append_line(&mut oversized, &event(77));
    fs::write(&tail_path, &oversized).unwrap();
    let tail_before = fs::read(&tail_path).unwrap();
    let tail = support_export::prepare(tail_dir.path(), false, ST).unwrap();
    assert!(tail.source_tail_truncated);
    assert_eq!(tail.malformed_lines, 0);
    assert_eq!(tail.events.len(), 1);
    assert_eq!(tail.events[0].unix_seconds, 77);
    assert_eq!(fs::read(&tail_path).unwrap(), tail_before);
}

#[test]
fn pretty_packet_budget_drops_oldest_and_json_rejects_unrequested_traces() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let count = 1_000usize;
    let mut blank_trace = event(0);
    blank_trace["backtrace"] = json!("");
    let base = serde_json::to_vec(&blank_trace).unwrap().len() + 1;
    let trace_length = (SOURCE_LIMIT - 32 * 1024) / count - base;
    let trace = "x".repeat(trace_length);
    let mut source = File::create(&path).unwrap();
    for seconds in 0..count {
        let mut row = event(seconds as u64);
        row["backtrace"] = json!(trace);
        serde_json::to_writer(&mut source, &row).unwrap();
        source.write_all(b"\n").unwrap();
    }
    source.sync_all().unwrap();
    drop(source);
    let source_length = fs::metadata(&path).unwrap().len();
    assert!(source_length <= SOURCE_LIMIT as u64);
    let before = fs::read(&path).unwrap();

    let snapshot = support_export::prepare(dir.path(), true, ST).unwrap();
    assert!(!snapshot.source_tail_truncated);
    assert!(snapshot.omitted_events > 0);
    assert_eq!(snapshot.events.len() + snapshot.omitted_events, count);
    let encoded = support_export::json(&snapshot).unwrap();
    assert!(encoded.len() <= SOURCE_LIMIT);
    assert_eq!(fs::read(&path).unwrap(), before);

    let mut inconsistent = support_export::prepare(dir.path(), false, ST).unwrap();
    inconsistent.events.push(support_export::Event {
        unix_seconds: 1,
        build: "0.29.0".into(),
        operation: "save".into(),
        code: "ERROR".into(),
        backtrace: Some("NOT_OPTED_IN".into()),
        trace_truncated: false,
    });
    assert!(support_export::json(&inconsistent).is_err());
}
