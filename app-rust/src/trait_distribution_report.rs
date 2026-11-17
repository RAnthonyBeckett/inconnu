use crate::{
    model::{Language, Result, error},
    searches::{self, append, append_escaped},
    trait_distribution::{self, Report, Request},
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub search: searches::Snapshot,
    pub request: Request,
    pub report: Report,
    pub omitted_lists: usize,
    pub unnamed_entries: usize,
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
pub fn title(l: Language) -> &'static str {
    t(
        l,
        "Trait value distribution",
        "Distribuição de valores da característica",
    )
}
fn row(out: &mut String, values: &[&str]) -> Result<()> {
    append(out, "<tr>")?;
    for value in values {
        append(out, "<td>")?;
        append_escaped(out, value)?;
        append(out, "</td>")?;
    }
    append(out, "</tr>")
}
fn table(out: &mut String, labels: &[&str]) -> Result<()> {
    append(out, "<table><thead><tr>")?;
    for label in labels {
        append(out, "<th scope=\"col\">")?;
        append_escaped(out, label)?;
        append(out, "</th>")?;
    }
    append(out, "</tr></thead><tbody>")
}
pub fn html(snapshot: &Snapshot, language: Language) -> Result<String> {
    trait_distribution::build(&[], &snapshot.request)?;
    if snapshot.report.characters != snapshot.search.rows.len()
        || snapshot
            .report
            .matched_characters
            .checked_add(snapshot.report.missing_list_characters)
            .and_then(|n| n.checked_add(snapshot.report.unmatched_characters))
            != Some(snapshot.report.characters)
        || snapshot.report.members.len() != snapshot.report.matched_characters
    {
        return Err(error(
            "TRAIT_DISTRIBUTION_SCOPE",
            "Prepare a fresh trait distribution for this search.",
        ));
    }
    let mut out = searches::report_start(&snapshot.search, language, title(language))?;
    append(&mut out, "<h2>")?;
    append_escaped(&mut out, &snapshot.request.name)?;
    append(&mut out, "</h2><p>")?;
    append_escaped(
        &mut out,
        snapshot
            .request
            .list
            .strip_suffix("List")
            .unwrap_or(&snapshot.request.list),
    )?;
    append(&mut out, "</p><p>")?;
    append_escaped(
        &mut out,
        t(
            language,
            "Saved values only. Unknown values are not zero. Duplicate entries count as separate occurrences; a character with different values may appear in several value rows. These counts do not assess rules or approval.",
            "Somente valores salvos. Valores desconhecidos não são zero. Entradas duplicadas contam como ocorrências separadas; um personagem com valores diferentes pode aparecer em várias linhas. Estas contagens não avaliam regras nem aprovação.",
        ),
    )?;
    append(&mut out, "</p>")?;
    table(
        &mut out,
        &[
            t(language, "Scope", "Escopo"),
            t(language, "Count", "Contagem"),
        ],
    )?;
    for (label, value) in [
        (
            t(language, "Characters in search", "Personagens na busca"),
            snapshot.report.characters,
        ),
        (
            t(
                language,
                "Characters with this named trait",
                "Personagens com esta característica",
            ),
            snapshot.report.matched_characters,
        ),
        (
            t(
                language,
                "Characters without a readable selected list",
                "Personagens sem a lista selecionada legível",
            ),
            snapshot.report.missing_list_characters,
        ),
        (
            t(
                language,
                "Readable list without this named trait",
                "Lista legível sem esta característica",
            ),
            snapshot.report.unmatched_characters,
        ),
        (
            t(language, "Matching entries", "Entradas correspondentes"),
            snapshot.report.occurrences,
        ),
        (
            t(
                language,
                "Known unsigned integer entries",
                "Entradas inteiras não negativas reconhecidas",
            ),
            snapshot.report.known_occurrences,
        ),
        (
            t(
                language,
                "Unknown value entries",
                "Entradas com valor desconhecido",
            ),
            snapshot.report.unknown_occurrences,
        ),
        (
            t(
                language,
                "Unreadable source lists excluded",
                "Listas ilegíveis da origem excluídas",
            ),
            snapshot.omitted_lists,
        ),
        (
            t(
                language,
                "Unnamed entries excluded",
                "Entradas sem nome excluídas",
            ),
            snapshot.unnamed_entries,
        ),
    ] {
        row(&mut out, &[label, &value.to_string()])?;
    }
    append(&mut out, "</tbody></table><h2>")?;
    append_escaped(
        &mut out,
        t(
            language,
            "Recorded value counts",
            "Contagens dos valores registrados",
        ),
    )?;
    append(&mut out, "</h2>")?;
    table(
        &mut out,
        &[
            t(language, "Value", "Valor"),
            t(language, "Characters", "Personagens"),
            t(language, "Occurrences", "Ocorrências"),
        ],
    )?;
    for rating in &snapshot.report.ratings {
        row(
            &mut out,
            &[
                rating
                    .value
                    .as_deref()
                    .unwrap_or(t(language, "Unknown", "Desconhecido")),
                &rating.characters.to_string(),
                &rating.occurrences.to_string(),
            ],
        )?;
    }
    append(&mut out, "</tbody></table><h2>")?;
    append_escaped(
        &mut out,
        t(
            language,
            "Original matching values",
            "Valores correspondentes originais",
        ),
    )?;
    append(&mut out, "</h2>")?;
    table(
        &mut out,
        &[
            t(language, "Character", "Personagem"),
            t(language, "Family", "Família"),
            t(
                language,
                "Original recorded values",
                "Valores originais registrados",
            ),
        ],
    )?;
    for member in &snapshot.report.members {
        let values = member
            .values
            .iter()
            .map(|s| {
                if s.is_empty() {
                    t(language, "(empty)", "(vazio)")
                } else {
                    s
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        row(&mut out, &[&member.name, &member.family, &values])?;
    }
    append(&mut out, "</tbody></table>")?;
    if snapshot.search.include_players {
        append(&mut out, "<h2>")?;
        append_escaped(
            &mut out,
            t(
                language,
                "Filtered roster with requested Player names",
                "Elenco filtrado com nomes de Jogadores solicitados",
            ),
        )?;
        append(&mut out, "</h2>")?;
        searches::append_search_rows(&mut out, &snapshot.search, language)?;
    }
    append(&mut out, "</body></html>")?;
    Ok(out)
}
