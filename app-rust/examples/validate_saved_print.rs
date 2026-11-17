//! Read-only print projection of a separate validation copy, never the working library.
use grapevine_workbench::{
    model::Role,
    printing::{Options, PrintScope},
    store::Library,
};
use sha2::{Digest, Sha256};
fn main() {
    let path = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("separate validation library"),
    );
    let before = std::fs::read(&path).unwrap();
    let library = Library::open(&path).unwrap();
    let revision = library.revision().unwrap();
    let mut characters = 0;
    let mut private_rows = 0;
    let mut public_rows = 0;
    let mut warned = 0;
    for item in library.summaries(Role::Storyteller).unwrap() {
        if !grapevine_workbench::sheet::is_character_kind(&item.kind) {
            continue;
        }
        let public = library
            .print_document(&item.id, &Options::default(), revision, Role::Player)
            .unwrap();
        let private = library
            .print_document(
                &item.id,
                &Options {
                    scope: PrintScope::PrivateStoryteller,
                    ..Options::default()
                },
                revision,
                Role::Storyteller,
            )
            .unwrap();
        assert!(
            public
                .sections
                .iter()
                .flat_map(|s| &s.rows)
                .all(|r| r.note.is_none())
        );
        assert!(
            private
                .sections
                .iter()
                .flat_map(|s| &s.rows)
                .all(|r| r.note.is_none())
        );
        let html = grapevine_workbench::printing::html(&public).unwrap();
        assert!(!html.to_lowercase().contains("[st]"));
        public_rows += public.sections.iter().map(|s| s.rows.len()).sum::<usize>();
        private_rows += private.sections.iter().map(|s| s.rows.len()).sum::<usize>();
        warned += usize::from(private.warnings.iter().any(|w| w.contains("unsupported")));
        characters += 1;
    }
    assert_eq!(library.revision().unwrap(), revision);
    drop(library);
    assert_eq!(
        Sha256::digest(&before),
        Sha256::digest(std::fs::read(&path).unwrap())
    );
    println!(
        "PASS read-only saved-sheet projection: {characters} characters; {public_rows} public rows; {private_rows} private rows; {warned} unsupported-shape warnings. No notes, mutation or full payload output."
    );
}
