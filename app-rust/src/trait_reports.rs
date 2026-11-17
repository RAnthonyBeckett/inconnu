use crate::{
    model::{Language, Result},
    searches,
    trait_statistics::{Kind, Report},
};
use searches::{append, append_escaped};

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub search: searches::Snapshot,
    pub kind: Kind,
    pub report: Report,
    pub omitted_lists: usize,
    pub unnamed_entries: usize,
}
fn t(language: Language, english: &'static str, portuguese: &'static str) -> &'static str {
    if language == Language::Portuguese {
        portuguese
    } else {
        english
    }
}
pub fn title(kind: Kind, language: Language) -> &'static str {
    match kind {
        Kind::Influence => t(language, "Influence report", "Relatório de influências"),
        Kind::MeritsFlaws => t(
            language,
            "Merits and Flaws report",
            "Relatório de méritos e defeitos",
        ),
    }
}
fn list_title(list: &str, language: Language) -> &'static str {
    match list {
        "InfluenceList" => t(language, "Influences", "Influências"),
        "MeritList" => t(language, "Merits", "Méritos"),
        _ => t(language, "Flaws", "Defeitos"),
    }
}
fn table(out: &mut String, headers: &[&str]) -> Result<()> {
    append(out, "<table><thead><tr>")?;
    for header in headers {
        append(out, "<th scope=\"col\">")?;
        append_escaped(out, header)?;
        append(out, "</th>")?;
    }
    append(out, "</tr></thead><tbody>")
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
pub fn html(snapshot: &Snapshot, language: Language) -> Result<String> {
    let mut out =
        searches::report_start(&snapshot.search, language, title(snapshot.kind, language))?;
    let unavailable = t(language, "Unavailable", "Indisponível");
    append(&mut out, "<section><h2>")?;
    append_escaped(&mut out, title(snapshot.kind, language))?;
    append(&mut out, "</h2><p>")?;
    append_escaped(
        &mut out,
        t(
            language,
            "Saved trait values are listed below. Numeric subtotals use recognized unsigned whole values. Unparsed values remain listed.",
            "Os valores salvos estão listados abaixo. Os subtotais numéricos usam valores inteiros não negativos reconhecidos. Valores não interpretados permanecem listados.",
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
    row(
        &mut out,
        &[
            t(
                language,
                "Matched characters",
                "Personagens correspondentes",
            ),
            &snapshot.report.characters.to_string(),
        ],
    )?;
    row(
        &mut out,
        &[
            t(
                language,
                "Distinct named traits by list",
                "Características distintas por lista",
            ),
            &snapshot.report.buckets.len().to_string(),
        ],
    )?;
    row(
        &mut out,
        &[
            t(
                language,
                "Unreadable selected lists excluded",
                "Listas selecionadas ilegíveis excluídas",
            ),
            &snapshot.omitted_lists.to_string(),
        ],
    )?;
    row(
        &mut out,
        &[
            t(
                language,
                "Unnamed entries excluded",
                "Entradas sem nome excluídas",
            ),
            &snapshot.unnamed_entries.to_string(),
        ],
    )?;
    append(&mut out, "</tbody></table></section>")?;
    if snapshot.report.buckets.is_empty() {
        append(&mut out, "<p>")?;
        append_escaped(
            &mut out,
            t(
                language,
                "No named traits in the recognized selected lists.",
                "Nenhuma característica nomeada nas listas selecionadas reconhecidas.",
            ),
        )?;
        append(&mut out, "</p>")?;
    }
    for bucket in &snapshot.report.buckets {
        append(&mut out, "<section><h2>")?;
        append_escaped(&mut out, list_title(&bucket.list, language))?;
        append(&mut out, " — ")?;
        append_escaped(&mut out, &bucket.name)?;
        append(&mut out, "</h2>")?;
        let percent = if snapshot.report.characters == 0 {
            0.0
        } else {
            100.0 * bucket.characters as f64 / snapshot.report.characters as f64
        };
        let percent = format!("{percent:.1}%");
        let percent = if language == Language::Portuguese {
            percent.replace('.', ",")
        } else {
            percent
        };
        table(
            &mut out,
            &[
                t(language, "Measure", "Medida"),
                t(language, "Recorded value", "Valor registrado"),
            ],
        )?;
        for (label, value) in [
            (
                t(language, "Characters", "Personagens"),
                bucket.characters.to_string(),
            ),
            (
                t(
                    language,
                    "Percent of matched characters",
                    "Percentual dos personagens correspondentes",
                ),
                percent,
            ),
            (
                t(
                    language,
                    "Entries including duplicates",
                    "Entradas incluindo duplicadas",
                ),
                bucket.occurrences.to_string(),
            ),
            (
                t(
                    language,
                    "Known numeric entries",
                    "Entradas numéricas reconhecidas",
                ),
                bucket.known_numeric.to_string(),
            ),
            (
                t(language, "Unparsed values", "Valores não interpretados"),
                bucket.unknown_numeric.to_string(),
            ),
            (
                t(
                    language,
                    "Known numeric subtotal",
                    "Subtotal numérico reconhecido",
                ),
                if bucket.known_sum.is_empty() {
                    unavailable.into()
                } else {
                    bucket.known_sum.clone()
                },
            ),
            (
                t(language, "Known maximum", "Máximo reconhecido"),
                bucket
                    .known_max
                    .clone()
                    .unwrap_or_else(|| unavailable.into()),
            ),
        ] {
            row(&mut out, &[label, &value])?;
        }
        append(&mut out, "</tbody></table>")?;
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
        for member in &bucket.members {
            let values = member
                .values
                .iter()
                .map(|value| {
                    if value.is_empty() {
                        t(language, "(empty)", "(vazio)")
                    } else {
                        value.as_str()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            row(&mut out, &[&member.name, &member.family, &values])?;
        }
        append(&mut out, "</tbody></table></section>")?;
    }
    append(&mut out, "</body></html>")?;
    Ok(out)
}
