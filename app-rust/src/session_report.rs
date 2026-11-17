//! Standalone attendance/manual-award receipt; raw records and private evidence omitted.
use crate::{
    model::{Language, Result, error},
    store::SessionReceipt,
};
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn html(receipt: &SessionReceipt, language: Language) -> Result<String> {
    let q = &receipt.quote;
    if receipt.committed_revision
        != q.revision
            .checked_add(1)
            .ok_or_else(|| error("INVALID_RECEIPT", "Receipt revision exceeds bounds."))?
    {
        return Err(error(
            "INVALID_RECEIPT",
            "Session receipt revision is inconsistent.",
        ));
    }
    let pt = language == Language::Portuguese;
    let t = |en, br| if pt { br } else { en };
    let mut rows = String::new();
    for attendance in &q.request.attendance {
        let name = q
            .record_names
            .get(&attendance.record_id)
            .ok_or_else(|| error("INVALID_RECEIPT", "An attendance identity is missing."))?;
        let award = q
            .awards
            .iter()
            .find(|a| a.record_id == attendance.record_id);
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(name),
            t(
                if attendance.present {
                    "Present"
                } else {
                    "Absent"
                },
                if attendance.present {
                    "Presente"
                } else {
                    "Ausente"
                }
            ),
            award
                .map(|a| escape(&a.command.amount))
                .unwrap_or_else(|| "—".into()),
            award
                .map(|a| escape(&a.before_unspent))
                .unwrap_or_else(|| "—".into()),
            award
                .map(|a| escape(&a.after_unspent))
                .unwrap_or_else(|| "—".into())
        ));
    }
    Ok(format!(
        "<!doctype html><html lang=\"{}\"><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{}</title><style>body{{font:16px system-ui;color:#111;margin:2rem}}table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #888;padding:.45rem;text-align:left}}thead{{display:table-header-group}}tr{{break-inside:avoid}}@media print{{body{{margin:0}}}}</style><h1>{}</h1><p>{} · {}</p><p>{}</p><table><thead><tr><th>{}</th><th>{}</th><th>XP</th><th>{}</th><th>{}</th></tr></thead><tbody>{}</tbody></table><p>{}: {}</p><small>{}: {} · {}</small></html>",
        if pt { "pt-BR" } else { "en" },
        escape(&q.request.title),
        escape(&q.request.title),
        escape(&q.request.day),
        t(
            "Session attendance and awards",
            "Presença e prêmios da sessão"
        ),
        t(
            "Manual amounts declared by the local storyteller. This receipt does not verify organizational limits or approval. Historical balances are those recorded at commit.",
            "Valores manuais declarados pelo Narrador local. Este recibo não verifica limites ou aprovação da organização. Os saldos históricos são os registrados no momento da gravação."
        ),
        t("Character", "Personagem"),
        t("Attendance", "Presença"),
        t("Before", "Antes"),
        t("After", "Depois"),
        rows,
        t("Awarded characters", "Personagens premiados"),
        q.awards.len(),
        t("Receipt", "Recibo"),
        escape(&q.request.id),
        receipt.committed_revision
    ))
}
