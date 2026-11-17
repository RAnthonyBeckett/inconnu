use crate::{
    chronicle::Handover,
    model::{Language, Result, error},
    world_card::escape,
};
use std::fmt::Write;

/// Render only the deliberately scoped private handover projection.
pub fn html(handover: &Handover, language: Language) -> Result<String> {
    if (!handover.include_contacts
        && (!handover.profile.email.is_empty() || !handover.profile.phone.is_empty()))
        || (!handover.include_notes && !handover.profile.private_notes.is_empty())
        || (!handover.include_source_names
            && handover
                .sources
                .iter()
                .any(|source| !source.name.is_empty()))
        || handover.sources.len() > 1000
    {
        return Err(error(
            "HANDOVER_SCOPE",
            "Prepare a fresh handover with the chosen privacy options.",
        ));
    }
    let t = |en, pt| {
        if language == Language::Portuguese {
            pt
        } else {
            en
        }
    };
    let title = t(
        "Chronicle handover",
        "Passagem de responsabilidade da crônica",
    );
    let mut out = format!(
        "<!doctype html><html lang=\"{}\"><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{}</title><style>body{{font:16px system-ui;margin:2rem auto;max-width:62rem;padding:0 1rem}}p,td,div{{overflow-wrap:anywhere}}table{{border-collapse:collapse}}th,td{{text-align:left;padding:.3rem 1rem .3rem 0}}h2,h3{{break-after:avoid}}section{{border-top:1px solid #777}}@media print{{body{{margin:0;max-width:none}}}}</style><h1>{}</h1><p><strong>{}</strong> · {} {}</p>",
        t("en", "pt-BR"),
        escape(title),
        escape(title),
        t(
            "Private Storyteller report",
            "Relatório privado do Narrador"
        ),
        t("Library revision", "Revisão da biblioteca"),
        handover.revision,
    );
    let _ = write!(
        out,
        "<h2>{}</h2><p>{}</p>",
        escape(&handover.profile.title),
        t(
            "Local saved information and record counts. Review the library and staff arrangements separately.",
            "Informações locais salvas e contagens de registros. Revise a biblioteca e a organização da equipe separadamente."
        )
    );
    for (label, value) in [
        (t("Website", "Site"), &handover.profile.website),
        (
            t("Meeting place", "Local do encontro"),
            &handover.profile.meeting_place,
        ),
        (
            t("Meeting time", "Horário do encontro"),
            &handover.profile.meeting_time,
        ),
        (t("Description", "Descrição"), &handover.profile.description),
        (
            t(
                "House rules entered by staff",
                "Regras da casa inseridas pela equipe",
            ),
            &handover.profile.house_rules,
        ),
    ] {
        field(&mut out, label, value);
    }
    if handover.include_contacts {
        field(&mut out, t("Email", "E-mail"), &handover.profile.email);
        field(&mut out, t("Phone", "Telefone"), &handover.profile.phone);
    }
    if handover.include_notes {
        field(
            &mut out,
            t("Private staff notes", "Anotações privadas da equipe"),
            &handover.profile.private_notes,
        );
    }
    let _ = write!(
        out,
        "<h2>{}</h2><table><thead><tr><th>{}</th><th>{}</th></tr></thead><tbody>",
        t("Library contents", "Conteúdo da biblioteca"),
        t("Records", "Registros"),
        t("Count", "Quantidade")
    );
    for count in &handover.counts {
        let label = count_label(&count.key, language);
        let _ = write!(
            out,
            "<tr><td>{}</td><td>{}</td></tr>",
            escape(&label),
            count.count
        );
    }
    let _ = write!(
        out,
        "</tbody></table><h2>{}</h2>",
        t(
            "Retained source evidence",
            "Evidências de fontes preservadas"
        )
    );
    if handover.sources.is_empty() {
        let _ = write!(
            out,
            "<p>{}</p>",
            t("No retained imports.", "Nenhuma importação preservada.")
        );
    }
    for source in &handover.sources {
        let _ = write!(
            out,
            "<section><p>{}</p><p>{}</p>",
            escape(&source.format),
            escape(&source.hash)
        );
        if handover.include_source_names {
            field(
                &mut out,
                t("Source filename", "Nome do arquivo de origem"),
                &source.name,
            );
        }
        out.push_str("</section>");
        if out.len() > 8 * 1024 * 1024 {
            return Err(error(
                "HANDOVER_LIMIT",
                "This handover is too large to print.",
            ));
        }
    }
    out.push_str("</html>");
    Ok(out)
}

fn field(out: &mut String, label: &str, value: &str) {
    if !value.is_empty() {
        let _ = write!(
            out,
            "<h3>{}</h3><div style=\"white-space:pre-wrap\">{}</div>",
            escape(label),
            escape(value)
        );
    }
}

fn count_label(key: &str, language: Language) -> String {
    let (en, pt) = match key {
        "characters_total" => (
            "Characters excluding Draft/Test copies",
            "Personagens sem as cópias de rascunho/teste",
        ),
        "players" => ("Player records", "Registros de jogadores"),
        "draft_copies" => ("Draft copies", "Cópias de rascunho"),
        "test_copies" => ("Test copies", "Cópias de teste"),
        "items" => ("Items", "Itens"),
        "locations" => ("Locations", "Locais"),
        "rotes" => ("Rotes", "Rotinas"),
        "actions" => ("Actions", "Ações"),
        "rumors" => ("Rumors", "Rumores"),
        "plots" => ("Plots", "Tramas"),
        "characters_without_local_xp_opening" => (
            "Characters without a local XP opening ledger",
            "Personagens sem registro local de abertura de XP",
        ),
        "saved_searches" => ("Saved searches", "Pesquisas salvas"),
        "scheduled_plans_active" => (
            "Uncancelled scheduled plans",
            "Planos agendados não cancelados",
        ),
        "scheduled_plans_cancelled" => ("Cancelled scheduled plans", "Planos agendados cancelados"),
        "committed_sessions" => ("Committed sessions", "Sessões registradas"),
        "imports" => ("Retained imports", "Importações preservadas"),
        "audit_rows" => ("Audit entries", "Entradas de auditoria"),
        _ => {
            if let Some(kind) = key.strip_prefix("characters_kind:") {
                return format!(
                    "{}: {kind}",
                    if language == Language::Portuguese {
                        "Personagens por tipo"
                    } else {
                        "Characters by type"
                    }
                );
            }
            ("Other retained records", "Outros registros preservados")
        }
    };
    if language == Language::Portuguese {
        pt
    } else {
        en
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chronicle::{Count, Profile, Source};

    fn sample() -> Handover {
        Handover {
            revision: 9,
            profile: Profile {
                title: "Café <literal>".into(),
                house_rules: "Manual <rule>".into(),
                ..Profile::default()
            },
            include_contacts: false,
            include_notes: false,
            include_source_names: false,
            counts: vec![Count {
                key: "characters_total".into(),
                count: 12,
            }],
            sources: vec![Source {
                hash: "evidence-hash".into(),
                format: "XML <literal>".into(),
                name: String::new(),
            }],
        }
    }

    #[test]
    fn private_scope_and_html_escaping_in_both_languages() {
        for language in [Language::English, Language::Portuguese] {
            let h = sample();
            let text = html(&h, language).unwrap();
            assert!(text.contains("Café &lt;literal&gt;"));
            assert!(text.contains("Manual &lt;rule&gt;"));
            assert!(text.contains("evidence-hash"));
            assert!(!text.contains("private_notes"));
            assert!(!text.contains("<literal>"));
        }
    }

    #[test]
    fn renderer_rejects_unrequested_private_fields() {
        let mut h = sample();
        h.profile.email = "CONTACT_SECRET".into();
        assert!(html(&h, Language::English).is_err());
        h.include_contacts = true;
        assert!(
            html(&h, Language::English)
                .unwrap()
                .contains("CONTACT_SECRET")
        );
        h.profile.private_notes = "NOTE_SECRET".into();
        assert!(html(&h, Language::English).is_err());
        h.include_notes = true;
        h.sources[0].name = "FILE_SECRET".into();
        assert!(html(&h, Language::English).is_err());
        h.include_source_names = true;
        let text = html(&h, Language::English).unwrap();
        assert!(text.contains("NOTE_SECRET") && text.contains("FILE_SECRET"));
    }
}
