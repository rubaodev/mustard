//! `mustard-rt run scratch-gc` — recolhe as cópias descartáveis que os agentes
//! deixam no diretório temporário.
//!
//! ## Por quê
//!
//! Cada experimento de revisor roda numa pasta descartável com uma cópia do
//! projeto, e cada cópia compila tudo do zero: de 2 a 5 GB de `target/` por
//! cópia. A trava de comandos (BG01) nega a exclusão recursiva solta, então a
//! pasta ficava para sempre. Esta porta é o caminho de limpeza: a exclusão é
//! feita pelo próprio binário (`std::fs::remove_dir_all`), nunca por comando de
//! shell, e só depois de conferir que o alvo é mesmo uma pasta descartável.
//!
//! ## Candidata — os quatro filtros juntos
//!
//! 1. mora no diretório temporário do sistema (filha direta de
//!    `std::env::temp_dir()`, inclusive os `tmp.*` do `mktemp`) ou dentro da
//!    pasta de trabalho de uma sessão do Claude Code
//!    (`<temp>/claude-<uid>/<projeto>/<sessao>/scratchpad/<pasta>`);
//! 2. contém uma cópia deste projeto (`Cargo.toml` + `apps/rt`) ou uma pasta
//!    `target/` de compilação — nela mesma ou numa filha direta, que é onde o
//!    `git clone` dentro de um `mktemp -d` a deixa;
//! 3. nada nela foi modificado há mais de [`MIN_AGE_HOURS`] horas;
//! 4. não é a pasta de trabalho da sessão atual.
//!
//! Os `mustard-removal-*` do temp ficam de fora: são worktrees registradas
//! que o `worktree-gc` recolhe pelo dono vivo ou morto, e duas portas
//! apagando o mesmo alvo com critérios diferentes não se somam.
//!
//! ## Modos
//!
//! - sem opção: SÓ LISTA as candidatas (caminho, tamanho, idade); nada é
//!   apagado;
//! - `--apply`: apaga exatamente as listadas, e esvazia a compilação
//!   compartilhada quando ela passa do teto;
//! - `--path <dir>`: apaga UMA pasta, sem o filtro de idade (o revisor apaga a
//!   própria pasta recém-criada ao terminar), mas só depois de conferir os
//!   filtros 1 e 2. Fora do diretório temporário — o repositório, a home — é
//!   recusado com erro (exit 1) e nada é tocado.
//!
//! ## Compilação compartilhada
//!
//! As cópias descartáveis compilam em `~/.cache/mustard/scratch-target`. Acima
//! de [`DEFAULT_SHARED_TARGET_CAP_BYTES`] (ajustável por
//! `MUSTARD_SCRATCH_TARGET_CAP_BYTES`) ela é esvaziada no `--apply`: é cache,
//! e o pior efeito de esvaziá-la durante uma compilação alheia é essa
//! compilação refazer o trabalho.
//!
//! ## Saída
//!
//! JSON pretty, campos em ordem de declaração e listas ordenadas por caminho.
//! Exit 0 sempre, exceto `--path` recusado (exit 1).

use crate::shared::context;
use crate::shared::events::economy;
use mustard_core::domain::model::event::ActorKind;
use serde::Serialize;
use serde_json::json;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

// ---------------------------------------------------------------------------
// Limites
// ---------------------------------------------------------------------------

/// Idade mínima, em horas, para uma pasta virar candidata. Cobre com folga a
/// unidade mais longa de um dia de trabalho: uma pasta tocada nas últimas 12
/// horas pode ser de um agente de outra sessão ainda rodando.
pub const MIN_AGE_HOURS: u64 = 12;

/// Teto da compilação compartilhada: 8 GiB. Acima disso ela vira o novo disco
/// cheio que esta porta existe para evitar.
pub const DEFAULT_SHARED_TARGET_CAP_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// Variável que ajusta o teto (em bytes) — existe para teste e para máquina
/// com disco apertado.
pub const CAP_ENV: &str = "MUSTARD_SCRATCH_TARGET_CAP_BYTES";

/// Prefixo da raiz das sessões do Claude Code no temp (`claude-<uid>`).
const SESSION_ROOT_PREFIX: &str = "claude-";

/// Pasta de trabalho de uma sessão, dentro de `claude-<uid>/<projeto>/<sessao>/`.
const SCRATCHPAD_DIR: &str = "scratchpad";

/// Prefixo das worktrees de prova de remoção — dono é o `worktree-gc`.
const REMOVAL_WORKTREE_PREFIX: &str = "mustard-removal-";

// ---------------------------------------------------------------------------
// Opções + relatório
// ---------------------------------------------------------------------------

/// Options for `mustard-rt run scratch-gc`.
pub struct ScratchGcOpts {
    /// `true` apaga as candidatas listadas; `false` (padrão) só lista.
    pub apply: bool,
    /// Apaga só esta pasta, conferida, sem o filtro de idade.
    pub path: Option<PathBuf>,
}

/// Uma candidata: pasta descartável antiga, pronta para sair.
#[derive(Debug, Serialize)]
pub(crate) struct ScratchRecord {
    pub path: String,
    pub size_bytes: u64,
    pub age_hours: u64,
    /// O caminho exato a apagar — fora do JSON, para a exclusão nunca
    /// depender de uma conversão com perda de `path`.
    #[serde(skip)]
    pub dir: PathBuf,
}

/// Uma pasta que passou nos filtros 1 e 2 e mesmo assim fica.
#[derive(Debug, Serialize)]
pub(crate) struct KeptRecord {
    pub path: String,
    /// `"current session"`, `"younger than 12h"` ou `"unknown age"`.
    pub reason: String,
}

/// Uma exclusão tentada que falhou, ou um `--path` recusado.
#[derive(Debug, Serialize)]
pub(crate) struct ErrorRecord {
    pub path: String,
    pub error: String,
}

/// Estado da compilação compartilhada.
#[derive(Debug, Serialize)]
pub(crate) struct SharedTargetRecord {
    pub path: String,
    pub size_bytes: u64,
    pub cap_bytes: u64,
    pub over_cap: bool,
    pub emptied: bool,
}

/// O relatório inteiro, legível por máquina.
#[derive(Debug, Serialize)]
pub(crate) struct ScratchGcReport {
    pub dry_run: bool,
    pub min_age_hours: u64,
    pub candidates: Vec<ScratchRecord>,
    pub candidates_bytes: u64,
    pub kept: Vec<KeptRecord>,
    pub removed: Vec<String>,
    pub errors: Vec<ErrorRecord>,
    pub shared_target: Option<SharedTargetRecord>,
}

// ---------------------------------------------------------------------------
// Onde olhar
// ---------------------------------------------------------------------------

/// As raízes e a identidade que a varredura consulta. Explícitas para o teste
/// montar um temp falso — a varredura nunca lê o ambiente por conta própria.
pub(crate) struct ScratchRoots {
    pub temp_root: PathBuf,
    pub shared_target: Option<PathBuf>,
    pub cap_bytes: u64,
    pub current_session: String,
    pub current_dir: Option<PathBuf>,
}

impl ScratchRoots {
    /// As raízes reais desta máquina e desta sessão.
    pub(crate) fn from_env() -> Self {
        Self {
            temp_root: std::env::temp_dir(),
            shared_target: shared_target_dir(),
            cap_bytes: cap_bytes_from_env(),
            current_session: context::session_id(),
            current_dir: std::env::current_dir().ok(),
        }
    }
}

/// `~/.cache/mustard/scratch-target` — onde as cópias descartáveis compilam.
pub fn shared_target_dir() -> Option<PathBuf> {
    crate::util::home_dir().map(|h| h.join(".cache").join("mustard").join("scratch-target"))
}

/// O teto em bytes: `MUSTARD_SCRATCH_TARGET_CAP_BYTES` quando é um número,
/// senão [`DEFAULT_SHARED_TARGET_CAP_BYTES`].
fn cap_bytes_from_env() -> u64 {
    std::env::var(CAP_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_SHARED_TARGET_CAP_BYTES)
}

/// Uma pasta onde uma cópia descartável pode morar (filtro 1), com o nome da
/// sessão dona quando ela está no `scratchpad/` de uma sessão.
struct Location {
    path: PathBuf,
    session: Option<String>,
}

fn file_name(path: &Path) -> Option<String> {
    path.file_name().and_then(OsStr::to_str).map(str::to_string)
}

/// Filhas diretas que são pastas — links simbólicos não contam, porque
/// `DirEntry::file_type` não os segue. Ordenadas; ilegível vira vazio.
fn child_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = read
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

/// Todas as pastas que passam no filtro 1, ordenadas por caminho.
fn list_locations(temp_root: &Path) -> Vec<Location> {
    let mut out = Vec::new();
    for dir in child_dirs(temp_root) {
        let Some(name) = file_name(&dir) else {
            continue;
        };
        if name.starts_with(SESSION_ROOT_PREFIX) {
            // `claude-<uid>` é contêiner, nunca candidata: só as filhas do
            // `scratchpad/` de cada sessão são.
            for project in child_dirs(&dir) {
                for session in child_dirs(&project) {
                    let owner = file_name(&session);
                    for scratch in child_dirs(&session.join(SCRATCHPAD_DIR)) {
                        out.push(Location { path: scratch, session: owner.clone() });
                    }
                }
            }
            continue;
        }
        if name.starts_with(REMOVAL_WORKTREE_PREFIX) {
            continue;
        }
        out.push(Location { path: dir, session: None });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

// ---------------------------------------------------------------------------
// O que ela contém (filtro 2)
// ---------------------------------------------------------------------------

/// Uma cópia deste projeto: `Cargo.toml` na raiz e `apps/rt`.
fn is_project_copy(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file() && dir.join("apps").join("rt").is_dir()
}

/// Uma pasta de compilação do cargo: tem a etiqueta de cache que o cargo
/// grava, ou um perfil (`debug/`, `release/`) dentro.
fn is_build_target(dir: &Path) -> bool {
    dir.is_dir()
        && (dir.join("CACHEDIR.TAG").is_file()
            || dir.join(".rustc_info.json").is_file()
            || dir.join("debug").is_dir()
            || dir.join("release").is_dir())
}

/// Filtro 2: a pasta — ou uma filha direta — é cópia do projeto, tem um
/// `target/` de compilação, ou ela mesma é um `target/`.
fn holds_scratch_build(dir: &Path) -> bool {
    let shaped = |d: &Path| is_project_copy(d) || is_build_target(&d.join("target"));
    if shaped(dir) {
        return true;
    }
    if file_name(dir).as_deref() == Some("target") && is_build_target(dir) {
        return true;
    }
    child_dirs(dir).iter().any(|child| shaped(child))
}

// ---------------------------------------------------------------------------
// Sessão atual (filtro 4) e medida (filtro 3)
// ---------------------------------------------------------------------------

/// Filtro 4: a pasta mora no `scratchpad/` da sessão atual, ou contém o
/// diretório de onde este comando roda.
fn is_current_session(loc: &Location, roots: &ScratchRoots) -> bool {
    if loc.session.as_deref() == Some(roots.current_session.as_str()) {
        return true;
    }
    let Some(cwd) = roots.current_dir.as_ref() else {
        return false;
    };
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(cwd).starts_with(canon(&loc.path))
}

/// Tamanho e modificação mais recente de uma árvore.
struct Measure {
    bytes: u64,
    newest: Option<SystemTime>,
}

/// Mede uma árvore numa passada só, sem seguir links simbólicos
/// (`DirEntry::metadata` não os segue).
///
/// A idade vem do ARQUIVO mais recente, não da pasta: a data de uma pasta só
/// muda quando uma filha direta entra ou sai, e um agente compilando fundo em
/// `target/debug/deps/` a deixaria parecendo abandonada. Sem arquivo algum, a
/// data da própria raiz responde.
fn measure(root: &Path) -> Measure {
    let mut bytes: u64 = 0;
    let mut newest: Option<SystemTime> = None;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
                continue;
            }
            bytes = bytes.saturating_add(meta.len());
            if let Ok(modified) = meta.modified() {
                newest = Some(newest.map_or(modified, |n| n.max(modified)));
            }
        }
    }
    let newest = newest.or_else(|| std::fs::metadata(root).and_then(|m| m.modified()).ok());
    Measure { bytes, newest }
}

// ---------------------------------------------------------------------------
// Varredura (reusada pelo `doctor --residue`)
// ---------------------------------------------------------------------------

/// O que a varredura encontrou, antes de qualquer exclusão.
pub(crate) struct Survey {
    pub candidates: Vec<ScratchRecord>,
    pub kept: Vec<KeptRecord>,
    pub shared_target: Option<SharedTargetRecord>,
}

impl Survey {
    /// Soma dos tamanhos das candidatas.
    pub(crate) fn candidates_bytes(&self) -> u64 {
        self.candidates.iter().fold(0u64, |acc, c| acc.saturating_add(c.size_bytes))
    }
}

/// Aplica os quatro filtros e mede a compilação compartilhada. Não apaga
/// nada — é a leitura que o `doctor --residue` também usa.
pub(crate) fn survey(roots: &ScratchRoots) -> Survey {
    let now = SystemTime::now();
    let min_age = Duration::from_secs(MIN_AGE_HOURS * 3600);
    let mut candidates = Vec::new();
    let mut kept = Vec::new();

    for loc in list_locations(&roots.temp_root) {
        if !holds_scratch_build(&loc.path) {
            continue;
        }
        let path = loc.path.display().to_string();
        // A sessão atual é conferida ANTES de medir: medir custa uma passada
        // pela árvore inteira, e a resposta já está decidida.
        if is_current_session(&loc, roots) {
            kept.push(KeptRecord { path, reason: "current session".into() });
            continue;
        }
        let measured = measure(&loc.path);
        let Some(elapsed) = measured.newest.and_then(|t| now.duration_since(t).ok()) else {
            // Data ilegível ou no futuro: sem medida não há autorização.
            kept.push(KeptRecord { path, reason: "unknown age".into() });
            continue;
        };
        if elapsed <= min_age {
            kept.push(KeptRecord { path, reason: format!("younger than {MIN_AGE_HOURS}h") });
            continue;
        }
        candidates.push(ScratchRecord {
            path,
            size_bytes: measured.bytes,
            age_hours: elapsed.as_secs() / 3600,
            dir: loc.path,
        });
    }

    let shared_target = roots.shared_target.as_deref().filter(|p| p.is_dir()).map(|p| {
        let size_bytes = measure(p).bytes;
        SharedTargetRecord {
            path: p.display().to_string(),
            size_bytes,
            cap_bytes: roots.cap_bytes,
            over_cap: size_bytes > roots.cap_bytes,
            emptied: false,
        }
    });

    Survey { candidates, kept, shared_target }
}

// ---------------------------------------------------------------------------
// Exclusão
// ---------------------------------------------------------------------------

/// Esvazia a compilação compartilhada: apaga e recria a pasta, para o
/// `CARGO_TARGET_DIR` das cópias continuar apontando para algo que existe.
fn empty_dir(dir: &Path) -> Result<(), String> {
    std::fs::remove_dir_all(dir).map_err(|e| format!("remove_dir_all failed: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("create_dir_all failed: {e}"))
}

/// Varredura + (com `apply`) exclusão das candidatas e do excesso da
/// compilação compartilhada. Sem stdout nem telemetria — o `run` cuida disso.
fn gc(roots: &ScratchRoots, apply: bool) -> ScratchGcReport {
    let survey = survey(roots);
    let candidates_bytes = survey.candidates_bytes();
    let mut report = ScratchGcReport {
        dry_run: !apply,
        min_age_hours: MIN_AGE_HOURS,
        candidates: survey.candidates,
        candidates_bytes,
        kept: survey.kept,
        removed: Vec::new(),
        errors: Vec::new(),
        shared_target: survey.shared_target,
    };
    if !apply {
        return report;
    }

    for candidate in &report.candidates {
        match std::fs::remove_dir_all(&candidate.dir) {
            Ok(()) => report.removed.push(candidate.path.clone()),
            Err(e) => report.errors.push(ErrorRecord {
                path: candidate.path.clone(),
                error: format!("remove_dir_all failed: {e}"),
            }),
        }
    }

    if let (Some(shared), Some(dir)) = (report.shared_target.as_mut(), roots.shared_target.as_deref()) {
        if shared.over_cap {
            match empty_dir(dir) {
                Ok(()) => shared.emptied = true,
                Err(error) => report.errors.push(ErrorRecord { path: shared.path.clone(), error }),
            }
        }
    }

    report
}

/// `--path`: confere os filtros 1 e 2 e apaga UMA pasta, sem o filtro de
/// idade. Devolve o caminho apagado, ou o motivo da recusa — e recusa não toca
/// em nada.
///
/// O caminho é resolvido (`canonicalize`) ANTES de qualquer conferência: um
/// link no temp apontando para o repositório vira o repositório, e é recusado
/// como tal.
pub(crate) fn remove_path(target: &Path, temp_root: &Path) -> Result<PathBuf, String> {
    let dir = std::fs::canonicalize(target)
        .map_err(|e| format!("refused: cannot resolve {}: {e}", target.display()))?;
    let temp = std::fs::canonicalize(temp_root)
        .map_err(|e| format!("refused: cannot resolve the temp directory {}: {e}", temp_root.display()))?;
    let Ok(rel) = dir.strip_prefix(&temp) else {
        return Err(format!(
            "refused: {} is outside the temp directory {}",
            dir.display(),
            temp.display()
        ));
    };
    let parts: Vec<&OsStr> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        return Err(format!("refused: {} is the temp directory itself", dir.display()));
    }
    // Dentro de `claude-<uid>/` só vale o que está abaixo de um `scratchpad/`:
    // os níveis de cima são a estrutura da sessão, não uma cópia.
    let in_session_tree = parts[0].to_str().is_some_and(|n| n.starts_with(SESSION_ROOT_PREFIX));
    if in_session_tree && !(parts.len() >= 5 && parts[3] == SCRATCHPAD_DIR) {
        return Err(format!(
            "refused: {} is part of a Claude Code session layout, not a folder inside its scratchpad/",
            dir.display()
        ));
    }
    if !dir.is_dir() {
        return Err(format!("refused: {} is not a directory", dir.display()));
    }
    if !holds_scratch_build(&dir) {
        return Err(format!(
            "refused: {} holds neither a copy of this project nor a build target/",
            dir.display()
        ));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("remove_dir_all failed: {e}"))?;
    Ok(dir)
}

/// O relatório do modo `--path`, e se a pasta foi recusada.
fn path_report(target: &Path, temp_root: &Path) -> (ScratchGcReport, bool) {
    let mut report = ScratchGcReport {
        dry_run: false,
        min_age_hours: MIN_AGE_HOURS,
        candidates: Vec::new(),
        candidates_bytes: 0,
        kept: Vec::new(),
        removed: Vec::new(),
        errors: Vec::new(),
        shared_target: None,
    };
    let refused = match remove_path(target, temp_root) {
        Ok(dir) => {
            report.removed.push(dir.display().to_string());
            false
        }
        Err(error) => {
            report.errors.push(ErrorRecord { path: target.display().to_string(), error });
            true
        }
    };
    (report, refused)
}

// ---------------------------------------------------------------------------
// Formatação
// ---------------------------------------------------------------------------

/// Bytes em unidade legível (base 1024, uma casa decimal): `512 B`, `3.4 GB`.
pub(crate) fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut unit = 0usize;
    let mut scale: u128 = 1;
    while unit + 1 < UNITS.len() && u128::from(n) >= scale * 1024 {
        scale *= 1024;
        unit += 1;
    }
    if unit == 0 {
        return format!("{n} B");
    }
    let tenths = u128::from(n) * 10 / scale;
    format!("{}.{} {}", tenths / 10, tenths % 10, UNITS[unit])
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

/// Dispatch `mustard-rt run scratch-gc [--apply] [--path <dir>]`.
pub fn run(opts: ScratchGcOpts) {
    let started = std::time::Instant::now();
    let roots = ScratchRoots::from_env();
    let (report, refused) = match opts.path.as_deref() {
        Some(target) => path_report(target, &roots.temp_root),
        None => (gc(&roots, opts.apply), false),
    };

    let body = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
    println!("{body}");
    if refused {
        for e in &report.errors {
            eprintln!("scratch-gc: {}", e.error);
        }
    }

    economy::emit_operation(
        &context::cwd(),
        ActorKind::Orchestrator,
        "scratch-gc",
        started.elapsed().as_millis() as u64,
        None,
        json!({"removed": report.removed.len(), "errors": report.errors.len()}),
    );
    if refused {
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    const CURRENT: &str = "sess-current";

    /// Um temp falso e uma compilação compartilhada falsa dentro de `base`.
    fn fake_roots(base: &Path) -> ScratchRoots {
        let temp_root = base.join("tmp");
        fs::create_dir_all(&temp_root).unwrap();
        ScratchRoots {
            temp_root,
            shared_target: Some(base.join("cache").join("scratch-target")),
            cap_bytes: DEFAULT_SHARED_TARGET_CAP_BYTES,
            current_session: CURRENT.to_string(),
            current_dir: None,
        }
    }

    /// Uma cópia deste projeto em `dir`, com um `target/` de compilação.
    fn project_copy(dir: &Path) {
        fs::create_dir_all(dir.join("apps").join("rt").join("src")).unwrap();
        fs::write(dir.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(dir.join("apps").join("rt").join("src").join("lib.rs"), "// copia\n").unwrap();
        fs::create_dir_all(dir.join("target").join("debug")).unwrap();
        fs::write(dir.join("target").join("CACHEDIR.TAG"), "Signature: 8a477f597d28d172789f06886806bc55\n").unwrap();
        fs::write(dir.join("target").join("debug").join("mustard-rt"), vec![0u8; 4096]).unwrap();
    }

    /// Só uma pasta `target/` de compilação dentro de `dir`.
    fn target_only(dir: &Path) {
        fs::create_dir_all(dir.join("target").join("debug").join("deps")).unwrap();
        fs::write(dir.join("target").join("CACHEDIR.TAG"), "Signature\n").unwrap();
        fs::write(dir.join("target").join("debug").join("deps").join("libx.rlib"), vec![0u8; 2048]).unwrap();
    }

    /// Envelhece todos os arquivos da árvore em `hours` horas. A idade vem do
    /// arquivo mais recente, então basta mexer nos arquivos — e `set_modified`
    /// num arquivo aberto para escrita funciona em toda plataforma.
    fn age_tree(dir: &Path, hours: u64) {
        let when = SystemTime::now() - Duration::from_secs(hours * 3600 + 60);
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in fs::read_dir(&d).unwrap().flatten() {
                let p = entry.path();
                if entry.file_type().unwrap().is_dir() {
                    stack.push(p);
                } else {
                    let file = fs::OpenOptions::new().write(true).open(&p).unwrap();
                    file.set_modified(when).unwrap();
                }
            }
        }
    }

    /// AC-1 — sem opção, a candidata antiga é listada com tamanho e idade, e
    /// nada é apagado.
    #[test]
    fn scratch_gc_dry_run_lists_and_keeps() {
        let base = tempdir().unwrap();
        let roots = fake_roots(base.path());
        let old = roots.temp_root.join("tmp.old1");
        project_copy(&old);
        age_tree(&old, 20);

        let report = gc(&roots, /* apply = */ false);

        assert!(report.dry_run);
        assert_eq!(report.candidates.len(), 1, "{:?}", report.candidates);
        let c = &report.candidates[0];
        assert_eq!(c.path, old.display().to_string());
        assert!(c.size_bytes >= 4096, "size reported: {}", c.size_bytes);
        assert!(c.age_hours >= 20, "age reported: {}", c.age_hours);
        assert_eq!(report.candidates_bytes, c.size_bytes);
        assert!(report.removed.is_empty(), "dry-run removes nothing");
        assert!(old.join("Cargo.toml").exists(), "and the folder is intact");

        let value = serde_json::to_value(&report).unwrap();
        assert!(value["candidates"][0].get("dir").is_none(), "the internal path stays out of the JSON");
        assert!(value["candidates"][0]["size_bytes"].is_u64());
        assert!(value["candidates"][0]["age_hours"].is_u64());
    }

    /// AC-2 — `--apply` apaga só as candidatas antigas; a pasta recente, a da
    /// sessão atual e a que não é do Mustard ficam.
    #[test]
    fn scratch_gc_apply_removes_only_old_candidates() {
        let base = tempdir().unwrap();
        let mut roots = fake_roots(base.path());
        let tmp = roots.temp_root.clone();

        let old = tmp.join("tmp.old");
        project_copy(&old);
        age_tree(&old, 30);

        // Clone dentro de um `mktemp -d`: a cópia está uma pasta abaixo.
        let nested = tmp.join("tmp.nested");
        project_copy(&nested.join("mustard"));
        age_tree(&nested, 30);

        let young = tmp.join("tmp.young");
        project_copy(&young);

        let foreign = tmp.join("outra-coisa");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("notas.txt"), "nao e do mustard").unwrap();
        age_tree(&foreign, 30);

        let sessions = tmp.join("claude-1000").join("-home-x-proj");
        let mine = sessions.join(CURRENT).join(SCRATCHPAD_DIR).join("copia");
        target_only(&mine);
        age_tree(&mine, 30);
        let other = sessions.join("sess-antiga").join(SCRATCHPAD_DIR).join("copia");
        target_only(&other);
        age_tree(&other, 30);

        // A pasta de onde o comando roda também é da sessão atual.
        let running = tmp.join("tmp.running");
        project_copy(&running);
        age_tree(&running, 30);
        roots.current_dir = Some(running.join("apps").join("rt"));

        let report = gc(&roots, /* apply = */ true);

        assert!(!old.exists(), "old project copy removed");
        assert!(!nested.exists(), "old mktemp folder holding a clone removed");
        assert!(!other.exists(), "old copy of another session removed");
        assert!(young.exists(), "recent copy kept");
        assert!(mine.exists(), "current session's scratchpad kept");
        assert!(running.exists(), "folder holding the current directory kept");
        assert!(foreign.exists(), "folder without a copy or target/ is never touched");
        assert!(sessions.join("sess-antiga").join(SCRATCHPAD_DIR).exists(), "only the child goes, never the scratchpad");

        let mut expected = vec![
            nested.display().to_string(),
            other.display().to_string(),
            old.display().to_string(),
        ];
        expected.sort();
        let mut removed = report.removed.clone();
        removed.sort();
        assert_eq!(removed, expected);
        assert!(report.errors.is_empty(), "{:?}", report.errors);

        let reason_of = |p: &Path| {
            report
                .kept
                .iter()
                .find(|k| k.path == p.display().to_string())
                .map(|k| k.reason.clone())
                .unwrap_or_default()
        };
        assert_eq!(reason_of(&young), "younger than 12h");
        assert_eq!(reason_of(&mine), "current session");
        assert_eq!(reason_of(&running), "current session");
    }

    /// AC-3 — `--path` fora do temp (o repositório, a home) é recusado e nada
    /// é apagado; dentro do temp, os filtros 1 e 2 continuam valendo.
    #[test]
    fn scratch_gc_path_refuses_outside_temp() {
        let base = tempdir().unwrap();
        let roots = fake_roots(base.path());

        // O "repositório": cópia completa do projeto, mas fora do temp.
        let repo = base.path().join("repo");
        project_copy(&repo);
        let err = remove_path(&repo, &roots.temp_root).unwrap_err();
        assert!(err.contains("outside the temp directory"), "{err}");
        assert!(repo.join("Cargo.toml").exists(), "the repository is untouched");

        // A "home": contém o temp, então também está fora dele.
        let err = remove_path(base.path(), &roots.temp_root).unwrap_err();
        assert!(err.contains("outside the temp directory"), "{err}");
        assert!(roots.temp_root.exists());

        // Um link no temp apontando para o repositório vira o repositório.
        #[cfg(unix)]
        {
            let link = roots.temp_root.join("tmp.link");
            std::os::unix::fs::symlink(&repo, &link).unwrap();
            let err = remove_path(&link, &roots.temp_root).unwrap_err();
            assert!(err.contains("outside the temp directory"), "{err}");
            assert!(repo.join("Cargo.toml").exists(), "a symlink never reaches the repository");
        }

        // O próprio temp, a estrutura da sessão e uma pasta alheia: recusados.
        assert!(remove_path(&roots.temp_root, &roots.temp_root).is_err());
        let session_layout = roots.temp_root.join("claude-1000").join("proj");
        target_only(&session_layout);
        assert!(remove_path(&session_layout, &roots.temp_root).is_err());
        assert!(session_layout.exists());
        let foreign = roots.temp_root.join("outra-coisa");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("notas.txt"), "x").unwrap();
        assert!(remove_path(&foreign, &roots.temp_root).is_err());
        assert!(foreign.join("notas.txt").exists());
    }

    /// `--path` apaga a pasta recém-criada do revisor: sem filtro de idade.
    #[test]
    fn path_removes_a_fresh_scratch_copy_without_the_age_filter() {
        let base = tempdir().unwrap();
        let roots = fake_roots(base.path());
        let fresh = roots.temp_root.join("tmp.fresh");
        project_copy(&fresh);
        let scratch = roots.temp_root.join("claude-1000").join("proj").join("sess").join(SCRATCHPAD_DIR).join("c");
        target_only(&scratch);

        assert!(remove_path(&fresh, &roots.temp_root).is_ok());
        assert!(!fresh.exists());
        assert!(remove_path(&scratch, &roots.temp_root).is_ok());
        assert!(!scratch.exists());
    }

    /// AC-4 — acima do teto, a compilação compartilhada é esvaziada no
    /// `--apply`; abaixo dele, ou sem `--apply`, fica como está.
    #[test]
    fn scratch_gc_empties_shared_target_above_cap() {
        let base = tempdir().unwrap();
        let mut roots = fake_roots(base.path());
        let shared = roots.shared_target.clone().unwrap();
        fs::create_dir_all(shared.join("debug")).unwrap();
        fs::write(shared.join("debug").join("big.rlib"), vec![0u8; 4096]).unwrap();

        // Abaixo do teto: nada muda.
        roots.cap_bytes = 1_000_000;
        let report = gc(&roots, true);
        let st = report.shared_target.as_ref().unwrap();
        assert!(!st.over_cap && !st.emptied);
        assert!(shared.join("debug").join("big.rlib").exists());

        // Acima do teto, sem `--apply`: só relata.
        roots.cap_bytes = 1024;
        let report = gc(&roots, false);
        let st = report.shared_target.as_ref().unwrap();
        assert!(st.over_cap && !st.emptied);
        assert!(shared.join("debug").join("big.rlib").exists());

        // Acima do teto, com `--apply`: esvazia e mantém a pasta.
        let report = gc(&roots, true);
        let st = report.shared_target.as_ref().unwrap();
        assert!(st.over_cap && st.emptied, "{st:?}");
        assert_eq!(st.size_bytes, 4096);
        assert!(shared.is_dir(), "the folder itself stays for CARGO_TARGET_DIR");
        assert_eq!(fs::read_dir(&shared).unwrap().count(), 0, "and it is empty");
    }

    #[test]
    fn human_bytes_formats_each_unit() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1536), "1.5 KB");
        assert_eq!(human_bytes(DEFAULT_SHARED_TARGET_CAP_BYTES), "8.0 GB");
    }
}
