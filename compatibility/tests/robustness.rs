use grapevine_legacy_reader::inspect;

#[test]
fn arbitrary_bytes_never_panic_or_claim_complete() {
    let mut state = 0x1234_5678_u32;
    for n in 0..1200 {
        let mut bytes = vec![0; (n % 256) as usize];
        for b in &mut bytes {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            *b = (state >> 24) as u8;
        }
        let report = inspect(&bytes, "windows-1252");
        assert_ne!(report.status, "complete");
        assert!(!report.diagnostics.is_empty());
    }
}

#[test]
fn unsupported_bytes_are_retained_exactly() {
    let bytes = b"\x04\0GVBE\0\0\0\0\0\0\x10\x40secret payload";
    let report = inspect(bytes, "windows-1252");
    assert_eq!(report.status, "unsupported");
    assert_eq!(report.unparsed.as_ref().unwrap()["offset"], 0);
    assert_eq!(report.unparsed.as_ref().unwrap()["length"], bytes.len());
}

#[test]
fn invalid_encoding_is_not_silently_accepted() {
    let report = inspect(b"anything", "made-up-encoding");
    assert_eq!(report.status, "invalid");
    assert_eq!(report.diagnostics[0].code, "UNKNOWN_ENCODING");
}

#[test]
fn doctype_text_in_comments_or_cdata_is_not_a_dtd() {
    let input = b"<grapevine version=\"3\"><!-- <!DOCTYPE example> --><vampire name=\"Example\"><notes><![CDATA[<!DOCTYPE text>]]></notes></vampire></grapevine>";
    let report = inspect(input, "windows-1252");
    assert_eq!(report.status, "complete");
    assert!(report.raw_xml.unwrap().contains("<!DOCTYPE text>"));
}
