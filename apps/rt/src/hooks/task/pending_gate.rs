//! `pending_gate` — a cobrança de fim de turno: o turno em que uma unidade
//! fechou não termina com uma mensagem que omite uma pendência aberta.
//!
//! ## O caso que a fez nascer
//!
//! Em 09/09/2026 três trabalhos foram combinados na ordem 2 → 3 → 1. Os dois
//! primeiros viraram pull requests e, no mesmo turno do último merge, o resumo
//! "O dia fechou assim" listou duas pendências e omitiu o terceiro trabalho. O
//! operador só descobriu no dia seguinte, perguntando. Nenhum gancho conferia o
//! texto final do assistente.
//!
//! ## Quando cobra — todos os fatos precisam valer
//!
//! 1. É o `Stop` da sessão principal (nunca o de um subagente) e não é a segunda
//!    passada de um bloqueio (`stop_hook_active`).
//! 2. Uma unidade FECHOU neste turno: a sessão carrega a marca que o escritor de
//!    eventos grava ao registrar `pipeline.complete` ou `pr.merged`
//!    ([`take_unit_closed`]). A marca é consumida aqui, então só o primeiro
//!    `Stop` depois do fechamento — o fim do turno em que ele aconteceu — a vê.
//! 3. O `Stop` trouxe `last_assistant_message` (o texto final do turno, campo
//!    documentado do evento). Sem ele não há o que conferir.
//! 4. Alguma pendência aberta não é citada nesse texto, pelo id (`P-3`) ou pelo
//!    título, sem diferenciar maiúsculas.
//!
//! Qualquer fato que falte libera o turno.
//!
//! ## Por que só no turno do fechamento
//!
//! É o momento exato da perda original. Cobrar em todo turno obrigaria cada
//! resposta curta a recitar a lista — e um aviso que sempre dispara aprende-se a
//! ignorar.
//!
//! ## Uma vez por turno
//!
//! Dois limites independentes, como no `stop_gate`: `stop_hook_active` marca a
//! passada que o próprio bloqueio provocou, e a marca consumida garante o mesmo
//! sem depender de o host mandar esse campo. O Claude Code também força a parada
//! depois de 8 bloqueios seguidos, mas aqui nunca se chega ao segundo.
//!
//! ## Sem modo `MUSTARD_*_MODE`
//!
//! Como o `stop_gate` e o `crystallise_nudge`: a trava não ganha porta de
//! configuração. Ela já se restringe sozinha ao turno do fechamento, e desligá-la
//! devolveria exatamente a perda que ela existe para impedir.

use crate::commands::event::pending::{format_pending_items, open_pending, OpenPending};
use crate::shared::context::take_unit_closed;
use mustard_core::domain::model::contract::{Check, Ctx, HookInput, Trigger, Verdict};
use mustard_core::platform::error::Error;
use serde_json::Value;
use std::path::Path;

/// A cobrança de pendências no `Stop`.
pub struct PendingGate;

impl Check for PendingGate {
    fn evaluate(&self, input: &HookInput, ctx: &Ctx) -> Result<Verdict, Error> {
        // Fato 1 — o `Stop` da sessão principal, na primeira passada.
        if ctx.trigger != Some(Trigger::Stop) || input.is_subagent() {
            return Ok(Verdict::Allow);
        }
        if input.raw.get("stop_hook_active").and_then(Value::as_bool) == Some(true) {
            return Ok(Verdict::Allow);
        }
        let project_dir = ctx.project_dir_or_cwd(input);
        let session = input.session_id.as_deref().unwrap_or_default();

        // Fato 2 — uma unidade fechou neste turno. Consumida ANTES de qualquer
        // outra leitura: o turno terminou, cobrado ou não, e o próximo não herda
        // um fechamento que não é dele.
        if !take_unit_closed(&project_dir, session) {
            return Ok(Verdict::Allow);
        }

        // Fato 3 — o texto final do turno.
        let Some(message) = input.raw.get("last_assistant_message").and_then(Value::as_str)
        else {
            return Ok(Verdict::Allow);
        };

        // Fato 4 — alguma pendência aberta ficou de fora.
        let omitted: Vec<OpenPending> = open_pending(Path::new(&project_dir))
            .into_iter()
            .filter(|item| !cites(message, item))
            .collect();
        if omitted.is_empty() {
            return Ok(Verdict::Allow);
        }
        Ok(Verdict::Deny { reason: block_reason(&omitted) })
    }
}

/// `true` quando `message` cita `item` pelo id ou pelo título, sem diferenciar
/// maiúsculas e sem ligar para quebras de linha no meio do título.
fn cites(message: &str, item: &OpenPending) -> bool {
    let text = normalize(message);
    if mentions_id(&text, &item.id.to_lowercase()) {
        return true;
    }
    let title = normalize(&item.title);
    !title.is_empty() && text.contains(&title)
}

/// Minúsculas, espaços colapsados — a forma em que texto e título se comparam.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// `id` aparece em `text` como um id inteiro: `p-1` não conta dentro de `p-10`
/// nem de `xp-1`, senão citar uma pendência bastaria para cobrir outra.
fn mentions_id(text: &str, id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    text.match_indices(id).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + id.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

/// O motivo do bloqueio: nomeia CADA pendência omitida — sem corte, porque o
/// próximo passo é citá-las todas — e diz as duas saídas honestas.
fn block_reason(omitted: &[OpenPending]) -> String {
    format!(
        "[Mustard] A unit closed in this turn, and the final message does not name {count} \
         open pending item(s): {items}. Agreed work outlives the unit that closed — rewrite \
         the closing message naming each one by id or title. An item that no longer stands \
         leaves the list only with a reason: `mustard-rt run pending --close <id> --reason \
         \"…\"` (delivered) or `mustard-rt run pending --drop <id> --reason \"…\"` (given up).",
        count = omitted.len(),
        items = format_pending_items(omitted, omitted.len()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::event::pending::{pending_at, PendingOpts};
    use crate::shared::context::mark_unit_closed;
    use serde_json::json;
    use tempfile::tempdir;

    fn ctx(dir: &Path) -> Ctx {
        Ctx {
            project_dir: dir.to_string_lossy().into_owned(),
            trigger: Some(Trigger::Stop),
            workspace_root: None,
            inject_only: None,
        }
    }

    /// Um `Stop` da sessão principal com o texto final do turno.
    fn stop(session: &str, message: &str) -> HookInput {
        HookInput {
            hook_event_name: Some("Stop".to_string()),
            session_id: Some(session.to_string()),
            raw: json!({ "last_assistant_message": message }),
            ..HookInput::default()
        }
    }

    /// Um projeto instalado com duas pendências abertas: P-1 "Humanize" e
    /// P-2 "HTML padrao da spec".
    fn project_with_two_open_items() -> tempfile::TempDir {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("mustard.json"), r#"{"lang":"pt-BR"}"#).expect("cfg");
        for (title, detail) in [("Humanize", "terceiro trabalho"), ("HTML padrao da spec", "segundo")] {
            let out = pending_at(&PendingOpts {
                root: root.to_path_buf(),
                add: true,
                title: Some(title.into()),
                detail: Some(detail.into()),
                close: None,
                drop: None,
                reason: None,
            });
            assert_eq!(out["ok"], json!(true), "seed: {out}");
        }
        dir
    }

    fn verdict(root: &Path, input: &HookInput) -> Verdict {
        PendingGate.evaluate(input, &ctx(root)).expect("no error")
    }

    /// AC-5 — o turno em que uma unidade fechou e cuja mensagem final omite uma
    /// pendência aberta é bloqueado, e o motivo NOMEIA a omitida (e só ela). A
    /// cobrança acontece uma vez: o `Stop` seguinte do mesmo turno passa.
    #[test]
    fn pending_gate_blocks_closing_turn_that_omits_open_item() {
        let dir = project_with_two_open_items();
        let root = dir.path();
        mark_unit_closed(&root.to_string_lossy(), "s-close");

        // O resumo do caso real: cita o segundo trabalho e esquece o terceiro.
        let summary = "O dia fechou assim: PRs 267 a 270 mergeados; segue o p-2.";
        match verdict(root, &stop("s-close", summary)) {
            Verdict::Deny { reason } => {
                assert!(reason.contains("P-1"), "the omitted item's id is named: {reason}");
                assert!(reason.contains("Humanize"), "the omitted item's title is named: {reason}");
                assert!(!reason.contains("P-2"), "a cited item is not demanded again: {reason}");
            }
            other => panic!("a closing turn that omits an open item must block, got {other:?}"),
        }

        // A reescrita que o bloqueio provoca chega com `stop_hook_active`…
        let mut again = stop("s-close", summary);
        again.raw["stop_hook_active"] = json!(true);
        assert_eq!(verdict(root, &again), Verdict::Allow, "the second pass is released");
        // …e mesmo sem o campo a marca já foi consumida: uma vez por turno.
        assert_eq!(verdict(root, &stop("s-close", summary)), Verdict::Allow, "once per turn");
    }

    /// AC-6 — sem fechamento neste turno, a resposta passa mesmo omitindo todas
    /// as pendências abertas; e o fechamento de OUTRA sessão não conta.
    #[test]
    fn pending_gate_ignores_turn_without_closure() {
        let dir = project_with_two_open_items();
        let root = dir.path();
        assert_eq!(
            verdict(root, &stop("s-quiet", "Pronto, ajustei o teste.")),
            Verdict::Allow,
            "an ordinary turn never recites the list",
        );

        mark_unit_closed(&root.to_string_lossy(), "s-other");
        assert_eq!(
            verdict(root, &stop("s-quiet", "Pronto, ajustei o teste.")),
            Verdict::Allow,
            "a closure recorded by another session is not this turn's",
        );
    }

    /// Citar pelo título também vale, sem diferenciar maiúsculas nem quebras de
    /// linha; um subagente nunca é cobrado; e sem o texto final não há o que
    /// conferir.
    #[test]
    fn a_title_counts_as_a_citation_and_the_gate_self_restricts() {
        let dir = project_with_two_open_items();
        let root = dir.path();
        let session = root.to_string_lossy().into_owned();

        mark_unit_closed(&session, "s-title");
        let both = "Seguem abertos: HUMANIZE e o html padrao\nda spec.";
        assert_eq!(verdict(root, &stop("s-title", both)), Verdict::Allow, "titles cite");

        mark_unit_closed(&session, "s-sub");
        let mut sub = stop("s-sub", "nada");
        sub.agent_id = Some("closure-1".to_string());
        assert_eq!(verdict(root, &sub), Verdict::Allow, "a subagent stop is never gated");

        mark_unit_closed(&session, "s-bare");
        let bare = HookInput {
            hook_event_name: Some("Stop".to_string()),
            session_id: Some("s-bare".to_string()),
            ..HookInput::default()
        };
        assert_eq!(verdict(root, &bare), Verdict::Allow, "no final text, nothing to check");
    }

    /// Um id conta inteiro: `P-10` não cita `P-1`.
    #[test]
    fn an_id_is_matched_whole() {
        assert!(mentions_id("segue o p-1.", "p-1"));
        assert!(mentions_id("(p-1)", "p-1"));
        assert!(!mentions_id("segue o p-10", "p-1"));
        assert!(!mentions_id("xp-1", "p-1"));
    }

    /// O escritor de eventos é quem arma a cobrança: gravar `pipeline.complete`
    /// ou `pr.merged` marca a sessão que o gravou, e só ela. Um evento comum não
    /// marca nada.
    #[test]
    fn a_recorded_closure_arms_the_gate_for_its_session() {
        let dir = project_with_two_open_items();
        let root = dir.path();
        let project = root.to_string_lossy().into_owned();
        let event = |name: &str, session: &str| mustard_core::domain::model::event::HarnessEvent {
            v: mustard_core::domain::model::event::SCHEMA_VERSION,
            ts: "2026-09-10T12:00:00.000Z".to_string(),
            session_id: session.to_string(),
            wave: 0,
            actor: mustard_core::domain::model::event::Actor {
                kind: mustard_core::domain::model::event::ActorKind::Orchestrator,
                id: Some("test".to_string()),
                actor_type: None,
            },
            event: name.to_string(),
            payload: json!({}),
            spec: Some("uma-unidade".to_string()),
        };

        crate::shared::events::route::emit(&project, &event("tool.use", "s-w"));
        assert!(!take_unit_closed(&project, "s-w"), "an ordinary event is not a closure");

        for closure in ["pipeline.complete", "pr.merged"] {
            crate::shared::events::route::emit(&project, &event(closure, "s-w"));
            assert!(!take_unit_closed(&project, "s-other"), "{closure}: only its own session");
            assert!(take_unit_closed(&project, "s-w"), "{closure} must arm the gate");
            assert!(!take_unit_closed(&project, "s-w"), "{closure}: consumed once");
        }
    }
}
