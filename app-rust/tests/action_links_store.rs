use grapevine_workbench::{
    action_links::{self, CopyOptions},
    actions::{Action, ActivitySlot},
    importer,
    model::{BuildKind, Edit as RecordEdit, Role},
    store::Library,
    story_work::Draft,
};
use rusqlite::Connection;
use serde_json::{Value, json};

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

const LEGACY_XML: &[u8] = br#"<?xml version="1.0"?><grapevine version="3">
  <action date="2026-11-01" character="Twin" done="no" lastmodified="2026-11-01">
    <subaction name="Contacts" level="2" unused="7" total="9" growth="4">
      <act>Ask around</act><results>Learned a name</results><unknown preserve="yes">UNKNOWN_ACTION_DATA</unknown>
    </subaction>
  </action>
</grapevine>"#;

fn slot(name: &str, unused: &str, growth: &str) -> ActivitySlot {
    ActivitySlot {
        name: name.into(),
        level: "2".into(),
        unused: unused.into(),
        total: "9".into(),
        growth: growth.into(),
        action: "Ask around".into(),
        result: "Learned a name".into(),
    }
}

fn action(character: &str, day: &str) -> Action {
    Action {
        character: character.into(),
        day: day.into(),
        slots: vec![slot("Contacts", "7", "4")],
    }
}

fn raw_record(path: &std::path::Path, id: &str) -> Value {
    let sql = Connection::open(path).unwrap();
    let data: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    serde_json::from_str(&data).unwrap()
}

fn import_legacy_action(lib: &mut Library, expected: i64) -> (String, Value, String) {
    let preview = importer::normalize(LEGACY_XML, "action-synthetic.gex", "windows-1252").unwrap();
    let source = preview
        .records
        .iter()
        .find(|record| record.kind == "Action")
        .unwrap()
        .clone();
    lib.commit_import(&preview, expected, false, ST).unwrap();
    (source.id, source.original, source.source_sha256)
}

fn audit_ops(path: &std::path::Path, record_id: &str) -> Vec<String> {
    let sql = Connection::open(path).unwrap();
    let mut statement = sql
        .prepare("SELECT operation FROM audit WHERE record_id=?1 ORDER BY revision")
        .unwrap();
    statement
        .query_map([record_id], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn explicit_ids_disambiguate_duplicate_labels_and_old_story_saves_keep_the_link() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("links.gvlib");
    let mut lib = Library::create(&path, "Synthetic links").unwrap();
    let first = lib.create_character("Vampire", "Twin", 0, ST).unwrap();
    let second = lib.create_character("Vampire", "Twin", 1, ST).unwrap();
    let first_action = lib
        .create_linked_action(&action("Twin", "2026-11-01"), Some(&first), 2, ST)
        .unwrap();
    let second_action = lib
        .create_linked_action(&action("Twin", "2026-11-01"), Some(&second), 3, ST)
        .unwrap();

    let linked = lib.linked_action_snapshot(&first_action, ST).unwrap();
    let binding = linked.binding.unwrap();
    assert_eq!(binding.binding.character_id, first);
    assert_eq!(binding.binding.label_at_binding, "Twin");
    assert_eq!(binding.current_name.as_deref(), Some("Twin"));
    assert_eq!(
        lib.action_timeline(&first, "2026-11-01", "2026-11-01", ST)
            .unwrap()
            .rows
            .iter()
            .map(|row| row.action_id.as_str())
            .collect::<Vec<_>>(),
        [first_action.as_str()]
    );
    assert_eq!(
        lib.action_timeline(&second, "2026-11-01", "2026-11-01", ST)
            .unwrap()
            .rows
            .iter()
            .map(|row| row.action_id.as_str())
            .collect::<Vec<_>>(),
        [second_action.as_str()]
    );

    let revision = lib.revision().unwrap();
    lib.save_edit(
        &first,
        &RecordEdit {
            name: "Renamed Twin".into(),
            status: "Active".into(),
            notes: String::new(),
        },
        revision,
        ST,
    )
    .unwrap();
    let record_before_read = raw_record(&path, &first_action);
    let after_rename = lib.linked_action_snapshot(&first_action, ST).unwrap();
    let Draft::Action(projected_story) = &after_rename.story.draft else {
        panic!("Expected projected Action draft")
    };
    assert_eq!(projected_story.character, "Renamed Twin");
    let binding = after_rename.binding.as_ref().unwrap();
    assert_eq!(binding.current_name.as_deref(), Some("Renamed Twin"));
    assert_eq!(binding.binding.label_at_binding, "Twin");
    let timeline = lib
        .action_timeline(&first, "2026-11-01", "2026-11-01", ST)
        .unwrap();
    let Draft::Action(projected_timeline_action) = &timeline.rows[0].snapshot.draft else {
        panic!("Expected projected timeline Action")
    };
    assert_eq!(projected_timeline_action.character, "Renamed Twin");
    assert_eq!(raw_record(&path, &first_action), record_before_read);

    let snapshot = lib.story_snapshot(&first_action, ST).unwrap();
    let Draft::Action(mut existing) = snapshot.draft else {
        panic!("Expected Action draft")
    };
    existing.slots[0].result = "Saved through the older entrypoint".into();
    lib.save_story(
        &first_action,
        &Draft::Action(existing),
        snapshot.revision,
        ST,
    )
    .unwrap();
    assert_eq!(
        lib.linked_action_snapshot(&first_action, ST)
            .unwrap()
            .binding
            .unwrap()
            .binding
            .character_id,
        first
    );

    let unlinked = lib
        .create_story(
            &Draft::Action(action("Twin", "2026-11-02")),
            lib.revision().unwrap(),
            ST,
        )
        .unwrap();
    assert!(
        lib.linked_action_snapshot(&unlinked, ST)
            .unwrap()
            .binding
            .is_none()
    );
}

#[test]
fn legacy_name_never_auto_links_and_explicit_binding_preserves_source_payload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy-link.gvlib");
    let mut lib = Library::create(&path, "Synthetic legacy").unwrap();
    let _first = lib.create_character("Vampire", "Twin", 0, ST).unwrap();
    let selected = lib.create_character("Vampire", "Twin", 1, ST).unwrap();
    let (action_id, original, source_hash) = import_legacy_action(&mut lib, 2);
    let old = lib.linked_action_snapshot(&action_id, ST).unwrap();
    assert!(old.binding.is_none());
    let Draft::Action(legacy) = old.story.draft else {
        panic!("Expected legacy Action")
    };
    assert_eq!(legacy.character, "Twin");
    assert!(old.story.legacy && old.story.retained);

    lib.save_linked_action(&action_id, &legacy, Some(&selected), old.story.revision, ST)
        .unwrap();
    let linked = lib.linked_action_snapshot(&action_id, ST).unwrap();
    let binding = linked.binding.unwrap();
    assert_eq!(binding.binding.character_id, selected);
    assert_eq!(binding.binding.label_at_binding, "Twin");
    assert_eq!(binding.current_name.as_deref(), Some("Twin"));

    let saved = raw_record(&path, &action_id);
    assert_eq!(saved["original"], original);
    assert_eq!(saved["source_sha256"], source_hash);
    assert!(
        saved["original"]
            .to_string()
            .contains("UNKNOWN_ACTION_DATA")
    );
    let imported: Vec<u8> = Connection::open(&path)
        .unwrap()
        .query_row("SELECT source_bytes FROM imports", [], |row| row.get(0))
        .unwrap();
    assert_eq!(imported, LEGACY_XML);
}

#[test]
fn timeline_is_exact_id_scoped_inclusive_and_reports_unlinked_and_undated_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("timeline.gvlib");
    let mut lib = Library::create(&path, "Synthetic timeline").unwrap();
    let first = lib.create_character("Vampire", "Twin", 0, ST).unwrap();
    let second = lib.create_character("Vampire", "Twin", 1, ST).unwrap();
    let before = lib
        .create_linked_action(&action("Twin", "2026-10-31"), Some(&first), 2, ST)
        .unwrap();
    let from_a = lib
        .create_linked_action(&action("Twin", "2026-11-01"), Some(&first), 3, ST)
        .unwrap();
    let from_b = lib
        .create_linked_action(&action("Twin", "2026-11-01"), Some(&first), 4, ST)
        .unwrap();
    let middle = lib
        .create_linked_action(&action("Twin", "2026-11-02"), Some(&first), 5, ST)
        .unwrap();
    let through = lib
        .create_linked_action(&action("Twin", "2026-11-03"), Some(&first), 6, ST)
        .unwrap();
    let after = lib
        .create_linked_action(&action("Twin", "2026-11-04"), Some(&first), 7, ST)
        .unwrap();
    let other_twin = lib
        .create_linked_action(&action("Twin", "2026-11-02"), Some(&second), 8, ST)
        .unwrap();
    let unlinked = lib
        .create_story(&Draft::Action(action("Twin", "2026-11-02")), 9, ST)
        .unwrap();
    let undated = lib
        .create_linked_action(&action("Twin", "2026-11-02"), Some(&first), 10, ST)
        .unwrap();

    // Synthetic preexisting record state: an established link with an unresolved source date.
    let mut raw = raw_record(&path, &undated);
    raw["private_fields"]["AppActionV1"]["day"] = json!("source-date-unknown");
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![raw.to_string(), undated],
        )
        .unwrap();

    let timeline = lib
        .action_timeline(&first, "2026-11-01", "2026-11-03", ST)
        .unwrap();
    assert_eq!(timeline.character_id, first);
    assert_eq!(timeline.character_name, "Twin");
    assert_eq!(timeline.undated_omitted, 1);
    assert_eq!(timeline.unlinked_in_library, 1);
    let ids: Vec<_> = timeline
        .rows
        .iter()
        .map(|row| row.action_id.as_str())
        .collect();
    assert_eq!(ids.len(), 4);
    assert!(ids.contains(&from_a.as_str()));
    assert!(ids.contains(&from_b.as_str()));
    assert!(ids.contains(&middle.as_str()));
    assert!(ids.contains(&through.as_str()));
    assert!(!ids.contains(&before.as_str()));
    assert!(!ids.contains(&after.as_str()));
    assert!(!ids.contains(&other_twin.as_str()));
    assert!(!ids.contains(&unlinked.as_str()));
    assert!(!ids.contains(&undated.as_str()));
    assert!(timeline.rows.windows(2).all(|pair| {
        let left = &pair[0];
        let right = &pair[1];
        left.snapshot
            .draft
            .day()
            .cmp(right.snapshot.draft.day())
            .then(left.action_id.cmp(&right.action_id))
            .is_le()
    }));
    assert_eq!(
        lib.action_timeline(&first, "2026-11-01", "2026-11-03", PLAYER)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert!(
        lib.action_timeline(&first, "2026-11-04", "2026-11-01", ST)
            .is_err()
    );
}

#[test]
fn copy_preview_requires_explicit_same_character_earlier_source_and_applies_only_options() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("copy-preview.gvlib");
    let mut lib = Library::create(&path, "Synthetic copy preview").unwrap();
    let character = lib.create_character("Vampire", "Twin", 0, ST).unwrap();
    let other = lib.create_character("Vampire", "Twin", 1, ST).unwrap();
    let source_action = Action {
        character: "Historical label".into(),
        day: "2026-11-01".into(),
        slots: vec![slot("Contacts", "7", "4"), slot("Influence", "2", "6")],
    };
    let source_id = lib
        .create_linked_action(&source_action, Some(&character), 2, ST)
        .unwrap();
    let other_id = lib
        .create_linked_action(&action("Twin", "2026-11-01"), Some(&other), 3, ST)
        .unwrap();
    let unlinked_id = lib
        .create_story(&Draft::Action(action("Twin", "2026-11-01")), 4, ST)
        .unwrap();
    let revision = lib.revision().unwrap();
    let options = CopyOptions {
        carry_unused: true,
        carry_growth: false,
    };
    let preview = lib
        .preview_action_copy(&character, &source_id, "2026-11-02", &options, ST)
        .unwrap();
    assert_eq!(preview.revision, revision);
    assert_eq!(preview.character_id, character);
    assert_eq!(preview.source_id, source_id);
    assert_eq!(preview.source_day, "2026-11-01");
    assert_eq!(preview.day, "2026-11-02");
    assert_eq!(preview.draft.character, "Twin");
    assert_eq!(preview.draft.day, "2026-11-02");
    assert_eq!(preview.draft.slots.len(), 2);
    assert_eq!(preview.draft.slots[0].name, "Contacts");
    assert_eq!(preview.draft.slots[0].level, "2");
    assert_eq!(preview.draft.slots[0].total, "9");
    assert_eq!(preview.draft.slots[0].unused, "7");
    assert_eq!(preview.draft.slots[0].growth, "0");
    assert_eq!(preview.draft.slots[0].action, "");
    assert_eq!(preview.draft.slots[0].result, "");
    assert_eq!(preview.draft.slots[1].unused, "2");
    assert_eq!(preview.draft.slots[1].growth, "0");

    let growth_only = lib
        .preview_action_copy(
            &character,
            &source_id,
            "2026-11-03",
            &CopyOptions {
                carry_unused: false,
                carry_growth: true,
            },
            ST,
        )
        .unwrap();
    assert_eq!(growth_only.draft.slots[0].unused, "0");
    assert_eq!(growth_only.draft.slots[0].growth, "4");
    assert_eq!(lib.revision().unwrap(), revision, "preview does not write");
    assert_eq!(
        lib.preview_action_copy(&character, &source_id, "2026-11-01", &options, ST)
            .unwrap_err()
            .code,
        "ACTION_COPY_DATE_ORDER"
    );
    assert_eq!(
        lib.preview_action_copy(&character, &other_id, "2026-11-02", &options, ST)
            .unwrap_err()
            .code,
        "ACTION_COPY_SOURCE_MISMATCH"
    );
    assert_eq!(
        lib.preview_action_copy(&character, &unlinked_id, "2026-11-02", &options, ST)
            .unwrap_err()
            .code,
        "ACTION_COPY_SOURCE_UNLINKED"
    );
    assert_eq!(
        lib.preview_action_copy(&character, &source_id, "invalid-date", &options, ST)
            .unwrap_err()
            .code,
        "ACTION_COPY_DATE_INVALID"
    );
    assert_eq!(
        lib.preview_action_copy(&character, &source_id, "2026-11-02", &options, PLAYER)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
}

#[test]
fn invalid_targets_stale_writes_and_player_access_leave_revision_and_audit_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("denials.gvlib");
    let mut lib = Library::create(&path, "Synthetic denial checks").unwrap();
    let character = lib.create_character("Vampire", "Target", 0, ST).unwrap();
    let player = lib.create_player("Player target", 1, ST).unwrap();
    let build = lib
        .create_build(&character, BuildKind::Draft, 2, ST)
        .unwrap();
    let linked = lib
        .create_linked_action(&action("Target", "2026-11-01"), Some(&character), 3, ST)
        .unwrap();
    let stale = lib.revision().unwrap() - 1;
    let revision = lib.revision().unwrap();
    let action_record = lib
        .create_story(&Draft::Action(action("Target", "2026-11-02")), revision, ST)
        .unwrap();
    let revision = lib.revision().unwrap();
    for invalid_target in [
        "missing-character",
        player.as_str(),
        build.as_str(),
        action_record.as_str(),
    ] {
        assert_eq!(
            lib.create_linked_action(
                &action("Target", "2026-11-03"),
                Some(invalid_target),
                revision,
                ST,
            )
            .unwrap_err()
            .code,
            "ACTION_LINK_TARGET_REQUIRED"
        );
    }
    assert_eq!(
        lib.create_linked_action(&action("Target", "2026-11-03"), Some(&character), stale, ST,)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    let invalid_action = action("Target", "not-a-date");
    assert_eq!(
        lib.save_linked_action(&linked, &invalid_action, Some(&character), revision, ST)
            .unwrap_err()
            .code,
        "ACTION_DATE_INVALID"
    );
    assert_eq!(
        lib.save_linked_action(
            &linked,
            &action("Target", "2026-11-02"),
            Some(&player),
            revision,
            ST
        )
        .unwrap_err()
        .code,
        "ACTION_LINK_TARGET_REQUIRED"
    );
    assert_eq!(
        lib.linked_action_snapshot(&linked, PLAYER)
            .unwrap_err()
            .code,
        "ACCESS_DENIED"
    );
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(audit_ops(&path, &linked), ["create_linked_action"]);
    assert_eq!(
        lib.linked_action_snapshot(&linked, ST)
            .unwrap()
            .story
            .revision,
        revision
    );
}

#[test]
fn unchanged_missing_target_can_be_retained_or_cleared_and_backup_restore_recovers_binding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recovery.gvlib");
    let backup = dir.path().join("recovery-backup.gvlib");
    let mut lib = Library::create(&path, "Synthetic recovery").unwrap();
    let character = lib
        .create_character("Vampire", "Original name", 0, ST)
        .unwrap();
    let linked = lib
        .create_linked_action(
            &action("Historical action label", "2026-11-01"),
            Some(&character),
            1,
            ST,
        )
        .unwrap();

    // Future metadata on a binding must survive a same-target save.
    let mut raw = raw_record(&path, &linked);
    raw["private_fields"][action_links::FIELD]["future_binding"] = json!({"keep":true});
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            rusqlite::params![raw.to_string(), linked],
        )
        .unwrap();
    let story = lib.story_snapshot(&linked, ST).unwrap();
    let Draft::Action(existing) = story.draft else {
        panic!("Expected Action")
    };
    lib.save_linked_action(&linked, &existing, Some(&character), story.revision, ST)
        .unwrap();
    assert_eq!(
        raw_record(&path, &linked)["private_fields"][action_links::FIELD]["future_binding"]["keep"],
        true
    );
    assert_eq!(
        lib.linked_action_snapshot(&linked, ST)
            .unwrap()
            .binding
            .unwrap()
            .binding
            .label_at_binding,
        "Original name"
    );
    lib.backup(&backup, ST).unwrap();

    Connection::open(&path)
        .unwrap()
        .execute("DELETE FROM records WHERE id=?1", [&character])
        .unwrap();
    let record_before_missing_read = raw_record(&path, &linked);
    let missing = lib.linked_action_snapshot(&linked, ST).unwrap();
    let binding = missing.binding.unwrap();
    assert_eq!(binding.current_name, None);
    assert_eq!(binding.binding.label_at_binding, "Original name");
    let Draft::Action(projected_missing) = &missing.story.draft else {
        panic!("Expected projected Action draft")
    };
    assert_eq!(projected_missing.character, "Original name");
    assert_eq!(raw_record(&path, &linked), record_before_missing_read);

    let saved_snapshot = lib.story_snapshot(&linked, ST).unwrap();
    let Draft::Action(existing) = saved_snapshot.draft else {
        panic!("Expected Action")
    };
    lib.save_linked_action(
        &linked,
        &existing,
        Some(&character),
        saved_snapshot.revision,
        ST,
    )
    .unwrap();
    assert_eq!(
        lib.linked_action_snapshot(&linked, ST)
            .unwrap()
            .binding
            .unwrap()
            .current_name,
        None
    );
    lib.save_linked_action(&linked, &existing, None, lib.revision().unwrap(), ST)
        .unwrap();
    assert!(
        lib.linked_action_snapshot(&linked, ST)
            .unwrap()
            .binding
            .is_none()
    );

    let restored_revision = lib.restore(&backup, lib.revision().unwrap(), ST).unwrap();
    drop(lib);
    let lib = Library::open(&path).unwrap();
    let restored = lib.linked_action_snapshot(&linked, ST).unwrap();
    assert_eq!(restored.story.revision, restored_revision);
    assert_eq!(
        restored.binding.unwrap().current_name.as_deref(),
        Some("Original name")
    );
    assert_eq!(
        raw_record(&path, &linked)["private_fields"][action_links::FIELD]["future_binding"]["keep"],
        true
    );
}
