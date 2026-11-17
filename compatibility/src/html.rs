use crate::Report;
use serde_json::Value;
use std::fmt::Write;

fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        out.push_str(match c {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&#39;",
            _ => {
                out.push(c);
                continue;
            }
        });
    }
    out
}

pub fn record_name(record: &Value) -> String {
    if let Some(fields) = record.get("fields") {
        for key in ["Name", "Title", "CharName"] {
            if let Some(name) = fields.get(key).and_then(Value::as_str) {
                return name.into();
            }
        }
    }
    if let Some(attributes) = record.pointer("/xml/attributes").and_then(Value::as_array) {
        for key in ["name", "title", "character"] {
            if let Some(value) = attributes
                .iter()
                .find(|a| a["name"] == key)
                .and_then(|a| a["value"].as_str())
            {
                return value.into();
            }
        }
    }
    "Unnamed record".into()
}

fn value_html(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            if let Some(v) = map.get("vb_raw") {
                let _ = write!(out, "{} (stored Boolean: {})", map["value"], v);
                return;
            }
            if let Some(v) = map.get("ole_automation_days") {
                let _ = write!(out, "{} OLE date days (original value)", v);
                return;
            }
            out.push_str("<dl>");
            for (key, v) in map {
                let _ = write!(out, "<dt>{}</dt><dd>", escape(key));
                value_html(v, out);
                out.push_str("</dd>");
            }
            out.push_str("</dl>");
        }
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("<span class=muted>Empty</span>");
            } else {
                out.push_str("<ol>");
                for v in items {
                    out.push_str("<li>");
                    value_html(v, out);
                    out.push_str("</li>");
                }
                out.push_str("</ol>");
            }
        }
        Value::String(s) => {
            let _ = write!(out, "<span class=text>{}</span>", escape(s));
        }
        _ => out.push_str(&escape(&value.to_string())),
    }
}

/// Standalone local report. All imported text is escaped; there are no scripts,
/// remote fonts, links to imported paths, or active template interpretation.
pub fn render(report: &Report, source_name: &str) -> String {
    let mut out = String::from(
        "<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><meta http-equiv=Content-Security-Policy content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\"><title>Grapevine legacy inspection</title><style>body{font:16px system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;color:#20242a;background:#f8f8fa}h1,h2{line-height:1.2}aside{padding:1rem;background:#fff0d5;border-left:5px solid #986200}details{background:white;border:1px solid #bbb;border-radius:4px;margin:.6rem 0;padding:.7rem}summary{cursor:pointer;font-weight:600;overflow-wrap:anywhere}dl{display:grid;grid-template-columns:minmax(140px,1fr) 4fr;gap:.3rem .8rem}dt{font-weight:600;overflow-wrap:anywhere}dd{margin:0;min-width:0;overflow-wrap:anywhere}.text{white-space:pre-wrap}.muted{color:#666}li{margin:.35rem 0}code{overflow-wrap:anywhere}@media(max-width:650px){dl{display:block}dd{margin:.2rem 0 .8rem}}@media print{details{break-inside:avoid}summary{display:block}}</style><h1>Grapevine legacy inspection</h1>",
    );
    let _ = write!(
        out,
        "<p><strong>{}</strong></p><p>Format: {} · Version: {:?} · Result: <strong>{}</strong> · {} records</p><aside>Private inspection report: may include player contact details and storyteller secrets. This is a read-only inventory, not an approved character sheet or player-safe export.</aside>",
        escape(source_name),
        escape(&report.format),
        report.version,
        escape(&report.status),
        report.record_count()
    );
    if !report.diagnostics.is_empty() {
        out.push_str("<h2>Read results and limitations</h2><ul>");
        for d in &report.diagnostics {
            let _ = write!(
                out,
                "<li><strong>{}</strong>: {} <span class=muted>({}, {})</span></li>",
                escape(&d.severity),
                escape(&d.message),
                escape(&d.code),
                escape(&d.path)
            );
        }
        out.push_str("</ul>");
    }
    out.push_str("<details><summary>Game information and settings</summary>");
    value_html(&Value::Object(report.metadata.clone()), &mut out);
    out.push_str("</details>");
    for (section, records) in &report.sections {
        let _ = write!(out, "<h2>{} ({})</h2>", escape(section), records.len());
        for record in records {
            let kind = record
                .pointer("/fields/CharacterType")
                .and_then(Value::as_str)
                .or_else(|| record.get("type").and_then(Value::as_str))
                .unwrap_or("record");
            let _ = write!(
                out,
                "<details><summary>{} — {}</summary>",
                escape(&record_name(record)),
                escape(kind)
            );
            value_html(
                record
                    .get("fields")
                    .or_else(|| record.get("xml"))
                    .unwrap_or(record),
                &mut out,
            );
            out.push_str("</details>");
        }
    }
    if report.unparsed.is_some() {
        out.push_str("<aside>Some bytes could not be interpreted. Their location and exact bytes are retained in the JSON report; this inventory is incomplete.</aside>");
    }
    let _ = write!(
        out,
        "<p class=muted>Input unchanged by this tool. Input size: {} bytes. SHA-256: <code>{}</code></p></html>",
        report.input_bytes,
        escape(&report.sha256)
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_markup_is_inert() {
        let report=crate::inspect(b"<grapevine version=\"3\"><vampire name=\"&lt;script&gt;alert(1)&lt;/script&gt;\"><notes>&lt;img src=x onerror=alert(2)&gt;</notes></vampire></grapevine>","windows-1252");
        let html = render(&report, "<script>source</script>");
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("default-src 'none'"));
    }
}
