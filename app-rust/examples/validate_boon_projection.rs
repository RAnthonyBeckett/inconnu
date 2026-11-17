use grapevine_workbench::{boons, importer};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide read-only source")?,
    );
    let before = std::fs::read(&path)?;
    let preview = importer::normalize(&before, "private-source.gex", "windows-1252")?;
    let mut holders = 0usize;
    let mut entries = 0usize;
    let mut uncertain_dates = 0usize;
    let mut uncertain_directions = 0usize;
    for record in preview
        .records
        .iter()
        .filter(|record| record.kind == "Vampire" && record.build.is_none())
    {
        holders += 1;
        let encoded = serde_json::to_vec(record)?;
        let projected = boons::legacy(record)?;
        entries += projected.len();
        uncertain_dates += projected.iter().filter(|entry| entry.day.is_none()).count();
        uncertain_directions += projected
            .iter()
            .filter(|entry| entry.owed_by_holder.is_none())
            .count();
        assert_eq!(encoded, serde_json::to_vec(record)?);
        assert_eq!(projected, boons::legacy(record)?);
    }
    assert_eq!(before, std::fs::read(&path)?);
    println!(
        "PASS Boon projection: {holders} Vampire holders, {entries} entries, {uncertain_dates} uncertain dates, {uncertain_directions} uncertain directions; records and source unchanged."
    );
    Ok(())
}
