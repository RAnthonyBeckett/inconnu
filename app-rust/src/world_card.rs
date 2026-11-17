//! Storyteller-only descriptive cards. No rules approval or player disclosure is inferred.
use crate::{
    model::{Language, Result, error},
    world,
};
use serde_json::Value;
pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn label(key: &str, language: Language) -> &str {
    let (en, pt) = match key {
        "Item" => ("Item", "Item"),
        "Location" => ("Location", "Local"),
        "Rote" => ("Rote", "Rotina"),
        "ItemType" => ("Item type", "Tipo de item"),
        "ItemSubtype" => ("Subtype", "Subtipo"),
        "DamageType" => ("Damage type", "Tipo de dano"),
        "Concealability" => ("Concealability", "Ocultabilidade"),
        "Powers" => ("Powers", "Poderes"),
        "Appearance" => ("Appearance", "Aparência"),
        "Notes" => ("Private notes", "Notas privadas"),
        "Level" => ("Level", "Nível"),
        "Bonus" => ("Bonus", "Bônus"),
        "DamageAmount" => ("Damage", "Dano"),
        "Duration" => ("Duration", "Duração"),
        "Description" => ("Description", "Descrição"),
        "Grades" => ("Grades", "Graus"),
        "LocType" => ("Location type", "Tipo de local"),
        "Owner" => ("Owner (text)", "Proprietário (texto)"),
        "Where" => ("Where", "Onde"),
        "Access" => ("Access", "Acesso"),
        "Security" => ("Security", "Segurança"),
        "Affinity" => ("Affinity", "Afinidade"),
        "Totem" => ("Totem", "Totem"),
        "Umbra" => ("Umbra", "Umbra"),
        "SecTraits" => ("Security traits", "Características de segurança"),
        "SecRetests" => ("Security retests", "Retestes de segurança"),
        "Gauntlet" => ("Gauntlet", "Película"),
        "TemperList" => ("Traits", "Características"),
        "NegativeList" => ("Negative traits", "Características negativas"),
        "AbilityList" => ("Abilities", "Habilidades"),
        "Availability" => ("Availability", "Disponibilidade"),
        "SphereList" => ("Spheres", "Esferas"),
        "LinkList" => (
            "Links (legacy traits)",
            "Vínculos (características legadas)",
        ),
        _ => (key, key),
    };
    if language == Language::English {
        en
    } else {
        pt
    }
}
pub fn html(
    kind: &str,
    edit: &world::Edit,
    language: Language,
    include_notes: bool,
) -> Result<String> {
    if !world::is_world_kind(kind) {
        return Err(error("WORLD_SCOPE", "Choose an item, location or rote."));
    }
    let pt = language == Language::Portuguese;
    let t = |en, br| if pt { br } else { en };
    let mut body = String::from("<dl>");
    for key in world::text_fields(kind)
        .iter()
        .chain(world::integer_fields(kind))
    {
        if *key == "Notes" && !include_notes {
            continue;
        }
        if let Some(value) = edit.fields.get(*key) {
            let text = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => continue,
            };
            body.push_str(&format!(
                "<dt>{}</dt><dd>{}</dd>",
                escape(label(key, language)),
                escape(&text)
            ));
        }
    }
    body.push_str("</dl>");
    for key in world::list_fields(kind) {
        if let Some(list) = edit
            .fields
            .get(*key)
            .filter(|v| crate::sheet::editable_list(v))
        {
            body.push_str(&format!(
                "<h2>{}</h2><table><thead><tr><th>{}</th><th>{}</th>{}</tr></thead><tbody>",
                escape(label(key, language)),
                t("Name", "Nome"),
                t("Value", "Valor"),
                if include_notes {
                    format!("<th>{}</th>", t("Private note", "Nota privada"))
                } else {
                    String::new()
                }
            ));
            for row in list["Traits"].as_array().unwrap() {
                body.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td>{}</tr>",
                    escape(row["Name"].as_str().unwrap_or_default()),
                    escape(row["Value"].as_str().unwrap_or_default()),
                    if include_notes {
                        format!(
                            "<td>{}</td>",
                            escape(row["Note"].as_str().unwrap_or_default())
                        )
                    } else {
                        String::new()
                    }
                ));
            }
            body.push_str("</tbody></table>");
        }
    }
    Ok(format!(
        "<!doctype html><html lang=\"{}\"><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{}</title><style>body{{font:16px system-ui;margin:2rem;color:#111}}h1{{overflow-wrap:anywhere}}dd{{white-space:pre-wrap;margin-bottom:1em;overflow-wrap:anywhere}}dt{{font-weight:bold}}table{{width:100%;border-collapse:collapse;table-layout:fixed}}td,th{{border:1px solid #888;padding:.4em;overflow-wrap:anywhere}}thead{{display:table-header-group}}tr{{break-inside:avoid}}@media print{{body{{margin:0}}}}</style><p><strong>{}</strong></p><h1>{}</h1><p>{}</p><p>{}</p>{}</html>",
        if pt { "pt-BR" } else { "en" },
        escape(&edit.name),
        t("Private Storyteller card", "Cartão privado do Narrador"),
        escape(&edit.name),
        escape(label(kind, language)),
        t(
            "Descriptive local record. Values do not verify rules, ownership or organizational approval. Unsupported fields and source metadata are omitted.",
            "Registro descritivo local. Os valores não verificam regras, propriedade ou aprovação da organização. Campos não compatíveis e metadados de origem foram omitidos."
        ),
        body
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn cards_escape_literals_omit_unknowns_and_private_notes_by_default() {
        let edit=world::Edit{name:"Café <script>alert(1)</script>".into(),fields:serde_json::from_value(json!({"Powers":"<b>Literal</b>","Notes":"PRIVATE_SECRET","SourceSecret":"UNKNOWN_SECRET","AbilityList":{"Traits":[{"Name":"Skill","Value":"2","Note":"ROW_SECRET","unknown":"NESTED_SECRET"}]}})).unwrap()};
        for language in [Language::English, Language::Portuguese] {
            let html = html("Item", &edit, language, false).unwrap();
            assert!(html.contains("&lt;script&gt;") && html.contains("&lt;b&gt;Literal&lt;/b&gt;"));
            assert!(!html.contains("<script>") && !html.contains("SECRET"));
            let private = super::html("Item", &edit, language, true).unwrap();
            assert!(
                private.contains("PRIVATE_SECRET")
                    && private.contains("ROW_SECRET")
                    && !private.contains("UNKNOWN_SECRET")
                    && !private.contains("NESTED_SECRET")
            );
        }
    }
    #[test]
    fn all_three_kinds_and_only_correct_fields() {
        for kind in ["Item", "Location", "Rote"] {
            let edit = world::Edit {
                name: "Synthetic".into(),
                ..Default::default()
            };
            assert!(
                html(kind, &edit, Language::Portuguese, false)
                    .unwrap()
                    .contains("Cartão privado")
            );
        }
        assert!(html("Vampire", &world::Edit::default(), Language::English, false).is_err());
    }
}
