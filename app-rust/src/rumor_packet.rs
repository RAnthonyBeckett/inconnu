use crate::{
    model::{Language, Result, error},
    rumor_audience::Snapshot,
    story_work::{self, Draft},
    world_card::escape,
};
use std::fmt::Write;

/// A private staff review packet, never a delivery receipt or recipient permission.
pub fn html(snapshot: &Snapshot, language: Language) -> Result<String> {
    if snapshot.recipients.len() > 500 {
        return Err(error(
            "RUMOR_PACKET_LIMIT",
            "Narrow the audience before printing.",
        ));
    }
    let t = |en, pt| {
        if language == Language::Portuguese {
            pt
        } else {
            en
        }
    };
    let mut out = story_work::header(
        t(
            "Rumor audience review packet",
            "Pacote de revisão do público do rumor",
        ),
        language,
        snapshot.revision,
    );
    let unresolved = snapshot
        .recipients
        .iter()
        .filter(|r| r.current_name.is_none())
        .count();
    let _ = write!(
        out,
        "<p><strong>{}</strong></p><p>{}: {} · {}: {}</p>",
        t(
            "Local staff review only. This list grants no access and records no sending or receipt.",
            "Somente revisão local da equipe. Esta lista não concede acesso nem registra envio ou recebimento."
        ),
        t(
            "Selected character records",
            "Registros de personagens selecionados"
        ),
        snapshot.recipients.len(),
        t("Unresolved saved links", "Vínculos salvos não resolvidos"),
        unresolved
    );
    if snapshot.recipients.is_empty() {
        let _ = write!(
            out,
            "<p>{}</p>",
            t("No characters selected.", "Nenhum personagem selecionado.")
        );
    } else {
        out.push_str("<ul>");
        for recipient in &snapshot.recipients {
            let label = recipient
                .current_name
                .as_deref()
                .unwrap_or(&recipient.binding.label_at_binding);
            let suffix: String = recipient.binding.character_id.chars().take(8).collect();
            let _ = write!(
                out,
                "<li>{} · {}{}</li>",
                escape(label),
                escape(&suffix),
                if recipient.current_name.is_none() {
                    t(" — unresolved saved link", " — vínculo salvo não resolvido")
                } else {
                    ""
                }
            );
            if out.len() > 8 * 1024 * 1024 {
                return Err(error(
                    "RUMOR_PACKET_LIMIT",
                    "Narrow the audience before printing.",
                ));
            }
        }
        out.push_str("</ul>");
    }
    story_work::section(
        &mut out,
        &story_work::Snapshot {
            revision: snapshot.revision,
            draft: Draft::Rumor(snapshot.rumor.clone()),
            legacy: snapshot.legacy,
            retained: snapshot.retained,
        },
        language,
    );
    if out.len() > 8 * 1024 * 1024 {
        return Err(error(
            "RUMOR_PACKET_LIMIT",
            "Narrow this packet before printing.",
        ));
    }
    out.push_str("</html>");
    Ok(out)
}
