use grapevine_workbench::model::{BuildKind, RecordView};
use grapevine_workbench::printing::{self, DocLanguage, Options, PrintScope, Reference};
use grapevine_workbench::purchase_preview::AnimalismPower;
use serde_json::{Map, json};

fn view(kind: &str) -> RecordView {
    RecordView {
        build: None,
        id: "record-1".into(),
        kind: kind.into(),
        name: "Lúcia".into(),
        status: "Active".into(),
        player_name: "Example Player".into(),
        notes: String::new(),
        fields: Map::new(),
    }
}

fn values(document: &printing::SheetDocument) -> Vec<String> {
    document
        .sections
        .iter()
        .flat_map(|section| {
            section
                .rows
                .iter()
                .flat_map(|row| {
                    [
                        Some(row.name.clone()),
                        Some(row.value.clone()),
                        row.note.clone(),
                    ]
                    .into_iter()
                    .flatten()
                })
                .chain(std::iter::once(section.title.clone()))
        })
        .collect()
}

#[test]
fn scope_and_options_control_trait_storyteller_and_temporary_notes() {
    let mut source = view("Vampire");
    source.notes = "[ST]private Chronicle clue[/ST]public note".into();
    source
        .fields
        .insert("Clan".into(), json!("[ST]secret[/ST]Tremere"));
    source.fields.insert("Blood".into(), json!("3"));
    source.fields.insert("TempBlood".into(), json!("2"));
    source.fields.insert(
        "DisciplineList".into(),
        json!({"Traits":[{"Name":"Auspex","Value":"2","Note":"[ST]trait clue[/ST]"}]}),
    );

    let mut public_options = Options {
        include_notes: true,
        ..Options::default()
    };
    let public = printing::document(&source, &[], &public_options).unwrap();
    let public_text = serde_json::to_string(&public).unwrap();
    assert!(!public.options.include_notes);
    assert!(!public_text.contains("private Chronicle clue"));
    assert!(!public_text.contains("public note"));
    assert!(!public_text.contains("trait clue"));
    assert!(!public_text.contains("secret"));
    assert!(
        values(&public)
            .iter()
            .any(|value| value == "Temporary Blood")
    );
    assert!(values(&public).iter().any(|value| value == "2"));

    public_options.include_temporary = false;
    let without_temporary = printing::document(&source, &[], &public_options).unwrap();
    assert!(
        !values(&without_temporary)
            .iter()
            .any(|value| value == "Temporary Blood")
    );

    let private = printing::document(
        &source,
        &[],
        &Options {
            scope: PrintScope::PrivateStoryteller,
            include_notes: true,
            ..Options::default()
        },
    )
    .unwrap();
    let private_text = serde_json::to_string(&private).unwrap();
    assert!(private_text.contains("private Chronicle clue"));
    assert!(private_text.contains("trait clue"));
    assert!(private_text.contains("[ST]secret[/ST]Tremere"));
}

#[test]
fn supported_character_kinds_are_bounded_and_unsupported_shapes_are_omitted() {
    let kinds = [
        "Vampire",
        "Werewolf",
        "Mortal",
        "Changeling",
        "Wraith",
        "Mage",
        "Fera",
        "Various",
        "Mummy",
        "KueiJin",
        "Hunter",
        "Demon",
        "Risen",
    ];
    for kind in kinds {
        assert!(
            printing::document(&view(kind), &[], &Options::default()).is_ok(),
            "{kind}"
        );
    }
    let err = printing::document(&view("UnknownKind"), &[], &Options::default()).unwrap_err();
    assert_eq!(err.code, "PRINT_UNSUPPORTED_CHARACTER");

    let mut source = view("Vampire");
    source
        .fields
        .insert("AbilityList".into(), json!("unsupported list"));
    source
        .fields
        .insert("Blood".into(), json!(["unsupported resource"]));
    source
        .fields
        .insert("Email".into(), json!("private@example.invalid"));
    source
        .fields
        .insert("XpHistory".into(), json!(["must never print"]));
    source
        .fields
        .insert("_AppPurchase".into(), json!({"private":"metadata"}));
    source
        .fields
        .insert("UnknownSecret".into(), json!("ignored entirely"));
    let fields_before = source.fields.clone();
    let notes_before = source.notes.clone();
    let doc = printing::document(&source, &[], &Options::default()).unwrap();
    let serialized = serde_json::to_string(&doc).unwrap();
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.contains("unsupported"))
    );
    assert!(!serialized.contains("unsupported list"));
    assert!(!serialized.contains("unsupported resource"));
    assert!(!serialized.contains("private@example"));
    assert!(!serialized.contains("XpHistory"));
    assert!(!serialized.contains("must never print"));
    assert!(!serialized.contains("_AppPurchase"));
    assert!(!serialized.contains("metadata"));
    assert!(!serialized.contains("UnknownSecret"));
    assert_eq!(source.fields, fields_before);
    assert_eq!(source.notes, notes_before);
}

#[test]
fn references_need_a_verified_source_id_and_the_exact_owned_power_name() {
    let mut source = view("Vampire");
    source.fields.insert(
        "DisciplineList".into(),
        json!({"Traits":[{"Name":"Heightened Senses","Value":"1"}]}),
    );
    let id = AnimalismPower::HeightenedSenses.stable_id().to_string();
    let no_verification = printing::document(&source, &[], &Options::default()).unwrap();
    assert!(no_verification.references.is_empty());
    let verified =
        printing::document(&source, std::slice::from_ref(&id), &Options::default()).unwrap();
    assert_eq!(verified.references.len(), 1);
    let reference = &verified.references[0];
    assert_eq!(reference.family, "Auspex");
    assert_eq!(reference.power, "Heightened Senses");
    assert_eq!((reference.printed_page, reference.pdf_page), (137, 138));
    assert!(
        reference
            .library_prompt
            .contains("Laws of the Night (Revised)")
    );

    source.fields.insert(
        "DisciplineList".into(),
        json!({"Traits":[{"Name":"Aura Perception","Value":"1"}]}),
    );
    assert!(
        printing::document(&source, std::slice::from_ref(&id), &Options::default())
            .unwrap()
            .references
            .is_empty()
    );
    source.kind = "Werewolf".into();
    source.fields.insert(
        "DisciplineList".into(),
        json!({"Traits":[{"Name":"Heightened Senses","Value":"1"}]}),
    );
    assert!(
        printing::document(&source, &[id], &Options::default())
            .unwrap()
            .references
            .is_empty()
    );
}

#[test]
fn html_escapes_every_dynamic_value_and_stays_self_contained_for_printing() {
    let mut source = view("Vampire");
    source.name = "Mara</title><script>alert(\"x\")</script> & Coração".into();
    source.player_name = "<img src=x onerror=alert(1)>".into();
    source.fields.insert(
        "DisciplineList".into(),
        json!({"Traits":[{"Name":"<b>Auspex</b>","Value":"<svg onload=x>","Note":"quote ' & <tag>"}]}),
    );
    let mut doc = printing::document(
        &source,
        &[],
        &Options {
            scope: PrintScope::PrivateStoryteller,
            include_notes: true,
            large_text: true,
            ..Options::default()
        },
    )
    .unwrap();
    doc.references.push(Reference {
        family: "<family>".into(),
        power: "Power & name".into(),
        book: "<img>".into(),
        edition: "Revised & checked".into(),
        printed_page: 137,
        pdf_page: 138,
        library_prompt: "Use <your> library & source".into(),
    });
    doc.warnings
        .push("<script>alert('warning')</script>".into());
    let html = printing::html(&doc).unwrap();
    let lower = html.to_ascii_lowercase();
    assert!(html.contains("Coração"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(html.contains("&lt;svg onload=x&gt;"));
    assert!(html.contains("&amp;"));
    assert!(html.contains("&#39;"));
    assert!(html.contains("@media print"));
    assert!(html.contains("@page{size:A4"));
    assert!(html.contains("font-size:16pt"));
    assert!(html.contains("default-src 'none'; style-src 'unsafe-inline'"));
    assert!(lower.contains("private storyteller copy"));
    assert!(!lower.contains("<script"));
    assert!(!lower.contains("<img"));
    assert!(!lower.contains("<svg"));
    assert!(!lower.contains("href="));
    assert!(!lower.contains("<img src="));
}

#[test]
fn portuguese_labels_watermark_and_generic_warnings_do_not_expose_input_fields() {
    let mut source = view("Risen");
    source.build = Some(BuildKind::Draft);
    source
        .fields
        .insert("AbilityList".into(), json!("bad secret shape"));
    source
        .fields
        .insert("UnknownPrivateField".into(), json!("private payload"));
    let options = Options {
        language: DocLanguage::Portuguese,
        scope: PrintScope::PrivateStoryteller,
        include_references: false,
        include_temporary: false,
        ..Options::default()
    };
    let doc = printing::document(&source, &[], &options).unwrap();
    assert!(
        doc.sections
            .iter()
            .any(|section| section.title == "Identidade")
    );
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.contains("estrutura não compatível"))
    );
    assert!(
        doc.warnings
            .iter()
            .any(|warning| warning.contains("omitidas pelas opções"))
    );
    let serialized = serde_json::to_string(&doc).unwrap();
    assert!(!serialized.contains("UnknownPrivateField"));
    assert!(!serialized.contains("private payload"));
    assert!(!serialized.contains("bad secret shape"));
    let html = printing::html(&doc).unwrap();
    assert!(html.contains("CÓPIA PRIVADA DO NARRADOR"));
    assert!(html.contains("CÓPIA DE RASCUNHO"));
    assert!(html.contains("NÃO IMPLICA APROVAÇÃO"));
    assert!(html.contains("referências de fonte"));
}
