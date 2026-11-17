use grapevine_workbench::safe_files::write_new;
use std::fs;
use tempfile::tempdir;

#[test]
fn publishes_complete_bytes_to_a_new_destination() {
    let directory = tempdir().expect("temporary test directory");
    let destination = directory.path().join("policy.json");

    write_new(&destination, b"{\"policy\":1}").expect("write new file");

    assert_eq!(fs::read(destination).unwrap(), b"{\"policy\":1}");
}

#[test]
fn existing_destination_is_unchanged() {
    let directory = tempdir().expect("temporary test directory");
    let destination = directory.path().join("policy.json");
    fs::write(&destination, b"existing data").unwrap();

    assert!(write_new(&destination, b"replacement data").is_err());

    assert_eq!(fs::read(destination).unwrap(), b"existing data");
}

#[test]
fn missing_parent_fails_without_creating_destination() {
    let directory = tempdir().expect("temporary test directory");
    let missing_parent = directory.path().join("missing");
    let destination = missing_parent.join("policy.json");

    assert!(write_new(&destination, b"data").is_err());

    assert!(!missing_parent.exists());
    assert!(!destination.exists());
}
