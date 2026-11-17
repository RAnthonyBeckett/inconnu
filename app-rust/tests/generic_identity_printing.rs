use grapevine_workbench::{
    creature_profiles,
    model::{RecordView, Role},
    power_catalogue,
    printing::{self, DocLanguage, Options},
    store::Library,
};
use serde_json::{Map, json};

#[test]
fn newborn_identity_matches_profile_and_legacy_vampire_keys_remain_exact() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("synthetic.gvlib"), "Synthetic").unwrap();
    for profile in &creature_profiles::builtin().profiles {
        let id = lib
            .create_character(
                &profile.kind,
                "Synthetic",
                lib.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap();
        let record = lib.record(&id, Role::Storyteller).unwrap();
        if profile.kind == "Vampire" {
            for field in ["Clan", "Sect", "Nature", "Demeanor"] {
                assert_eq!(record.fields[field], json!(""));
            }
            assert!(!record.fields.contains_key("Path"));
        } else {
            for field in &profile.identity_fields {
                assert_eq!(record.fields[field], json!(""));
            }
            for field in ["Clan", "Sect"] {
                if !profile.identity_fields.iter().any(|f| f == field) {
                    assert!(!record.fields.contains_key(field));
                }
            }
        }
    }
}

#[test]
fn saved_public_identity_prints_for_every_profile_in_both_languages() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::create(&dir.path().join("synthetic.gvlib"), "Synthetic").unwrap();
    for profile in &creature_profiles::builtin().profiles {
        let id = lib
            .create_character(
                &profile.kind,
                "Synthetic",
                lib.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap();
        let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
        for field in &profile.identity_fields {
            edit.identity
                .insert(field.clone(), format!("SYNTH_{field}"));
            edit.visibility.insert(field.clone(), true);
        }
        lib.save_sheet(&id, &edit, lib.revision().unwrap(), Role::Storyteller)
            .unwrap();
        for language in [DocLanguage::English, DocLanguage::Portuguese] {
            let options = Options {
                language,
                ..Default::default()
            };
            let doc = lib
                .print_document(&id, &options, lib.revision().unwrap(), Role::Player)
                .unwrap();
            let html = printing::html(&doc).unwrap();
            for field in &profile.identity_fields {
                assert!(
                    html.contains(&format!("SYNTH_{field}")),
                    "{} {field}",
                    profile.kind
                );
            }
        }
    }
}

#[test]
fn citations_follow_both_visible_target_lists_and_never_cross_creature_or_hidden_scope() {
    for profile in &creature_profiles::builtin().profiles {
        let mut rules = serde_json::to_value(creature_profiles::builtin()).unwrap();
        let row = rules["profiles"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["kind"] == profile.kind)
            .unwrap();
        row["cost_source"] = json!({"book":"Synthetic book","edition":"Fixture","system":"Tests only","printed_page":1,"pdf_page_1based":2,"original_pdf_sha256":"a".repeat(64)});
        let rules = creature_profiles::Catalogue::from_json(&rules.to_string()).unwrap();
        let mut entries = Vec::new();
        let mut fields = Map::new();
        let mut ids = Vec::new();
        for (index, list) in [&profile.primary_power_list, &profile.secondary_power_list]
            .iter()
            .enumerate()
        {
            let id = format!(
                "synthetic.{}.power{index}",
                profile.kind.to_ascii_lowercase()
            );
            let name = format!("Synthetic power {index}");
            ids.push(id.clone());
            entries.push(json!({"stable_id":id,"template_scope":profile.kind,"target_list":list,"family":"Synthetic family","branch":"Common","name":name,"tier":"Basic","purchase_family":"Other","profile":"ConfiguredCreature","printed_page":3+index,"pdf_page_1based":4+index,"original_pdf_sha256":"a".repeat(64),"prerequisite_note":"Synthetic only","source_reviewed":true}));
            fields.insert((*list).clone(),json!({"Traits":[{"Name":name,"Value":"basic","Note":"[ST]PRIVATE_TEST_NOTE[/ST]"}]}));
        }
        let powers = power_catalogue::Catalogue::from_json(
            &json!({"version":1,"entries":entries}).to_string(),
        )
        .unwrap();
        let mut view = RecordView {
            build: None,
            id: "synthetic".into(),
            kind: profile.kind.clone(),
            name: "Synthetic".into(),
            status: "Draft".into(),
            player_name: String::new(),
            notes: String::new(),
            fields,
        };
        for language in [DocLanguage::English, DocLanguage::Portuguese] {
            let options = Options {
                language,
                ..Default::default()
            };
            let doc =
                printing::document_from_catalogue(&view, &ids, &options, &powers, &rules).unwrap();
            assert_eq!(doc.references.len(), 2);
            assert!(
                doc.references
                    .iter()
                    .all(|r| r.book == "Synthetic book" && r.edition == "Fixture")
            );
            assert!(!printing::html(&doc).unwrap().contains("PRIVATE_TEST_NOTE"));
            let unverified =
                printing::document_from_catalogue(&view, &[], &options, &powers, &rules).unwrap();
            assert!(unverified.references.is_empty());
        }
        view.fields.remove(&profile.primary_power_list);
        let doc =
            printing::document_from_catalogue(&view, &ids, &Options::default(), &powers, &rules)
                .unwrap();
        assert_eq!(doc.references.len(), 1);
        assert_eq!(doc.references[0].power, "Synthetic power 1");
        view.kind = if profile.kind == "Vampire" {
            "Werewolf".into()
        } else {
            "Vampire".into()
        };
        assert!(
            printing::document_from_catalogue(&view, &ids, &Options::default(), &powers, &rules)
                .unwrap()
                .references
                .is_empty()
        );
    }
}
