use grapevine_workbench::{
    associations::{self, Kind, Link},
    importer,
    model::{BuildKind, Language, Role},
    store::Library,
    world,
};
use serde_json::{Value, json};

const ST: Role = Role::Storyteller;
const PLAYER: Role = Role::Player;

fn link(id: &str, kind: Kind, quantity: u16) -> Link {
    Link {
        target_id: id.into(),
        kind,
        label_at_binding: "Untrusted input label".into(),
        quantity,
        share_challenge_bonus: false,
        note: "PRIVATE_ASSOCIATION_NOTE".into(),
    }
}
fn world_record(name: &str, field: &str, text: &str) -> world::Edit {
    world::Edit {
        name: name.into(),
        fields: std::collections::BTreeMap::from([(field.into(), json!(text))]),
    }
}

#[test]
fn assigns_exact_ids_without_reconciling_duplicate_or_legacy_labels() {
    let bytes=br#"<grapevine version="3"><mage name="Archivist" id="m1"><traitlist name="Equipment"><trait name="Ring &lt;literal&gt;" val="3" note="PRIVATE_LEGACY_NOTE"/></traitlist><traitlist name="Rotes"><trait name="A legacy rote"/></traitlist><traitlist name="Locations"><trait name="An old hangout"/></traitlist></mage></grapevine>"#;
    let preview = importer::normalize(bytes, "synthetic.gex", "windows-1252").unwrap();
    let c = &preview.records[0];
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.gvlib");
    let backup = dir.path().join("synthetic.backup");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    lib.commit_import(&preview, 0, false, ST).unwrap();
    let other = lib
        .create_world(
            "Item",
            &world_record("Ring <literal>", "Appearance", "Wrong record"),
            1,
            ST,
        )
        .unwrap();
    let item = lib
        .create_world(
            "Item",
            &world_record("Ring <literal>", "Appearance", "Chosen record"),
            2,
            ST,
        )
        .unwrap();
    let rote = lib
        .create_world(
            "Rote",
            &world_record("Rote", "Description", "A description"),
            3,
            ST,
        )
        .unwrap();
    let place = lib
        .create_world(
            "Location",
            &world_record("Local place", "Appearance", "A known place"),
            4,
            ST,
        )
        .unwrap();
    let initial = lib.association_snapshot(&c.id, ST).unwrap();
    assert_eq!(initial.legacy.len(), 3);
    assert!(
        initial.linked.is_empty(),
        "Never match a legacy label automatically"
    );
    assert!(lib.association_snapshot(&c.id, PLAYER).is_err());
    let chosen = link(&item, Kind::Item, 2);
    let links = [
        chosen.clone(),
        link(&rote, Kind::Rote, 1),
        link(&place, Kind::Location, 1),
    ];
    assert!(lib.save_associations(&c.id, &links, 5, PLAYER).is_err());
    assert!(lib.save_associations(&c.id, &links, 4, ST).is_err());
    assert!(
        lib.save_associations(&c.id, &[link(&item, Kind::Rote, 1)], 5, ST)
            .is_err()
    );
    assert!(
        lib.save_associations(&c.id, &[chosen.clone(), chosen], 5, ST)
            .is_err()
    );
    assert_eq!(lib.revision().unwrap(), 5);
    lib.save_associations(&c.id, &links, 5, ST).unwrap();
    let s = lib.association_snapshot(&c.id, ST).unwrap();
    assert_eq!(s.linked[0].link.target_id, item);
    assert_ne!(s.linked[0].link.target_id, other);
    assert_eq!(s.linked[0].link.label_at_binding, "Ring <literal>");
    assert_eq!(s.legacy.len(), 3);
    assert_eq!(
        s.legacy
            .iter()
            .find(|r| r.kind == Kind::Item)
            .unwrap()
            .value,
        "3"
    );
    let html = associations::html(&s, Language::English, false).unwrap();
    assert!(html.contains("Chosen record") && html.contains("&lt;literal&gt;"));
    assert!(!html.contains("Wrong record") && !html.contains(&item) && !html.contains("PRIVATE_"));
    let pt = associations::html(&s, Language::Portuguese, true).unwrap();
    assert!(pt.contains("PRIVATE_ASSOCIATION_NOTE") && pt.contains("PRIVATE_LEGACY_NOTE"));
    assert!(
        !serde_json::to_string(&lib.record(&c.id, PLAYER).unwrap())
            .unwrap()
            .contains("PRIVATE_ASSOCIATION_NOTE")
    );
    lib.save_world(
        &item,
        &world_record("Renamed ring", "Appearance", "Chosen record"),
        6,
        ST,
    )
    .unwrap();
    let renamed = lib.association_snapshot(&c.id, ST).unwrap();
    assert_eq!(renamed.linked[0].link.label_at_binding, "Ring <literal>");
    assert_eq!(
        renamed.linked[0].current.as_ref().unwrap().name,
        "Renamed ring"
    );
    let draft = lib.create_build(&c.id, BuildKind::Draft, 7, ST).unwrap();
    assert!(lib.association_snapshot(&draft, ST).is_err());
    assert!(lib.save_associations(&draft, &links, 8, ST).is_err());
    let sql = rusqlite::Connection::open(&path).unwrap();
    let raw = |id: &str| {
        let d: String = sql
            .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
            .unwrap();
        serde_json::from_str::<Value>(&d).unwrap()
    };
    assert_eq!(raw(&c.id)["original"], c.original);
    assert!(
        raw(&draft)["private_fields"]
            .get("AppAssociationsV1")
            .is_none()
    );
    let old: Vec<u8> = sql
        .query_row("SELECT source_bytes FROM imports", [], |r| r.get(0))
        .unwrap();
    assert_eq!(old, bytes);
    drop(sql);
    lib.backup(&backup, ST).unwrap();
    lib.save_associations(&c.id, &[], 8, ST).unwrap();
    assert!(
        lib.association_snapshot(&c.id, ST)
            .unwrap()
            .linked
            .is_empty()
    );
    assert!(lib.world_edit(&item, ST).is_ok());
    drop(lib);
    let mut lib = Library::open(&path).unwrap();
    lib.restore(&backup, 9, ST).unwrap();
    assert_eq!(lib.association_snapshot(&c.id, ST).unwrap().linked.len(), 3);
}

#[test]
fn missing_targets_remain_evidence_and_are_never_rebound_by_label() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.gvlib");
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    let c = lib.create_character("Vampire", "Character", 0, ST).unwrap();
    let old = lib
        .create_world("Item", &world_record("Relic", "Appearance", "Old"), 1, ST)
        .unwrap();
    lib.save_associations(&c, &[link(&old, Kind::Item, 1)], 2, ST)
        .unwrap();
    let replacement = lib
        .create_world("Item", &world_record("Relic", "Appearance", "Other"), 3, ST)
        .unwrap();
    // Simulate a missing target in a disposable database. No normal target deletion yet.
    let sql = rusqlite::Connection::open(&path).unwrap();
    sql.execute("DELETE FROM records WHERE id=?1", [&old])
        .unwrap();
    drop(sql);
    let s = lib.association_snapshot(&c, ST).unwrap();
    assert_eq!(s.linked[0].link.target_id, old);
    assert_ne!(s.linked[0].link.target_id, replacement);
    assert!(s.linked[0].current.is_none());
    let unchanged = s.linked[0].link.clone();
    let mut edited = unchanged.clone();
    edited.quantity = 2;
    assert!(lib.save_associations(&c, &[edited], 4, ST).is_err());
    lib.save_associations(&c, &[unchanged], 4, ST).unwrap();
    lib.save_associations(&c, &[], 5, ST).unwrap();
    assert!(lib.world_edit(&replacement, ST).is_ok());
}
