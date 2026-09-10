//! `spec_doc_present` — no fim de cada resposta, entrega ao usuário o resumo
//! legível da unidade aberta (`resumo.html`), só quando ele mudou.
//!
//! ## Por que existe
//!
//! A página que o `spec-doc` monta só serve se chegar ao usuário. Em 10/09/2026
//! ele recusou uma aprovação por não conseguir ler a spec no terminal (K-3) e
//! escolheu a entrega (K-7): as formas de abrir o documento aparecem a cada
//! mudança dele, e na espera de aprovação o navegador abre sozinho, uma vez por
//! versão.
//!
//! ## Os fatos, todos necessários
//!
//! 1. É o `Stop` da sessão principal — nunca o de um subagente.
//! 2. Há uma unidade aberta: `current_spec` nomeia uma, e o `meta.json` dela não
//!    diz `Completed` (o mesmo corte do `crystallise_nudge`, pelo mesmo motivo:
//!    o arquivo de estado de uma unidade fechada sobrevive até o `SessionEnd`).
//! 3. O `spec-doc` monta a página, e o hash dela difere do último que ESTE
//!    gancho mostrou. O marcador guarda o hash mostrado, não o gravado: uma
//!    página regravada à mão por `run spec-doc` ainda chega ao usuário uma vez.
//!
//! Com os três, a resposta leva a mensagem com as formas de abrir.
//!
//! ## Por que `systemMessage`, e por que um `Inject`
//!
//! RO-4.1, conferido em code.claude.com/docs/en/hooks.md em 10/09/2026:
//! `systemMessage` é campo universal ("Warning message shown to the user"), e a
//! seção do `Stop` não o descarta. Já `hookSpecificOutput.additionalContext` no
//! `Stop` faz a conversa CONTINUAR — um link por ali custaria um turno. O
//! `Verdict` mora no núcleo e não tem variante para o usuário; como no `Stop`
//! não existe próximo turno onde injetar contexto, o `hook_output` lê um
//! `Inject` do `Stop` como a mensagem ao usuário. Nunca bloqueia.
//!
//! ## Como o documento chega, conforme o lugar da sessão
//!
//! - Sessão local: o link `file://`, que o terminal torna clicável.
//! - Sessão por SSH (`SSH_CONNECTION` ou `SSH_CLIENT`): o arquivo está no
//!   servidor e o navegador na outra ponta, onde `file://` não chega. Vão
//!   comandos `scp` prontos para colar — PowerShell do Windows, macOS e Linux —
//!   com o usuário de `$USER` e o host do terceiro campo de `SSH_CONNECTION` (o
//!   endereço do servidor).
//! - Sempre: pedir ao assistente que publique a página no claude.ai.
//!
//! ## Abrir sozinho, só na espera de aprovação
//!
//! Estágio `Plan` sem `.approved-by-user` (o predicado compartilhado do
//! gravador de aprovação, mais o marcador), com tela local — `DISPLAY` ou
//! `WAYLAND_DISPLAY` no Linux; macOS e Windows sempre têm — e fora de SSH. Uma
//! vez por versão: o marcador de abertura é gravado ANTES de abrir, então um
//! marcador que não grava nunca vira uma aba nova por turno.
//! `MUSTARD_DOC_OPEN=off` desliga só a abertura; as formas de abrir continuam.
//!
//! ## Fail-open
//!
//! Toda falha — spec ilegível, disco sem escrita, abridor ausente — só cala o
//! gancho neste turno. Limite conhecido: se um gancho irmão bloquear este mesmo
//! `Stop`, o `Deny` dele vence o `fold` e a mensagem se perde com o marcador já
//! gravado; a próxima mudança do documento volta a mostrá-la.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use mustard_core::domain::model::contract::{Check, Ctx, HookInput, Trigger, Verdict};
use mustard_core::io::fs;
use mustard_core::platform::error::Error;
use mustard_core::ClaudePaths;

use crate::commands::spec::spec_doc::{generate, DOC_FILE};
use crate::hooks::observe::approval_marker_observer::is_awaiting_approval;
use crate::hooks::task::crystallise_nudge::spec_is_closed;
use crate::shared::context::{approval_marker_path, current_spec};

/// O interruptor da abertura automática do navegador.
const OPEN_ENV: &str = "MUSTARD_DOC_OPEN";

/// Se o navegador pode abrir sozinho na espera de aprovação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenMode {
    /// Nunca abre; a mensagem continua aparecendo.
    Off,
    /// Abre uma vez por versão (padrão).
    On,
}

/// Lê `MUSTARD_DOC_OPEN` (padrão `on`). Só `off` desliga; ausente ou qualquer
/// outro valor abre — é um conforto, não uma trava, então não há modo estrito.
fn open_mode() -> OpenMode {
    open_mode_from(std::env::var(OPEN_ENV).ok().as_deref())
}

fn open_mode_from(raw: Option<&str>) -> OpenMode {
    match raw.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "off" => OpenMode::Off,
        _ => OpenMode::On,
    }
}

/// Onde a sessão roda — o que decide como o documento chega ao usuário.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seat {
    /// Disco e terminal na mesma máquina; `display` diz se há tela para abrir.
    Local { display: bool },
    /// Sessão por SSH: o arquivo mora no servidor `host`, acessado como `user`.
    Remote { user: String, host: String },
}

impl Seat {
    fn detect() -> Self {
        Self::from_env(
            &|key| std::env::var(key).ok(),
            cfg!(any(target_os = "macos", target_os = "windows")),
        )
    }

    /// `native_display` é a tela que o sistema sempre tem (macOS, Windows); no
    /// Linux ela vem de `DISPLAY` / `WAYLAND_DISPLAY`. Sem o endereço do servidor
    /// (só `SSH_CLIENT`), vale `HOSTNAME`; sem nada, um marcador `HOST` que o
    /// `scp` recusa em voz alta em vez de copiar de outro lugar.
    fn from_env(env: &dyn Fn(&str) -> Option<String>, native_display: bool) -> Self {
        let var = |key: &str| env(key).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let connection = var("SSH_CONNECTION");
        if connection.is_some() || var("SSH_CLIENT").is_some() {
            let host = connection
                .as_deref()
                .and_then(|c| c.split_whitespace().nth(2))
                .map(str::to_string)
                .or_else(|| var("HOSTNAME"))
                .unwrap_or_else(|| "HOST".to_string());
            // IPv6 vai entre colchetes, senão o `scp` lê o primeiro `:` como o
            // separador do caminho.
            let host = if host.contains(':') { format!("[{host}]") } else { host };
            let user = var("USER")
                .or_else(|| var("USERNAME"))
                .unwrap_or_else(|| "USER".to_string());
            return Self::Remote { user, host };
        }
        Self::Local {
            display: native_display || var("DISPLAY").is_some() || var("WAYLAND_DISPLAY").is_some(),
        }
    }

    fn can_open(&self) -> bool {
        matches!(self, Self::Local { display: true })
    }
}

/// O gancho de fim de turno que entrega o resumo da spec.
pub struct SpecDocPresent;

impl Check for SpecDocPresent {
    fn evaluate(&self, input: &HookInput, ctx: &Ctx) -> Result<Verdict, Error> {
        // Fato 1 — o `Stop` da sessão principal.
        if ctx.trigger != Some(Trigger::Stop) || input.is_subagent() {
            return Ok(Verdict::Allow);
        }
        let project_dir = ctx.project_dir_or_cwd(input);
        let root = Path::new(&project_dir);

        // Fato 2 — uma unidade aberta.
        let Some(spec) = current_spec(&project_dir).filter(|s| !s.is_empty()) else {
            return Ok(Verdict::Allow);
        };
        if spec_is_closed(root, &spec) {
            return Ok(Verdict::Allow);
        }

        // Fato 3 — a página mudou desde a última entrega.
        let message = present(root, &spec, open_mode(), &Seat::detect(), &open_with_system);
        Ok(message.map_or(Verdict::Allow, |context| Verdict::Inject { context }))
    }
}

/// Monta a página e devolve a mensagem ao usuário quando ela mudou desde a
/// última entrega; na espera de aprovação, abre-a por `opener` uma vez por
/// versão. `opener` é injetável para o teste nunca abrir um navegador de
/// verdade.
fn present(
    root: &Path,
    spec: &str,
    mode: OpenMode,
    seat: &Seat,
    opener: &dyn Fn(&Path) -> bool,
) -> Option<String> {
    let report = generate(root, spec);
    if !report.ok || report.hash.is_empty() {
        return None;
    }
    let file = root.join(&report.path);
    let project = root.to_string_lossy();
    let awaiting = is_awaiting_approval(&project, spec)
        && !approval_marker_path(&project, spec).is_some_and(|p| p.is_file());
    if awaiting
        && mode == OpenMode::On
        && seat.can_open()
        && remember(root, "opened", spec, &report.hash)
    {
        let _ = opener(&file);
    }
    if !remember(root, "shown", spec, &report.hash) {
        return None;
    }
    Some(ways_to_open(awaiting, &file, &report.url, seat))
}

/// A mensagem: o que mudou e cada forma de abrir, uma por linha.
fn ways_to_open(awaiting: bool, file: &Path, url: &str, seat: &Seat) -> String {
    let what = if awaiting { "spec awaiting approval" } else { "spec summary" };
    let mut text = format!("Mustard · {what}: {DOC_FILE} changed. Ways to open it:");
    match seat {
        Seat::Local { .. } => {
            let _ = write!(text, "\n- Click: {url}");
        }
        Seat::Remote { user, host } => {
            let source = format!("{user}@{host}:{}", file.display());
            let _ = write!(
                text,
                "\n- Windows (PowerShell): scp {source} $env:TEMP\\{DOC_FILE}; start $env:TEMP\\{DOC_FILE}"
            );
            let _ = write!(text, "\n- macOS: scp {source} /tmp/{DOC_FILE} && open /tmp/{DOC_FILE}");
            let _ = write!(text, "\n- Linux: scp {source} /tmp/{DOC_FILE} && xdg-open /tmp/{DOC_FILE}");
        }
    }
    text.push_str("\n- Ask the assistant to publish it as a claude.ai page.");
    text
}

/// Grava `hash` como a última versão que `what` (`shown` / `opened`) viu desta
/// spec. `true` só quando a versão é nova E ficou gravada: um marcador que não
/// grava responde `false`, e o gancho se cala em vez de repetir a cada turno.
fn remember(root: &Path, what: &str, spec: &str, hash: &str) -> bool {
    let Some(path) = marker_path(root, what, spec) else {
        return false;
    };
    if fs::read_to_string(&path).is_ok_and(|seen| seen.trim() == hash) {
        return false;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write_atomic(&path, hash.as_bytes()).is_ok()
}

/// `<root>/.claude/.harness/spec-doc-<what>-<spec>`.
fn marker_path(root: &Path, what: &str, spec: &str) -> Option<PathBuf> {
    Some(
        ClaudePaths::for_project(root)
            .ok()?
            .harness_dir()
            .join(format!("spec-doc-{what}-{}", spec.replace(['/', '\\'], "-"))),
    )
}

/// O abridor do sistema (`open` / `start` / `xdg-open`). Nenhum fluxo é
/// herdado: o stdout do gancho é o JSON que o harness lê, e um filho segurando
/// o pipe prenderia a resposta até o navegador fechar.
fn open_with_system(file: &Path) -> bool {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = Command::new("xdg-open");
    command
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_output::hook_specific_output;
    use mustard_core::domain::model::contract::Outcome;
    use std::cell::RefCell;
    use tempfile::tempdir;

    const LOCAL: Seat = Seat::Local { display: true };

    /// Uma unidade `demo` no estágio pedido, com uma spec que dá para mudar.
    fn seed(root: &Path, stage: &str) {
        std::fs::write(root.join("mustard.json"), r#"{"specLang":"pt-BR"}"#).unwrap();
        let dir = root.join(".claude/spec/demo");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("meta.json"),
            format!(r#"{{"stage":"{stage}","outcome":"Active","lang":"pt-BR"}}"#),
        )
        .unwrap();
        rewrite_spec(root, "Primeira versão.");
    }

    fn rewrite_spec(root: &Path, context: &str) {
        std::fs::write(
            root.join(".claude/spec/demo/spec.md"),
            format!("# Demo\n\n## Contexto\n\n{context}\n"),
        )
        .unwrap();
    }

    fn never(_: &Path) -> bool {
        panic!("only an approval wait on a local screen opens the browser")
    }

    /// AC-7 — o link aparece quando a página muda, e só então; chega ao
    /// usuário como `systemMessage`, sem bloquear.
    #[test]
    fn stop_presents_doc_link_only_when_changed() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        seed(root, "Execute");

        let first = present(root, "demo", OpenMode::On, &LOCAL, &never).expect("first turn shows");
        assert!(first.contains("spec summary"), "{first}");
        assert!(first.contains("- Click: file://") && first.contains("/resumo.html"), "{first}");
        assert!(first.contains("claude.ai page"), "{first}");

        let outcome = Outcome {
            verdict: Verdict::Inject {
                context: first.clone(),
            },
            warnings: Vec::new(),
        };
        let json = hook_specific_output("Stop", &outcome).expect("a Stop inject emits");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["systemMessage"].as_str(), Some(first.as_str()));
        assert!(parsed.get("decision").is_none(), "never blocks: {json}");
        assert!(parsed.get("hookSpecificOutput").is_none(), "never continues: {json}");

        // A mesma página: silêncio, turno após turno.
        assert!(present(root, "demo", OpenMode::On, &LOCAL, &never).is_none());
        assert!(present(root, "demo", OpenMode::On, &LOCAL, &never).is_none());

        // A spec mudou: a página muda, e o link volta — uma vez.
        rewrite_spec(root, "Segunda versão.");
        assert!(present(root, "demo", OpenMode::On, &LOCAL, &never).is_some());
        assert!(present(root, "demo", OpenMode::On, &LOCAL, &never).is_none());
    }

    /// AC-8 — na espera de aprovação o documento abre uma vez por versão, e não
    /// abre com `MUSTARD_DOC_OPEN=off` nem depois de aprovado.
    #[test]
    fn approval_wait_opens_doc_once() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        seed(root, "Plan");
        let opened: RefCell<Vec<PathBuf>> = RefCell::new(Vec::new());
        let opener = |p: &Path| {
            opened.borrow_mut().push(p.to_path_buf());
            true
        };

        let message = present(root, "demo", OpenMode::On, &LOCAL, &opener).expect("shows");
        assert!(message.contains("spec awaiting approval"), "{message}");
        assert_eq!(opened.borrow().len(), 1);
        assert!(opened.borrow()[0].ends_with(Path::new(".claude/spec/demo").join(DOC_FILE)));

        // A mesma versão nunca abre uma segunda aba.
        let _ = present(root, "demo", OpenMode::On, &LOCAL, &opener);
        assert_eq!(opened.borrow().len(), 1);

        // Versão nova: abre de novo, uma vez.
        rewrite_spec(root, "Segunda versão.");
        let _ = present(root, "demo", OpenMode::On, &LOCAL, &opener);
        let _ = present(root, "demo", OpenMode::On, &LOCAL, &opener);
        assert_eq!(opened.borrow().len(), 2);

        // `MUSTARD_DOC_OPEN=off`: nada abre, e a mensagem continua.
        assert_eq!(open_mode_from(Some("off")), OpenMode::Off);
        assert_eq!(open_mode_from(Some(" OFF ")), OpenMode::Off);
        assert_eq!(open_mode_from(None), OpenMode::On);
        assert_eq!(open_mode_from(Some("on")), OpenMode::On);
        rewrite_spec(root, "Terceira versão.");
        assert!(present(root, "demo", open_mode_from(Some("off")), &LOCAL, &opener).is_some());
        assert_eq!(opened.borrow().len(), 2);

        // Aprovada, a spec não espera mais: nada abre.
        let marker = approval_marker_path(&root.to_string_lossy(), "demo").unwrap();
        std::fs::write(marker, "approved\n").unwrap();
        rewrite_spec(root, "Quarta versão.");
        let after = present(root, "demo", OpenMode::On, &LOCAL, &opener).expect("still shows");
        assert!(after.contains("spec summary"), "{after}");
        assert_eq!(opened.borrow().len(), 2);
    }

    /// Em SSH nada abre, mesmo esperando aprovação e com `DISPLAY`; a mensagem
    /// traz os `scp` prontos e a publicação no claude.ai, e nenhum `file://`.
    #[test]
    fn ssh_session_never_opens_and_lists_the_copy_commands() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        seed(root, "Plan");
        let seat = Seat::from_env(
            &|key| match key {
                "SSH_CONNECTION" => Some("10.0.0.9 51234 10.0.0.5 22".to_string()),
                "USER" => Some("rubens".to_string()),
                "DISPLAY" => Some("localhost:10.0".to_string()),
                _ => None,
            },
            false,
        );
        assert_eq!(seat, Seat::Remote { user: "rubens".to_string(), host: "10.0.0.5".to_string() });

        let message = present(root, "demo", OpenMode::On, &seat, &never).expect("shows");
        let source = format!("rubens@10.0.0.5:{}", root.join(".claude/spec/demo/resumo.html").display());
        for needle in [
            format!("- Windows (PowerShell): scp {source} $env:TEMP\\resumo.html; start $env:TEMP\\resumo.html"),
            format!("- macOS: scp {source} /tmp/resumo.html && open /tmp/resumo.html"),
            format!("- Linux: scp {source} /tmp/resumo.html && xdg-open /tmp/resumo.html"),
            "- Ask the assistant to publish it as a claude.ai page.".to_string(),
        ] {
            assert!(message.contains(&needle), "missing {needle}:\n{message}");
        }
        assert!(!message.contains("file://"), "{message}");
    }

    /// O lugar da sessão: SSH vence a tela; sem o endereço do servidor vale o
    /// `HOSTNAME`; IPv6 vai entre colchetes; no Linux sem tela nada abre.
    #[test]
    fn seat_follows_ssh_and_the_local_screen() {
        let from = |pairs: &[(&str, &str)], native: bool| {
            let owned: Vec<(String, String)> =
                pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect();
            Seat::from_env(
                &|key| owned.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()),
                native,
            )
        };
        assert_eq!(
            from(&[("SSH_CLIENT", "10.0.0.9 51234 22"), ("HOSTNAME", "srv"), ("USER", "ana")], true),
            Seat::Remote { user: "ana".to_string(), host: "srv".to_string() },
        );
        assert_eq!(
            from(&[("SSH_CONNECTION", "fe80::9 51234 fe80::5 22"), ("USER", "ana")], false),
            Seat::Remote { user: "ana".to_string(), host: "[fe80::5]".to_string() },
        );
        assert!(!from(&[], false).can_open(), "a Linux box with no screen opens nothing");
        assert!(from(&[("WAYLAND_DISPLAY", "wayland-0")], false).can_open());
        assert!(from(&[], true).can_open(), "macOS and Windows always have a screen");
    }

    /// Fora do `Stop` da sessão principal o gancho nem olha a unidade.
    #[test]
    fn the_doc_link_self_restricts_to_the_main_stop() {
        let tmp = tempdir().unwrap();
        let project = tmp.path().to_string_lossy().into_owned();
        let ctx = |trigger| Ctx {
            project_dir: project.clone(),
            trigger: Some(trigger),
            workspace_root: None,
            inject_only: None,
        };
        let sub = HookInput {
            hook_event_name: Some("Stop".to_string()),
            agent_id: Some("child".to_string()),
            ..HookInput::default()
        };
        assert_eq!(SpecDocPresent.evaluate(&sub, &ctx(Trigger::Stop)).unwrap(), Verdict::Allow);
        let pre = HookInput::default();
        assert_eq!(SpecDocPresent.evaluate(&pre, &ctx(Trigger::PreToolUse)).unwrap(), Verdict::Allow);
    }
}
