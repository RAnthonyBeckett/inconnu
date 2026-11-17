use grapevine_workbench::{importer, model::Role, store::Library};
use serde_json::{Value, json};

#[test]
fn older_world_rows_gain_editable_source_fields_without_replacing_saved_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("legacy.gvlib");
    let preview=importer::normalize(br#"<grapevine version="3"><item name="Relic" id="i1" level="2" bonus="3"><traitlist name="Abilities"><trait name="Research" val="2"/></traitlist></item></grapevine>"#,"synthetic.gex","windows-1252").unwrap();
    let r = &preview.records[0];
    let id = &r.id;
    let mut lib = Library::create(&path, "Synthetic").unwrap();
    lib.commit_import(&preview, 0, false, Role::Storyteller)
        .unwrap();
    drop(lib);
    let sql = rusqlite::Connection::open(&path).unwrap();
    let raw: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .unwrap();
    let mut old: Value = serde_json::from_str(&raw).unwrap();
    let fields = old["private_fields"].as_object_mut().unwrap();
    let ability = fields.remove("AbilityList").unwrap();
    fields.insert("UnmappedTraitLists".into(), ability.clone());
    fields.insert("Level".into(), json!("2"));
    fields.insert("Bonus".into(), json!(7));
    sql.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        [old.to_string().as_str(), id.as_str()],
    )
    .unwrap();
    let mut lib = Library::open(&path).unwrap();
    let rev = lib.revision().unwrap();
    let mut draft = lib.world_edit(id, Role::Storyteller).unwrap();
    assert_eq!(draft.fields["Level"], 2);
    assert_eq!(draft.fields["Bonus"], 7);
    assert_eq!(draft.fields["AbilityList"]["Traits"][0]["Name"], "Research");
    assert_eq!(lib.revision().unwrap(), rev);
    let unchanged: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&unchanged).unwrap(), old);
    draft.fields.insert("Level".into(), json!(4));
    lib.save_world(id, &draft, rev, Role::Storyteller).unwrap();
    let new: String = sql
        .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
        .unwrap();
    let value: Value = serde_json::from_str(&new).unwrap();
    assert_eq!(value["original"], r.original);
    assert_eq!(value["legacy_id"], "i1");
    assert_eq!(value["private_fields"]["UnmappedTraitLists"], ability);
    assert_eq!(
        lib.world_edit(id, Role::Storyteller).unwrap().fields["Level"],
        4
    );
}
