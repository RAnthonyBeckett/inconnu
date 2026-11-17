use grapevine_workbench::published_powers::{Catalogue, Edition};
use serde_json::json;

#[test]
fn builtins_keep_their_names_debug_output_and_wire_strings() {
    let v20 = Edition::V20;
    let da20 = Edition::DA20;
    assert_eq!(v20.as_str(), "V20");
    assert_eq!(da20.as_str(), "DA20");
    assert_eq!(format!("{v20:?}"), "V20");
    assert_eq!(format!("{da20:?}"), "DA20");
    assert_eq!(v20, Edition::from_tag("V20").unwrap());
    assert_eq!(da20, Edition::from_tag("DA20").unwrap());
    assert_eq!(serde_json::to_string(&v20).unwrap(), "\"V20\"");
    assert_eq!(serde_json::to_string(&da20).unwrap(), "\"DA20\"");
    assert_eq!(serde_json::from_str::<Edition>("\"V20\"").unwrap(), v20);
    assert_eq!(serde_json::from_str::<Edition>("\"DA20\"").unwrap(), da20);
}

#[test]
fn future_edition_tags_round_trip_without_new_variants() {
    for tag in ["W20", "WTA", "MTA", "LOTW", "OBLIVION", "LAWS_OF_ASCENSION"] {
        let edition = Edition::from_tag(tag).unwrap();
        assert_eq!(edition.as_str(), tag);
        assert_eq!(
            serde_json::from_str::<Edition>(&serde_json::to_string(&edition).unwrap()).unwrap(),
            edition
        );
    }
}

#[test]
fn edition_tags_reject_empty_control_invalid_and_oversize_values() {
    for tag in ["", "bad tag", "bad-tag", "bad\r\ntag", "é"] {
        assert!(Edition::from_tag(tag).is_none(), "accepted {tag:?}");
    }
    assert!(Edition::from_tag(&"A".repeat(49)).is_none());
    let max_tag = "A".repeat(48);
    assert_eq!(
        Edition::from_tag(&max_tag).unwrap().as_str(),
        max_tag.as_str()
    );
    assert!(serde_json::from_str::<Edition>("\"bad-tag\"").is_err());
}

#[test]
fn catalogue_accepts_custom_edition_tags_and_preserves_legacy_fact_hashes() {
    let source = json!({
        "version": 1,
        "books": [{
            "id": "v20-core",
            "title": "Synthetic V20",
            "edition": "V20",
            "pdf_sha256": "a".repeat(64),
            "coverage": "partial",
            "notes": ["Synthetic test data"]
        }],
        "entries": [{
            "id": "v20-core.discipline.example.3",
            "book_id": "v20-core",
            "name": "Example",
            "discipline": "Animalism",
            "path": null,
            "kind": "discipline",
            "level": 3,
            "printed_page": 129,
            "pdf_page": 138,
            "evidence": "index_only"
        }]
    });
    let catalogue = Catalogue::from_json(&source.to_string()).unwrap();
    assert_eq!(catalogue.book("v20-core").unwrap().edition, Edition::V20);
    assert_eq!(
        catalogue
            .reference("v20-core.discipline.example.3")
            .unwrap()
            .facts_hash,
        "70eb0eb6e5df7bb50e2bbe55d91309a10bd40c6bb6037aa5e97a0641fc5afeed"
    );

    let mut custom = source;
    custom["books"][0]["edition"] = json!("LAWS_OF_ASCENSION");
    let catalogue = Catalogue::from_json(&custom.to_string()).unwrap();
    assert_eq!(
        catalogue.book("v20-core").unwrap().edition.as_str(),
        "LAWS_OF_ASCENSION"
    );
    custom["books"][0]["edition"] = json!("bad-tag");
    assert!(Catalogue::from_json(&custom.to_string()).is_err());
}
