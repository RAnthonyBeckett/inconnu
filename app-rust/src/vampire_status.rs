use crate::{
    model::{Language, Result},
    searches::{self, append, append_escaped},
};

#[derive(Clone, Debug)]
pub struct Trait {
    pub name: String,
    pub value: String,
}
#[derive(Clone, Debug)]
pub struct Boon {
    pub kind: String,
    pub partner: String,
    pub owed_by_holder: Option<bool>,
    pub day: Option<String>,
    pub description: String,
}
#[derive(Clone, Debug)]
pub struct Character {
    pub name: String,
    pub title: String,
    pub group: String,
    pub statuses: Vec<Trait>,
    pub unreadable_status: bool,
    pub boons: Vec<Boon>,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub search: searches::Snapshot,
    pub include_boons: bool,
    pub include_descriptions: bool,
    pub characters: Vec<Character>,
}
fn t(l: Language, en: &'static str, pt: &'static str) -> &'static str {
    if l == Language::Portuguese { pt } else { en }
}
fn cells(out: &mut String, values: &[&str]) -> Result<()> {
    append(out, "<tr>")?;
    for value in values {
        append(out, "<td>")?;
        append_escaped(out, value)?;
        append(out, "</td>")?;
    }
    append(out, "</tr>")
}
pub fn html(snapshot: &Snapshot, l: Language) -> Result<String> {
    let mut out = searches::report_start(
        &snapshot.search,
        l,
        t(
            l,
            "Vampire Status report",
            "Relatório de status de Vampiros",
        ),
    )?;
    let unavailable = t(l, "Unavailable", "Indisponível");
    for character in &snapshot.characters {
        append(&mut out, "<section><h2>")?;
        append_escaped(&mut out, &character.name)?;
        append(&mut out, "</h2>")?;
        if !character.title.is_empty() || !character.group.is_empty() {
            append(&mut out, "<p>")?;
            append_escaped(&mut out, &character.title)?;
            if !character.title.is_empty() && !character.group.is_empty() {
                append(&mut out, " · ")?;
            }
            append_escaped(&mut out, &character.group)?;
            append(&mut out, "</p>")?;
        }
        append(&mut out, "<h3>")?;
        append_escaped(
            &mut out,
            t(
                l,
                "Recorded status entries",
                "Entradas de status registradas",
            ),
        )?;
        append(&mut out, "</h3>")?;
        if character.unreadable_status {
            append(&mut out, "<p>")?;
            append_escaped(
                &mut out,
                t(
                    l,
                    "The retained status list requires review.",
                    "A lista de status preservada requer revisão.",
                ),
            )?;
            append(&mut out, "</p>")?;
        }
        append(&mut out, "<table><thead><tr><th>")?;
        append_escaped(&mut out, t(l, "Status", "Status"))?;
        append(&mut out, "</th><th>")?;
        append_escaped(&mut out, t(l, "Recorded value", "Valor registrado"))?;
        append(&mut out, "</th></tr></thead><tbody>")?;
        for entry in &character.statuses {
            cells(&mut out, &[&entry.name, &entry.value])?;
        }
        if character.statuses.is_empty() {
            cells(
                &mut out,
                &[
                    if character.unreadable_status {
                        unavailable
                    } else {
                        t(l, "None recorded", "Nenhuma entrada registrada")
                    },
                    "",
                ],
            )?;
        }
        append(&mut out, "</tbody></table>")?;
        if snapshot.include_boons {
            append(&mut out, "<h3>")?;
            append_escaped(&mut out, t(l, "Recorded Boons", "Favores registrados"))?;
            append(&mut out, "</h3><table><thead><tr>")?;
            for header in [
                t(l, "Type", "Tipo"),
                t(l, "Partner", "Contraparte"),
                t(l, "Direction", "Direção"),
                t(l, "Date", "Data"),
            ] {
                append(&mut out, "<th>")?;
                append_escaped(&mut out, header)?;
                append(&mut out, "</th>")?;
            }
            if snapshot.include_descriptions {
                append(&mut out, "<th>")?;
                append_escaped(&mut out, t(l, "Private description", "Descrição privada"))?;
                append(&mut out, "</th>")?;
            }
            append(&mut out, "</tr></thead><tbody>")?;
            for boon in &character.boons {
                let direction = match boon.owed_by_holder {
                    Some(true) => t(l, "Holder owes partner", "Titular deve à contraparte"),
                    Some(false) => t(l, "Partner owes holder", "Contraparte deve ao titular"),
                    None => unavailable,
                };
                let mut values = vec![
                    if boon.kind.trim().is_empty() {
                        unavailable
                    } else {
                        boon.kind.as_str()
                    },
                    if boon.partner.trim().is_empty() {
                        unavailable
                    } else {
                        boon.partner.as_str()
                    },
                    direction,
                    boon.day.as_deref().unwrap_or(unavailable),
                ];
                if snapshot.include_descriptions {
                    values.push(&boon.description);
                }
                cells(&mut out, &values)?;
            }
            if character.boons.is_empty() {
                let columns = if snapshot.include_descriptions { 5 } else { 4 };
                cells(
                    &mut out,
                    &std::iter::once(t(l, "No active Boons", "Nenhum favor ativo"))
                        .chain(std::iter::repeat_n("", columns - 1))
                        .collect::<Vec<_>>(),
                )?;
            }
            append(&mut out, "</tbody></table>")?;
        }
        append(&mut out, "</section>")?;
    }
    if snapshot.characters.is_empty() {
        append(&mut out, "<p>")?;
        append_escaped(
            &mut out,
            t(
                l,
                "No matching saved Vampires.",
                "Nenhum Vampiro salvo correspondente.",
            ),
        )?;
        append(&mut out, "</p>")?;
    }
    append(&mut out, "</body></html>")?;
    Ok(out)
}
