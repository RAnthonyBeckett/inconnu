use grapevine_workbench::importer::redact_custom;
use serde_json::json;

#[test]
fn ascii_custom_markers_ignore_case_around_unicode_text() {
    assert_eq!(
        redact_custom(&json!("Pré🙂{sT}HIDDEN{/sT} café"), "{ST}", "{/ST}"),
        "Pré🙂 café"
    );
}

#[test]
fn ascii_custom_markers_keep_malformed_blocks_fail_closed() {
    assert_eq!(
        redact_custom(&json!("safe{st}secret"), "{ST}", "{/ST}"),
        "safe"
    );
    assert_eq!(
        redact_custom(&json!("ambiguous {/st} tail"), "{ST}", "{/ST}"),
        ""
    );
}

#[test]
fn non_ascii_custom_markers_remain_case_sensitive() {
    assert_eq!(
        redact_custom(&json!("visible<é>secret</é> tail"), "<é>", "</é>"),
        "visible tail"
    );
    assert_eq!(
        redact_custom(&json!("visible<É>text</É> tail"), "<é>", "</é>"),
        "visible<É>text</É> tail"
    );
}
