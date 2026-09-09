//! `mustard-rt run ac-add` — introduce an acceptance criterion the spec does
//! NOT yet carry, AFTER the artefacts are frozen, and prove it knows how to
//! fail before it lands anywhere.
//!
//! ## Why this exists
//!
//! The flow already carries the rule: a mid-pipeline request that gets
//! implemented but is named by no criterion makes the gate report green without
//! ever verifying it. The rule was written and nothing implemented it — the only
//! criterion-editing operation, [`super::ac_amend`], REPLACES an id that already
//! exists and refuses one it does not know. So when a review demanded a
//! criterion for a finding, there was no door, and criteria were typed into
//! `spec.md` by hand against the role contract that forbids touching the spec.
//!
//! ## Why not a flag on amend
//!
//! Amend's whole contract is that a replacement must prove it can fail against a
//! tree where the criterion it supersedes already lived — that is what makes
//! `superseded_command` meaningful and what the INEXECUTABLE exception is keyed
//! to. An added id has no predecessor to supersede, so folding it in would blur
//! the one rule that makes amend trustworthy. Two doors, one engine.
//!
//! ## The same proof, no softer
//!
//! The new criterion goes through [`ac_negative_check::prove_one`] and is
//! REFUSED on anything but a red. A door that admitted a criterion nobody proved
//! would import exactly the vacuous-criterion defect the proof was built to
//! stop — and would do it at the one moment nobody is watching, mid-pipeline.
//!
//! ## Where it lands, and why not last
//!
//! The criterion is inserted directly ABOVE the section's LAST criterion, never
//! appended after it. The trailing criterion is exempt by position
//! ([`ac_negative_check::is_exempt`]) — it is the build-green safety net — so
//! appending would silently move the exemption onto the new criterion AND demand
//! a red proof from a build command that is green by design. Position is not
//! decoration here; it is a rule two mechanisms read.
//!
//! It lands in the root `spec.md` and in `wave-plan.md` — the list every reader
//! derives from, and the union QA executes. NOT in a `wave-*/spec.md`: which
//! wave a criterion belongs to is decided by the PLAN (`plan-materialize` cuts
//! each wave's `## Acceptance Criteria` from the plan's `satisfies`), and the
//! plan predates a criterion added mid-pipeline. So the report names the
//! materialised waves that would need re-materialising (`wavesStale` /
//! `staleWaves`, the shape `ac-amend` already reports) and the stderr WARN
//! names the command that brings them forward. Transcripts (`qa/`, `review/`)
//! are NOT artefacts: they are records of a run, and writing a criterion into a
//! past run's report would forge evidence.
//!
//! ## Refusal, not silence
//!
//! Five refusals, each of which writes NOTHING anywhere: a blank reason, a blank
//! statement, an unknown spec, an id the spec ALREADY carries (that is an
//! amendment — a different door), and a command the negative test does not
//! report as proven. Every refusal names its reason AND the one action that
//! clears it.
//!
//! ## One parser, one ledger
//!
//! The criteria are read through [`super::ac_amend`]'s shared helpers, which are
//! themselves [`qa_run`]'s parser and the negative test's ledger. The write is
//! confirmed by RE-READING each artefact: the addition reports whether it landed
//! instead of assuming it did. The record goes in the ledger's `additions`
//! array — never `amendments`, which means a supersession that did not happen.

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::commands::review::ac_negative_check::{
    self, AcProof, AC_PROOF_JSON,
};
use crate::commands::review::qa_run;
use crate::commands::spec::ac_amend::{
    criteria_of, landed, normalise_id, read_ledger, write_ledger, AC_SECTION_KEY,
};
use crate::commands::spec::spec_sections;
use mustard_core::io::fs as mfs;

/// Options for `mustard-rt run ac-add`.
#[derive(Debug, Clone)]
pub struct AcAddOpts {
    /// Spec slug under `.claude/spec/`.
    pub spec: String,
    /// The criterion id to introduce (`AC-9`, `AC-W4-3`, …). Must not exist.
    pub ac: String,
    /// The EARS statement the criterion asserts. Never blank — a criterion
    /// nobody stated is one a later reader cannot check the command against.
    pub statement: String,
    /// The command that asserts the new behaviour.
    pub command: String,
    /// The `Expect:` evidence regex, when the criterion carries one.
    pub expect: Option<String>,
    /// Why the criterion is being added. Never blank.
    pub reason: String,
    /// The criterion's `Control:` — a command that must come back GREEN against
    /// the tree as it is, proving this criterion's red came from the missing
    /// behaviour rather than from a filter that selects nothing.
    ///
    /// The same door `ac-amend` carries, for the same reason. Optional for every
    /// command shape: given, it is taken in the same pass as the red proof and
    /// written onto the new line; omitted, the record says `not-declared`.
    /// Worth declaring for a FILTERED TEST RUNNER — the usual shape of a
    /// criterion added for a finding, and the one the drafting lint names as
    /// `test-ac-no-control` — but its absence never refuses the addition.
    pub control: Option<String>,
    /// Take the negative proof against ANOTHER checkout instead of this tree.
    ///
    /// The same door `ac-amend` carries, for the same reason and one step
    /// earlier. The negative proof asks "can this criterion FAIL?", and that is
    /// only answerable where the behaviour is ABSENT. A criterion added to
    /// cover work that has ALREADY LANDED comes back green here, and green
    /// proves nothing — so without this flag the only way through is to hide
    /// the work, add the criterion, and put the work back: a contrivance that
    /// leaves no trace of where the red was taken.
    ///
    /// Point it at a worktree of a commit that predates the work
    /// (`git worktree add --detach <dir> <base>`). The spec is still read and
    /// rewritten HERE; only the command runs elsewhere, and the ledger records
    /// the COMMIT the red was taken on, so the claim can be checked later.
    pub proof_tree: Option<PathBuf>,
}

/// JSON report printed on stdout. Deterministic: repo-relative paths, sorted,
/// and no timestamp (that lives in the ledger).
#[derive(Debug, Serialize)]
pub(crate) struct AcAddReport {
    /// `true` only when the addition was accepted AND every write is confirmed.
    pub(crate) ok: bool,
    /// The spec the criterion joins.
    pub(crate) spec: String,
    /// The criterion id, normalised (`AC-9`).
    pub(crate) ac: String,
    /// The statement the criterion now carries.
    pub(crate) statement: String,
    /// The command it now carries.
    pub(crate) command: String,
    /// The evidence regex it now carries, when it has one.
    pub(crate) expect: Option<String>,
    /// The proof the negative test took — recorded whichever way it fell, so a
    /// refusal shows exactly what the engine saw.
    pub(crate) proof: Option<AcProof>,
    /// Every artefact the criterion was written into AND confirmed on re-read,
    /// as repo paths with forward slashes.
    pub(crate) written: Vec<String>,
    /// `true` when materialised waves exist that do NOT carry the new
    /// criterion — the flag, with [`Self::stale_waves`] as its evidence.
    ///
    /// A new id is claimed by no wave: the per-wave `## Acceptance Criteria`
    /// is the cut `plan-materialize` made from the plan's `satisfies`, and the
    /// plan predates the criterion. So the addition lands in the root `spec.md`
    /// and the frozen `wave-plan.md` (where QA reads the union) and names here
    /// the waves whose dispatched `## ACCEPTANCE` will not show it until the
    /// plan routes the id and the waves are re-materialised. The same shape
    /// `ac-amend` and `spec-draft --material-only` report, said the same way.
    #[serde(rename = "wavesStale", skip_serializing_if = "std::ops::Not::not")]
    pub(crate) waves_stale: bool,
    /// The wave directories that would need re-materialising, sorted.
    #[serde(rename = "staleWaves", skip_serializing_if = "Vec::is_empty")]
    pub(crate) stale_waves: Vec<String>,
    /// Where the proof ledger lives, when it was updated.
    pub(crate) ledger: Option<String>,
    /// Refusal / failure code: `blank_reason`, `blank_statement`,
    /// `unknown_spec`, `duplicate_criterion`, `criterion_not_proven`,
    /// `write_failed`, `ledger_write_failed`.
    pub(crate) error: Option<String>,
    /// The one action that clears the refusal. Absent when nothing is wrong.
    pub(crate) remedy: Option<String>,
}

impl AcAddReport {
    /// A report for an addition that was refused before anything was written.
    fn refused(opts: &AcAddOpts, ac: &str, error: &str, remedy: &str) -> Self {
        Self {
            ok: false,
            spec: opts.spec.clone(),
            ac: ac.to_string(),
            statement: opts.statement.clone(),
            command: opts.command.clone(),
            expect: opts.expect.clone(),
            proof: None,
            written: Vec::new(),
            waves_stale: false,
            stale_waves: Vec::new(),
            ledger: None,
            error: Some(error.to_string()),
            remedy: Some(remedy.to_string()),
        }
    }
}

/// One addition's entry in the ledger's `additions` array — the id, what it
/// asserts, how it is checked, why it was added, and when.
#[derive(Debug, Serialize)]
struct Addition {
    /// The criterion that joined the spec.
    id: String,
    /// When it was accepted. The ledger is where a timestamp belongs: stdout
    /// must stay byte-stable.
    at: String,
    /// The stated reason — the whole point of recording an addition.
    reason: String,
    /// The statement the criterion carries.
    statement: String,
    /// The command it carries.
    command: String,
    /// The evidence regex it carries, when it has one.
    expect: Option<String>,
    /// The artefacts it was written into, as repo paths.
    wrote: Vec<String>,
}

// ---------------------------------------------------------------------------
// Line surgery
//
// The reader side is `qa_run::parse_ac_items`: a drafter-form criterion is a
// header line plus an indented `Command:` line and an optional `Expect:` line.
// The writer below emits exactly that, and the unit tests assert the ROUND TRIP
// — an inserted block is parsed back by the same parser and must yield the new
// criterion. That round trip, not a shared template, is what keeps the two
// honest.
// ---------------------------------------------------------------------------

/// The criterion block, in the drafter's canonical shape, with `eol` appended to
/// every line so a CRLF document survives the insertion. Pure, total.
///
/// `control` is emitted exactly the way `expect` is, and for a reason the
/// ledger cannot cover: `--control` reaches [`ac_negative_check::prove_one`] and
/// lands in `control_command`, but the NEXT pass of that gate re-reads the
/// MARKDOWN. A criterion admitted with a control whose line carried none was
/// refused all over again at the approval gate — the door opened onto a closed
/// one.
fn criterion_block(
    id: &str,
    statement: &str,
    command: &str,
    expect: Option<&str>,
    control: Option<&str>,
    eol: &str,
) -> Vec<String> {
    let mut block = vec![
        format!("- **{id}** — {}{eol}", statement.trim()),
        format!("  Command: `{command}`{eol}"),
    ];
    if let Some(expect) = expect {
        block.push(format!("  Expect: `{expect}`{eol}"));
    }
    if let Some(control) = control {
        block.push(format!("  Control: `{control}`{eol}"));
    }
    block
}

/// Insert `id`'s criterion block into the `## Acceptance Criteria` section of
/// one markdown document. `None` when the document declares no such section.
///
/// Esse `None` NÃO é o que mantém um artefato de onda fora da adição — e
/// enquanto foi, quebrou: desde que o `wave-scaffold` materializa a seção em
/// toda onda que satisfaz algo, o `None` deixou de acontecer e o critério novo
/// passou a entrar em todas elas. Quem decide o destino é [`write_targets`];
/// esta função só recusa um documento que não tem onde receber a linha.
///
/// Among HOMONYMOUS sections (legacy drafts duplicated the heading) the one
/// carrying criteria wins, mirroring [`spec_sections::section_block`]'s own
/// defensive pick — writing into the placeholder copy would land the criterion
/// where no reader looks.
fn insert_criterion(
    body: &str,
    id: &str,
    statement: &str,
    command: &str,
    expect: Option<&str>,
    control: Option<&str>,
) -> Option<String> {
    let lines: Vec<&str> = body.split('\n').collect();
    let mut carrying: Option<(usize, usize)> = None;
    let mut first: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        if !spec_sections::is_heading(lines[i], AC_SECTION_KEY) {
            i += 1;
            continue;
        }
        let end = spec_sections::section_end(&lines, i);
        if first.is_none() {
            first = Some((i, end));
        }
        if carrying.is_none()
            && (i + 1..end).any(|j| qa_run::parse_ac_header(lines[j]).is_some())
        {
            carrying = Some((i, end));
        }
        i = end;
    }
    let (start, end) = carrying.or(first)?;

    // The anchor: the section's LAST criterion header. The new block lands
    // ABOVE it so the trailing build-green criterion STAYS trailing — the
    // positional exemption must not move onto the criterion being added.
    let anchor = (start + 1..end).rfind(|j| qa_run::parse_ac_header(lines[*j]).is_some());
    let at = match anchor {
        Some(j) => j,
        // An empty section: after its last non-blank line, so the block does
        // not weld onto the heading or trail behind the section's blank tail.
        None => (start + 1..end)
            .rev()
            .find(|j| !lines[*j].trim().is_empty())
            .map_or(end, |j| j + 1),
    };

    // CRLF is a property of the DOCUMENT, not of the anchor line: a file whose
    // lines end `\r\n` must not gain three lone-LF lines in the middle.
    let eol = if body.contains("\r\n") { "\r" } else { "" };
    let mut out: Vec<String> = lines.iter().map(|l| (*l).to_string()).collect();
    let block = criterion_block(id, statement, command, expect, control, eol);
    out.splice(at..at, block);
    Some(out.join("\n"))
}

/// Onde uma ADIÇÃO escreve o critério novo: o `spec.md` do pai e o
/// `wave-plan.md` — a lista que o QA executa — mais o `spec.md` de cada onda que
/// SATISFAZ o id, e só dela.
///
/// O conjunto era [`plan_artefacts`] inteiro, e isso deixou de ser verdade no
/// momento em que o `wave-scaffold` passou a materializar `## Acceptance
/// Criteria` no spec de cada onda. [`insert_criterion`] documenta que devolve
/// `None` "quando o documento não declara a seção — que é como um artefato de
/// onda que não carrega critérios fica de fora", e essa era a garantia inteira:
/// agora TODA onda que satisfaz alguma coisa declara a seção, então o critério
/// novo entrava nas quatro ondas de uma spec de quatro, sob a frase "estes
/// critérios são o JUIZ desta onda", em ondas cujo `satisfies` nunca o nomeou —
/// o ruído por onda que esta unidade existe para tirar, reintroduzido pela porta
/// ao lado.
///
/// Onde uma adição ESCREVE: o `spec.md` do pai e o `wave-plan.md` — a lista de
/// onde todo outro leitor deriva, e a união que o QA executa.
///
/// Nunca uma onda. De quem é um critério está decidido pelo PLANO: o
/// `## Acceptance Criteria` de cada `wave-*/spec.md` é o recorte que o
/// `plan-materialize` fez pelo `satisfies` do plano
/// ([`crate::commands::wave::wave_scaffold::satisfied_ids`]), e o plano é
/// anterior ao critério novo — nenhuma onda o reivindica, e a recusa
/// `duplicate_criterion` já garantiu que nenhum artefato o declara. A versão
/// anterior desta função filtrava as ondas por "a onda declara o id?", condição
/// que a recusa acima torna impossível: escrevia em onda nenhuma e não dizia
/// isso a ninguém, e o agente do `fix-loop` re-despachado pelo achado que
/// motivou o critério era justamente quem não o via. Agora as ondas que ficam
/// para trás são NOMEADAS ([`stale_waves`]) com o comando que as atualiza.
///
/// Comparação com o diretório da spec, nunca com o prefixo do nome: uma spec
/// chamada `wave-algo` tem o `spec.md` do PAI num diretório que começa por
/// `wave-`.
fn write_targets(spec_dir: &Path) -> Vec<PathBuf> {
    plan_artefacts(spec_dir)
        .into_iter()
        .filter(|path| path.parent().is_some_and(|p| p == spec_dir))
        .collect()
}

/// As ondas materializadas sob `spec_dir` — cada `wave-*/spec.md` — que NÃO
/// carregam `id`: as que o operador terá de re-materializar depois de rotear o
/// critério novo no `plan.json`.
///
/// Para uma adição é toda onda materializada, e é dito assim porque é assim: o
/// id é novo e nenhum recorte o contém. A releitura é a mesma de
/// [`declares_criterion`], então uma onda que por algum caminho já carregue o
/// id não é nomeada. Ordenada e determinística — a lista viaja num relatório
/// comparado byte a byte. Diretório ilegível devolve lista vazia: isto é um
/// aviso, e recusar a adição por uma listagem que não abriu custaria mais do
/// que ele vale.
fn stale_waves(spec_dir: &Path, id: &str) -> Vec<String> {
    let mut stale: Vec<String> = plan_artefacts(spec_dir)
        .into_iter()
        .filter(|path| path.parent().is_some_and(|p| p != spec_dir))
        .filter(|path| !declares_criterion(path, id))
        .filter_map(|path| {
            path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })
        .collect();
    stale.sort();
    stale
}

/// `true` quando `path` já declara `id` — a leitura que responde tanto "esta
/// onda ficou sem o critério?" ([`stale_waves`]) quanto "este id já existe?"
/// (a recusa `duplicate_criterion`), pelo mesmo parser, para as duas não
/// discordarem sobre o que um artefato declara.
fn declares_criterion(path: &Path, id: &str) -> bool {
    mfs::read_to_string(path)
        .map(|body| criteria_of(&body).iter().any(|item| item.id == id))
        .unwrap_or(false)
}

/// Every PLAN artefact under a spec directory that can carry criterion lines:
/// the root `spec.md` / `wave-plan.md` and each `wave-*/spec.md`.
///
/// Este é o conjunto que a recusa `duplicate_criterion` PERGUNTA — todo lugar
/// onde um id poderia já estar. Onde a adição ESCREVE é um subconjunto dele:
/// ver [`write_targets`].
///
/// Named rather than walked. The amendment door walks every markdown two levels
/// down because it only ever rewrites a line that is already there; an ADD
/// writes a line that is not, and the same walk would reach `qa/report.md` and
/// `review/` transcripts — records of a run that finished, into which a new
/// criterion would be forged evidence. Sorted, so the report and the ledger are
/// byte-stable.
fn plan_artefacts(spec_dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = ["spec.md", "wave-plan.md"]
        .into_iter()
        .map(|name| spec_dir.join(name))
        .filter(|p| p.is_file())
        .collect();
    if let Ok(entries) = std::fs::read_dir(spec_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_wave = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("wave-"));
            if path.is_dir() && is_wave && path.join("spec.md").is_file() {
                found.push(path.join("spec.md"));
            }
        }
    }
    found.sort();
    found
}

// ---------------------------------------------------------------------------
// The operation
// ---------------------------------------------------------------------------

/// Add one criterion to `spec` under an explicit project `root`.
///
/// `root` is a PARAMETER, never re-derived from the process working directory:
/// this tool cuts a worktree per work unit, so the command runs off-root as a
/// matter of course — and the unit tests drive it against a temp tree.
pub(crate) fn add(root: &Path, opts: &AcAddOpts) -> AcAddReport {
    let id = normalise_id(&opts.ac);

    // A reason nobody stated is an addition nobody can audit later.
    let reason = opts.reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if reason.is_empty() {
        return AcAddReport::refused(
            opts,
            &id,
            "blank_reason",
            "state WHY the criterion is being added: `--reason \"<sentence>\"`. The reason is the \
             only part of an addition a later reader cannot reconstruct from the diff",
        );
    }

    // A criterion with no statement is a command with no claim beside it — the
    // reader cannot tell whether the command checks what was asked for.
    let statement = opts.statement.split_whitespace().collect::<Vec<_>>().join(" ");
    if statement.is_empty() {
        return AcAddReport::refused(
            opts,
            &id,
            "blank_statement",
            "state WHAT the criterion asserts: `--statement \"when <trigger>, then <outcome>\"`. \
             A command with no statement cannot be checked against what was asked for",
        );
    }

    // A slug with no spec markdown is a typo, not a new spec.
    let Some(spec_file) = qa_run::spec_file_for(root, &opts.spec) else {
        return AcAddReport::refused(
            opts,
            &id,
            "unknown_spec",
            "no spec markdown under `.claude/spec/<slug>/` for that name — check the slug with \
             `mustard-rt run active-specs`",
        );
    };
    let spec_dir = spec_file.parent().unwrap_or(root).to_path_buf();

    let Ok(_markdown) = mfs::read_to_string(&spec_file) else {
        return AcAddReport::refused(
            opts,
            &id,
            "unknown_spec",
            "the spec markdown could not be read — check the file exists and is readable",
        );
    };
    // An id the spec ALREADY carries is an amendment, and amendments have their
    // own door with their own rule. Routing it here would let a replacement skip
    // the supersession record entirely.
    //
    // Every plan artefact is ASKED, not just the root — a wider set than the one
    // the write lands in ([`write_targets`]), and deliberately so: an id a wave
    // already declares is an amendment no matter where it is declared, and a
    // root-only check would admit a second copy of it under the same id.
    let carries_id =
        plan_artefacts(&spec_dir).into_iter().any(|path| declares_criterion(&path, &id));
    if carries_id {
        return AcAddReport::refused(
            opts,
            &id,
            "duplicate_criterion",
            &format!(
                "the spec already declares {id} — changing a criterion that exists is an \
                 AMENDMENT: `mustard-rt run ac-amend --spec {} --ac {id} …`. To add a new one, \
                 pick an id the spec does not carry",
                opts.spec
            ),
        );
    }

    let expect = opts.expect.clone().filter(|e| !e.trim().is_empty());

    // THE gate — the same engine, at the same strictness. The criterion is
    // inserted ABOVE the trailing one, so it is never the exempt position: it
    // owes a red proof like any criterion the plan declared.
    // The `Control:` the CALLER declared, if any — blank is the same as absent,
    // so a shell that expanded an empty variable cannot write an empty control
    // onto the line. Omitted, the record says the control was not declared —
    // a WARN, never a refusal; the next `ac-negative-check` pass sees the
    // markdown's control (if the author adds one) differ from the record and
    // takes it then.
    let control = opts
        .control
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    // WHERE the command runs, which is not always where the spec lives — see
    // `AcAddOpts::proof_tree`. Everything else (reading the spec, rewriting the
    // artefacts, appending to the ledger) stays in THIS tree.
    let proof_root: &Path = opts.proof_tree.as_deref().unwrap_or(root);
    if let Some(tree) = opts.proof_tree.as_deref() {
        if !tree.is_dir() {
            // `error` is a CODE — a closed vocabulary a caller can match on.
            // The path is volatile, so it belongs in `remedy`, which is prose.
            return AcAddReport::refused(
                opts,
                &id,
                "proof_tree_not_a_directory",
                &format!(
                    "`--proof-tree {}` is not a directory — point it at a checkout of this \
                     repository that does not carry the work yet \
                     (`git worktree add --detach <dir> <base-commit>`)",
                    tree.display()
                ),
            );
        }
    }
    let proof_tree_record =
        crate::commands::spec::ac_amend::proof_tree_record(opts.proof_tree.as_deref(), root);
    let mut proof = ac_negative_check::prove_one(
        proof_root,
        &id,
        &opts.command,
        expect.as_deref(),
        control,
        false,
    );
    proof.proof_tree.clone_from(&proof_tree_record);
    if proof.proof != ac_negative_check::Proof::Red {
        let why = proof.reason.clone().unwrap_or_default();
        let mut remedy = format!(
            "the criterion being ADDED does not clear the negative test, so it would join the \
             spec verifying exactly nothing — {why}"
        );
        // GREEN in the current tree is the refusal with a way out the reader
        // cannot see from the reason alone — a criterion added to cover work
        // that ALREADY landed passes here by construction. Same paragraph as
        // the amendment door, from the same function.
        if proof.proof == ac_negative_check::Proof::Green {
            remedy.push_str("\n\n");
            remedy.push_str(&crate::commands::spec::ac_amend::green_in_this_tree_way_out(
                "ac-add",
                opts.proof_tree.as_deref(),
            ));
        }
        let mut report = AcAddReport::refused(opts, &id, "criterion_not_proven", &remedy);
        report.proof = Some(proof);
        return report;
    }

    // Accepted. From here on the writes happen; every one of them is re-read.
    // ONDE elas caem é [`write_targets`] — o pai e a união do QA, nunca uma
    // onda: o plano decide de quem é um critério, e o plano é anterior a este.
    let mut written: Vec<String> = Vec::new();
    for path in write_targets(&spec_dir) {
        let Ok(body) = mfs::read_to_string(&path) else {
            continue;
        };
        let Some(updated) =
            insert_criterion(&body, &id, &statement, &opts.command, expect.as_deref(), control)
        else {
            continue;
        };
        if mfs::write_atomic(&path, updated.as_bytes()).is_err() {
            continue;
        }
        if landed(&path, &id, &opts.command) {
            written.push(ac_negative_check::repo_relative(root, &path));
        }
    }
    written.sort();

    // As ondas que ficaram sem o critério — toda onda materializada, porque o
    // plano que as recortou é anterior a ele. Nomeadas com o remédio, pelo mesmo
    // motivo do `ac-amend`: quem fica sabendo do problema sem o remédio fica
    // com o problema. Alto na stderr, nunca no stdout — a linha JSON é comparada
    // byte a byte.
    let stale_waves = stale_waves(&spec_dir, &id);
    if !stale_waves.is_empty() {
        eprintln!(
            "ac-add: WARN: {id} was added to `spec.md` and `wave-plan.md`, but {n} materialised \
             wave(s) do not carry it ({waves}) — their dispatched `## ACCEPTANCE` will not show \
             the criterion until the plan routes it. Add {id} to the `satisfies` of the wave that \
             owns it in `plan.json`, then re-run `mustard-rt run plan-materialize --spec-dir \
             {spec} --plan <plan.json>`.",
            n = stale_waves.len(),
            waves = stale_waves.join(", "),
            spec = opts.spec,
        );
    }

    let mut report = AcAddReport {
        ok: false,
        spec: opts.spec.clone(),
        ac: id.clone(),
        statement: statement.clone(),
        command: opts.command.clone(),
        expect: expect.clone(),
        proof: Some(proof.clone()),
        written: written.clone(),
        waves_stale: !stale_waves.is_empty(),
        stale_waves,
        ledger: None,
        error: None,
        remedy: None,
    };

    // The ROOT spec is the one artefact that must have changed: it is the list
    // every other reader is derived from. Nothing confirmed there is a lost
    // write, reported.
    let root_path = ac_negative_check::repo_relative(root, &spec_file);
    if !written.contains(&root_path) {
        report.error = Some("write_failed".to_string());
        report.remedy = Some(format!(
            "the criterion was not written into `{root_path}` — re-read the file and check it \
             declares a `## Acceptance Criteria` section"
        ));
        return report;
    }

    let ledger_path = spec_dir.join(AC_PROOF_JSON);
    let mut ledger = read_ledger(&ledger_path);
    ledger.spec = spec_dir
        .file_name()
        .map_or_else(|| opts.spec.clone(), |n| n.to_string_lossy().into_owned());
    // The proof record joins the ledger so the approval door accepts the new
    // criterion on the evidence it just produced, never on a later re-run.
    ledger.criteria.retain(|c| c.id != id);
    ledger.criteria.push(proof);
    ledger.criteria.sort_by(|a, b| a.id.cmp(&b.id));

    let entry = Addition {
        id: id.clone(),
        at: mustard_core::time::now_iso8601(),
        reason,
        statement,
        command: opts.command.clone(),
        expect,
        wrote: written,
    };
    if let Ok(value) = serde_json::to_value(&entry) {
        ledger.additions.push(value);
    }
    if !write_ledger(&ledger_path, &ledger) {
        report.error = Some("ledger_write_failed".to_string());
        report.remedy = Some(
            "the artefacts were written but the proof ledger did not land — re-run \
             `mustard-rt run ac-negative-check --spec <slug>` to rebuild it"
                .to_string(),
        );
        return report;
    }
    report.ledger = Some(ac_negative_check::repo_relative(root, &ledger_path));
    report.ok = true;
    report
}

/// The process exit code for a finished report: `0` accepted, `1` refused.
fn exit_code(report: &AcAddReport) -> i32 {
    i32::from(!report.ok)
}

/// CLI entry — `mustard-rt run ac-add`.
pub fn run(opts: AcAddOpts) {
    let root = PathBuf::from(crate::shared::context::project_dir());
    let report = add(&root, &opts);
    let body = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
    println!("{body}");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    std::process::exit(exit_code(&report));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use tempfile::tempdir;

    /// A command that comes back RED on both shells (`cmd.exe` and `sh`): the
    /// directory does not exist, so `cd` exits non-zero.
    const RED_COMMAND: &str = "cd no-such-directory-abc";
    /// A command that comes back GREEN on both shells — `cd .` is a builtin
    /// everywhere and always succeeds.
    const GREEN_COMMAND: &str = "cd .";

    /// Seed `<root>/.claude/spec/<spec>/` with a root `spec.md`, the frozen
    /// `wave-plan.md` and a wave artefact — the three shapes a plan artefact
    /// takes — plus a `qa/report.md` transcript that must NOT be written into.
    fn seed(root: &Path, spec: &str) -> PathBuf {
        let dir = root.join(".claude").join("spec").join(spec);
        std::fs::create_dir_all(dir.join("wave-1-rt")).unwrap();
        std::fs::create_dir_all(dir.join("qa")).unwrap();
        let criteria = format!(
            "## Acceptance Criteria\n\
             - **AC-1** — when the work lands, then the behaviour holds.\n  Command: `{RED_COMMAND}`\n\
             - **AC-2** — the project build passes green.\n  Command: `{GREEN_COMMAND}`\n"
        );
        std::fs::write(dir.join("spec.md"), format!("# S\n\n{criteria}")).unwrap();
        std::fs::write(dir.join("wave-plan.md"), format!("# Plan\n\n{criteria}")).unwrap();
        std::fs::write(
            dir.join("wave-1-rt").join("spec.md"),
            format!("# Wave 1\n\n{criteria}"),
        )
        .unwrap();
        std::fs::write(dir.join("qa").join("report.md"), format!("# QA\n\n{criteria}")).unwrap();
        dir
    }

    fn opts(spec: &str, ac: &str, command: &str) -> AcAddOpts {
        AcAddOpts {
            spec: spec.to_string(),
            ac: ac.to_string(),
            statement: "when the finding is fixed, then the gate refuses the old shape".to_string(),
            command: command.to_string(),
            expect: None,
            reason: "the review found a defect no criterion names".to_string(),
            // No control by default: the fixtures' commands are not test
            // runners, so nothing is owed. `--control` has its own test below.
            control: None,
            // The default door: the proof is taken in the tree the spec lives
            // in. `--proof-tree` is exercised by its own test below.
            proof_tree: None,
        }
    }

    /// `--proof-tree` — a criterion added to cover work that ALREADY LANDED.
    ///
    /// The pair below is the whole argument for the flag existing. The command
    /// asserts a file the work created: in the current tree it is GREEN, so the
    /// door refuses it — correctly, because a criterion that already passes
    /// proves nothing. Pointed at a checkout that predates the work, the same
    /// command comes back RED and the addition is accepted.
    ///
    /// Without this, the only way to cover landed work was to hide it, add the
    /// criterion, and put it back — which works and leaves no trace of where
    /// the evidence came from.
    #[test]
    fn proof_tree_takes_the_red_where_the_work_is_absent() {
        let dir = tempdir().unwrap();
        seed(dir.path(), "added");
        // The "work": a file that exists HERE and not in the older checkout.
        std::fs::write(dir.path().join("landed.txt"), "the work").unwrap();
        let older = tempdir().unwrap();

        // Refused in this tree — the behaviour is present, so the command is
        // green and the criterion would verify nothing.
        let o = opts("added", "AC-3", "test -f landed.txt");
        let refused = add(dir.path(), &o);
        assert!(!refused.ok, "a green criterion must never be accepted");
        assert_eq!(refused.error.as_deref(), Some("criterion_not_proven"));

        // Accepted against the checkout that predates the work.
        let mut o = opts("added", "AC-3", "test -f landed.txt");
        o.proof_tree = Some(older.path().to_path_buf());
        let report = add(dir.path(), &o);
        assert!(
            report.ok,
            "unexpected refusal: {:?} / {:?}",
            report.error, report.remedy
        );
        let proof = report.proof.clone().expect("the addition records its proof");
        assert_eq!(proof.proof, ac_negative_check::Proof::Red, "{proof:?}");
        // The spec was still rewritten HERE — only the command ran elsewhere.
        assert!(
            report
                .written
                .iter()
                .any(|w| w.ends_with(".claude/spec/added/spec.md")),
            "the spec must be rewritten in the current tree: {:?}",
            report.written
        );
    }

    /// Um critério NOVO cujo comando é executor de teste FILTRADO e que não
    /// declara `--control` é julgado pelo COMANDO, como qualquer outro: a porta
    /// não o recusa por falta de controle.
    ///
    /// Substitui `a_filtered_runner_criterion_owes_a_control_and_the_flag_clears_it`,
    /// que trancava a tese "opcional deixa de ser opcional" com `!refused.ok`,
    /// `refused.error == Some("control_required")` e `refused.written.is_empty()`
    /// para o caso (a) abaixo — asserções que agora falham por desenho. O que
    /// fica: a flag `--control` continua sendo tomada, registrada e escrita na
    /// linha quando declarada, e um comando que não é executor continua não
    /// devendo nada.
    #[test]
    fn a_filtered_runner_criterion_without_a_control_is_judged_by_its_command() {
        // Um executor de teste FILTRADO: `my_new_case` é seleção por nome.
        const FILTERED: &str = "cargo test -p mustard-rt my_new_case";

        // (a) Sem `--control`: o comando é LANÇADO e o veredito é o dele. Que
        // cor sai depende desta máquina (o cargo sem `Cargo.toml` sai vermelho;
        // um cargo ausente sai 127), e as duas leituras são honestas — o que
        // não pode acontecer é a recusa por exigência de controle.
        let a = tempdir().unwrap();
        seed(a.path(), "added");
        let judged = add(a.path(), &opts("added", "AC-3", FILTERED));
        assert_ne!(
            judged.error.as_deref(),
            Some("control_required"),
            "não existe mais essa recusa: {judged:?}",
        );
        let proof = judged.proof.as_ref().expect("a prova é registrada seja qual for o veredito");
        assert!(proof.exit.is_some(), "o comando foi lançado — nada o recusou antes do shell: {judged:?}");
        assert_eq!(proof.control, ac_negative_check::Control::NotDeclared, "{judged:?}");
        assert!(
            !judged.remedy.as_deref().unwrap_or_default().contains("--control"),
            "a recusa, se houver, é sobre o comando: {judged:?}",
        );

        // (b) Com `--control`: o controle declarado chega ao registro da prova
        // — a flag continua viva, como entrada OPCIONAL.
        let b = tempdir().unwrap();
        seed(b.path(), "added");
        let mut with_control = opts("added", "AC-3", FILTERED);
        with_control.control = Some(GREEN_COMMAND.to_string());
        let taken = add(b.path(), &with_control);
        assert_eq!(
            taken.proof.as_ref().and_then(|p| p.control_command.as_deref()),
            Some(GREEN_COMMAND),
            "o controle declarado chega ao registro da prova: {taken:?}",
        );
        assert_eq!(
            taken.proof.as_ref().map(|p| p.control),
            Some(ac_negative_check::Control::Green),
            "e foi tomado no mesmo passo: {taken:?}",
        );

        // (c) A outra metade: um comando que NÃO é executor de teste continua
        // sem dever controle nenhum, e passa.
        let c = tempdir().unwrap();
        seed(c.path(), "added");
        let plain = add(c.path(), &opts("added", "AC-3", RED_COMMAND));
        assert!(plain.ok, "{plain:?}");
    }

    /// O ROUND TRIP que faltava ao `--control` desta porta: o controle declarado
    /// tem de chegar à LINHA do critério novo, porque é o MARKDOWN que a passada
    /// seguinte do `ac-negative-check` lê.
    ///
    /// A regressão que isto tranca: a flag chegava ao motor e ao
    /// `control_command` do ledger, e o bloco escrito na spec só tinha
    /// `Command:` e `Expect:`. O critério entrava com controle no registro e sem
    /// controle na spec, e o portão de aprovação o recusava outra vez.
    ///
    /// Dois lados, e o veredito de cada um vem do predicado do lint de rascunho
    /// (`test_runner_has_selector`, o que nomeia `test-ac-no-control`)
    /// alimentado pela releitura.
    #[test]
    fn an_added_control_lands_on_the_line_the_next_gate_reads() {
        use crate::commands::review::analyze_validation::test_runner_has_selector;
        // Um executor de teste FILTRADO — a forma que o lint de rascunho nomeia
        // quando não acha `Control:` na linha.
        const FILTERED: &str = "cargo test -p mustard-rt my_new_case";
        let md = "## Acceptance Criteria\n- **AC-1** — build green.\n  Command: `cd a`\n";

        // COM controle: a releitura acha o marcador e o portão nada cobra.
        let with = insert_criterion(md, "AC-9", "when x, then y", FILTERED, None, Some(GREEN_COMMAND))
            .expect("the criterion was inserted");
        let added = criteria_of(&with)
            .into_iter()
            .find(|i| i.id == "AC-9")
            .unwrap_or_else(|| panic!("AC-9 unreadable after insertion: {with:?}"));
        assert_eq!(
            added.control.as_deref(),
            Some(GREEN_COMMAND),
            "o `Control:` tem de estar NA LINHA, não só no registro da prova: {with:?}",
        );
        assert!(
            test_runner_has_selector(&added.command) && added.control.is_some(),
            "a passada seguinte lê o controle da linha: {with:?}",
        );

        // SEM ele: o critério fica sem controle e o lint volta a nomeá-lo.
        let without = insert_criterion(md, "AC-9", "when x, then y", FILTERED, None, None)
            .expect("the criterion was inserted");
        let bare = criteria_of(&without)
            .into_iter()
            .find(|i| i.id == "AC-9")
            .unwrap_or_else(|| panic!("AC-9 unreadable after insertion: {without:?}"));
        assert_eq!(bare.control, None, "{without:?}");
        assert!(
            test_runner_has_selector(&bare.command) && bare.control.is_none(),
            "sem controle o lint TEM de nomear — senão este teste não mede nada: {without:?}",
        );

        // Fim a fim, no disco: no `spec.md` do pai e na lista que o QA executa —
        // os dois artefatos que TODA adição toca. A onda fica de fora porque o
        // plano não a nomeia dona do id novo (ver `write_targets`), e é NOMEADA
        // como desatualizada.
        let dir = tempdir().unwrap();
        let spec_dir = seed(dir.path(), "added");
        let mut o = opts("added", "AC-3", RED_COMMAND);
        o.control = Some(GREEN_COMMAND.to_string());
        let report = add(dir.path(), &o);
        assert!(report.ok, "unexpected refusal: {:?} / {:?}", report.error, report.remedy);
        assert!(report.waves_stale && report.stale_waves == ["wave-1-rt"], "{report:?}");
        for name in ["spec.md", "wave-plan.md"] {
            let body = std::fs::read_to_string(spec_dir.join(name)).unwrap();
            let item = criteria_of(&body)
                .into_iter()
                .find(|i| i.id == "AC-3")
                .unwrap_or_else(|| panic!("{name}: AC-3 unreadable"));
            assert_eq!(item.command, RED_COMMAND, "{name}");
            assert_eq!(
                item.control.as_deref(),
                Some(GREEN_COMMAND),
                "{name}: o `Control:` tem de estar na linha",
            );
        }
    }

    /// A `--proof-tree` that is not a directory is refused by CODE, before any
    /// artefact is touched.
    #[test]
    fn proof_tree_that_is_not_a_directory_is_refused() {
        let dir = tempdir().unwrap();
        seed(dir.path(), "added");
        let mut o = opts("added", "AC-3", RED_COMMAND);
        o.proof_tree = Some(dir.path().join("nope"));
        let report = add(dir.path(), &o);
        assert!(!report.ok);
        assert_eq!(report.error.as_deref(), Some("proof_tree_not_a_directory"));
        assert!(report.written.is_empty(), "a refusal writes nothing");
    }

    /// AC-1 — the accepted direction. A criterion the spec does not carry is
    /// introduced, and it lands in the artefacts and in the ledger ONLY after
    /// taking the same red proof a planned criterion takes.
    ///
    /// Three claims, all load-bearing:
    ///
    /// 1. **The proof came first.** The ledger records the new id with a RED
    ///    proof — the evidence the approval gate reads.
    /// 2. **It lands where it belongs**: the root `spec.md` and the frozen
    ///    `wave-plan.md`, the list QA executes. The wave scaffold is NOT one of
    ///    them — it carries only the criteria the PLAN routes to that wave, and
    ///    the plan predates a brand new id (see `write_targets`) — so the
    ///    report NAMES it as stale, with the flag `ac-amend` uses for the same
    ///    fact. The `qa/` transcript is left alone too — it is a record of a
    ///    run.
    /// 3. **The trailing criterion stays trailing**, so the positional exemption
    ///    does not move onto the criterion just added.
    #[test]
    fn ac_add_lands_only_after_taking_the_proof() {
        let dir = tempdir().unwrap();
        let spec_dir = seed(dir.path(), "added");
        let qa_before = std::fs::read_to_string(spec_dir.join("qa").join("report.md")).unwrap();
        let wave_before =
            std::fs::read_to_string(spec_dir.join("wave-1-rt").join("spec.md")).unwrap();

        let mut o = opts("added", "AC-3", RED_COMMAND);
        o.expect = Some("1 passed".to_string());
        let report = add(dir.path(), &o);

        assert!(report.ok, "unexpected refusal: {:?} / {:?}", report.error, report.remedy);
        assert_eq!(exit_code(&report), 0);
        let proof = report.proof.clone().expect("the addition records its proof");
        assert_eq!(proof.proof, ac_negative_check::Proof::Red, "{proof:?}");

        // The root and the frozen plan, and ONLY those.
        assert_eq!(
            report.written,
            vec![
                ".claude/spec/added/spec.md".to_string(),
                ".claude/spec/added/wave-plan.md".to_string(),
            ],
            "the root and the frozen plan — the plan routes no new id to the wave"
        );
        assert!(report.waves_stale, "the wave the plan did not reach is named: {report:?}");
        assert_eq!(report.stale_waves, ["wave-1-rt"], "{report:?}");
        let printed = serde_json::to_string(&report).unwrap();
        assert!(
            printed.contains("\"wavesStale\":true") && printed.contains("\"staleWaves\":[\"wave-1-rt\"]"),
            "the same keys `ac-amend` prints, so one reader serves both doors: {printed}"
        );
        assert_eq!(
            std::fs::read_to_string(spec_dir.join("qa").join("report.md")).unwrap(),
            qa_before,
            "a finished run's transcript is a record, never an artefact to write into"
        );
        assert_eq!(
            std::fs::read_to_string(spec_dir.join("wave-1-rt").join("spec.md")).unwrap(),
            wave_before,
            "and the wave scaffold is byte-identical: it never satisfied AC-3"
        );

        for name in ["spec.md", "wave-plan.md"] {
            let body = std::fs::read_to_string(spec_dir.join(name)).unwrap();
            let items = criteria_of(&body);
            let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
            assert_eq!(ids, ["AC-1", "AC-3", "AC-2"], "{name}: the build criterion stays LAST");
            let added = items.iter().find(|i| i.id == "AC-3").unwrap();
            assert_eq!(added.command, RED_COMMAND, "{name}");
            assert_eq!(added.expect.as_deref(), Some("1 passed"), "{name}");
            assert!(
                added.statement.contains("the gate refuses the old shape"),
                "{name}: {}",
                added.statement
            );
        }

        // The ledger: the new criterion's proof is there for the approval gate,
        // and the addition is recorded APART from the amendment history.
        let ledger: Value =
            serde_json::from_str(&std::fs::read_to_string(spec_dir.join(AC_PROOF_JSON)).unwrap())
                .unwrap();
        let record = ledger["criteria"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "AC-3")
            .unwrap();
        assert_eq!(record["verdict"], "proven");
        assert_eq!(record["proof"], "red");
        assert!(
            ledger["amendments"].as_array().is_some_and(Vec::is_empty),
            "an addition supersedes nothing: {ledger}"
        );
        let addition = &ledger["additions"].as_array().unwrap()[0];
        assert_eq!(addition["id"], "AC-3");
        assert_eq!(addition["reason"], "the review found a defect no criterion names");
        assert!(
            addition["at"].as_str().is_some_and(|s| s.ends_with('Z')),
            "the timestamp lives in the ledger: {addition}"
        );
        // ...and NOT on stdout, which is snapshot-compared.
        let printed = serde_json::to_string(&report).unwrap();
        assert!(!printed.contains("\"at\""), "no timestamp on stdout: {printed}");
    }

    /// AC-2 — the load-bearing refusal. A criterion whose command ALREADY passes
    /// against the tree as it is verifies nothing, so the door refuses it and
    /// NOTHING is written: not the artefacts, not the ledger.
    ///
    /// Two-sided within one test: the same seed accepts a RED criterion, so the
    /// refusal cannot pass by the door being inert. The other four refusals —
    /// blank reason, blank statement, unknown spec and an id the spec already
    /// carries — are checked the same way, each writing nothing.
    #[test]
    fn ac_add_refuses_a_criterion_that_cannot_fail() {
        let dir = tempdir().unwrap();
        let spec_dir = seed(dir.path(), "refused");
        let before = std::fs::read_to_string(spec_dir.join("spec.md")).unwrap();
        let plan_before = std::fs::read_to_string(spec_dir.join("wave-plan.md")).unwrap();

        let report = add(dir.path(), &opts("refused", "AC-3", GREEN_COMMAND));
        assert!(!report.ok, "a criterion that passes now must be refused");
        assert_eq!(report.error.as_deref(), Some("criterion_not_proven"));
        let remedy = report.remedy.clone().unwrap_or_default();
        assert!(remedy.contains("verifying exactly nothing"), "{remedy}");
        assert!(remedy.contains("rewrite the command"), "the engine's own remedy: {remedy}");
        // And the WAY OUT: a criterion added to cover work that already landed
        // is green here by construction, and the refusal must say where the
        // red can still be taken — naming THIS door, not the amendment's.
        assert!(
            remedy.contains("git worktree add --detach <dir> <commit-before-the-work>")
                && remedy.contains("mustard-rt run ac-add … --proof-tree <dir>")
                && remedy.contains("git worktree remove <dir>"),
            "the refusal gives the --proof-tree recipe verbatim: {remedy}"
        );
        assert_eq!(exit_code(&report), 1, "a refusal exits non-zero");
        assert!(report.written.is_empty(), "{:?}", report.written);
        assert_eq!(
            std::fs::read_to_string(spec_dir.join("spec.md")).unwrap(),
            before,
            "the root spec must be byte-identical after a refusal"
        );
        assert_eq!(
            std::fs::read_to_string(spec_dir.join("wave-plan.md")).unwrap(),
            plan_before,
            "and so must the frozen plan"
        );
        assert!(
            !spec_dir.join(AC_PROOF_JSON).exists(),
            "a refusal must not even create the ledger"
        );

        // The input refusals, each writing nothing.
        let blank_reason = AcAddOpts { reason: "  ".to_string(), ..opts("refused", "AC-3", RED_COMMAND) };
        let blank_statement =
            AcAddOpts { statement: String::new(), ..opts("refused", "AC-3", RED_COMMAND) };
        for (o, code) in [
            (blank_reason, "blank_reason"),
            (blank_statement, "blank_statement"),
            (opts("no-such-spec", "AC-3", RED_COMMAND), "unknown_spec"),
            (opts("refused", "AC-1", RED_COMMAND), "duplicate_criterion"),
        ] {
            let report = add(dir.path(), &o);
            assert!(!report.ok, "{code} must refuse");
            assert_eq!(report.error.as_deref(), Some(code));
            assert!(
                report.remedy.is_some_and(|r| !r.trim().is_empty()),
                "{code} must name what to do about it"
            );
            assert!(report.written.is_empty(), "{code} wrote an artefact");
            assert!(!spec_dir.join(AC_PROOF_JSON).exists(), "{code} wrote a ledger");
        }
        // The duplicate refusal points at the door that DOES change an existing
        // criterion, instead of leaving the caller to guess.
        let dup = add(dir.path(), &opts("refused", "AC-1", RED_COMMAND));
        let remedy = dup.remedy.unwrap_or_default();
        assert!(remedy.contains("ac-amend"), "{remedy}");
        assert!(remedy.contains("AMENDMENT"), "{remedy}");

        // An id only a WAVE artefact carries is a duplicate too. The duplicate
        // check reads every artefact, so reading the root alone would insert
        // a second copy under an id the dispatched agent already reads — a
        // duplicate in the one file nobody opens.
        let wave = spec_dir.join("wave-1-rt").join("spec.md");
        let wave_before = std::fs::read_to_string(&wave).unwrap();
        std::fs::write(
            &wave,
            wave_before.replace(
                "- **AC-2**",
                &format!(
                    "- **AC-9** — only the wave carries this one.\n  Command: `{RED_COMMAND}`\n\
                     - **AC-2**"
                ),
            ),
        )
        .unwrap();
        let wave_only = add(dir.path(), &opts("refused", "AC-9", RED_COMMAND));
        assert_eq!(
            wave_only.error.as_deref(),
            Some("duplicate_criterion"),
            "an id a wave artefact declares is not free to be added again"
        );
        assert!(wave_only.written.is_empty(), "{:?}", wave_only.written);

        // Two-sided: the same seed accepts a RED criterion.
        let ok = add(dir.path(), &opts("refused", "AC-3", RED_COMMAND));
        assert!(ok.ok, "unexpected refusal: {:?} / {:?}", ok.error, ok.remedy);
    }

    /// Round trip on the insertion: what is written is what the SAME parser
    /// reads back, in every document shape — with and without an `Expect:`
    /// line, over LF and CRLF, and into a section that carries no criterion yet.
    #[test]
    fn inserted_blocks_parse_back_as_the_new_criterion() {
        for (original, expect) in [
            ("## Acceptance Criteria\n- **AC-1** — first.\n  Command: `cd a`\n", None),
            (
                "## Acceptance Criteria\n- **AC-1** — first.\n  Command: `cd a`\n",
                Some("2 passed"),
            ),
            (
                "## Acceptance Criteria\r\n- **AC-1** — first.\r\n  Command: `cd a`\r\n",
                Some("2 passed"),
            ),
            // A section with no criterion at all: the block is the whole list.
            ("## Acceptance Criteria\n\nnone yet.\n\n## Files\n\n- `a.rs`\n", None),
        ] {
            let updated = insert_criterion(
                original,
                "AC-9",
                "when x, then y",
                "cd new",
                expect,
                Some(GREEN_COMMAND),
            )
            .unwrap_or_else(|| panic!("nothing inserted into {original:?}"));
            let items = criteria_of(&updated);
            let added = items
                .iter()
                .find(|i| i.id == "AC-9")
                .unwrap_or_else(|| panic!("AC-9 unreadable after insertion: {updated:?}"));
            assert_eq!(added.command, "cd new", "{updated:?}");
            assert_eq!(added.expect.as_deref(), expect, "{updated:?}");
            assert_eq!(added.control.as_deref(), Some(GREEN_COMMAND), "{updated:?}");
            assert_eq!(added.statement, "when x, then y", "{updated:?}");
            if original.contains("\r\n") {
                // `split('\n')` and not `lines()`: the latter strips the `\r`
                // this assertion is looking for.
                assert!(
                    !updated
                        .split('\n')
                        .any(|l| (l.contains("AC-9") || l.contains("Control:"))
                            && !l.ends_with('\r')),
                    "a CRLF document must not gain lone-LF lines: {updated:?}"
                );
            }
            // The sibling section is never disturbed.
            if original.contains("## Files") {
                assert!(updated.contains("## Files\n\n- `a.rs`"), "{updated:?}");
            }
        }
        // A document with no acceptance-criteria section is left alone entirely
        // — a linha não tem onde cair. (QUAIS documentos são visitados é outra
        // pergunta, e ela é de `write_targets`.)
        assert!(
            insert_criterion("# Wave\n\n## Tasks\n\n- do it\n", "AC-9", "s", "c", None, None)
                .is_none()
        );
    }

    /// The new criterion never takes the trailing slot, because the trailing
    /// slot is EXEMPT from the proof — moving it would demand a red from a
    /// build command that is green by design and hand the exemption to the
    /// criterion that most needs proving.
    #[test]
    fn the_addition_never_takes_the_exempt_trailing_slot() {
        let md = "## Acceptance Criteria\n\
                  - **AC-1** — first.\n  Command: `cd a`\n\
                  - **AC-2** — build green.\n  Command: `cargo build`\n";
        let updated = insert_criterion(md, "AC-3", "when x, then y", "cd new", None, None)
            .expect("the criterion was inserted");
        let items = criteria_of(&updated);
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["AC-1", "AC-3", "AC-2"], "{updated:?}");
        let last = items.len() - 1;
        assert!(
            ac_negative_check::is_exempt(last, items.len()) && items[last].id == "AC-2",
            "the build criterion keeps the exemption: {ids:?}"
        );
    }

    /// O PAR, ponta a ponta: um layout de ondas materializado pelo MATERIALIZADOR
    /// de verdade, uma adição pela porta de verdade, e a régua de uma onda lida
    /// de volta pelo RENDERIZADOR de verdade.
    ///
    /// A regressão que isto tranca: a adição escrevia em todo artefato de plano
    /// porque `insert_criterion` "deixa em paz o artefato que não declara a
    /// seção" — e a onda 1 desta mesma unidade fez o `wave-scaffold` escrever
    /// `## Acceptance Criteria` em toda onda que satisfaz alguma coisa, então
    /// esse `None` parou de acontecer. O critério novo entrava nas duas ondas,
    /// sob a frase "estes critérios são o JUIZ desta onda", numa onda cujo
    /// `satisfies` nunca o nomeou.
    ///
    /// Medido no `## ACCEPTANCE` renderizado, não no arquivo: é lá que o ruído
    /// chega ao agente despachado.
    #[test]
    fn an_addition_does_not_splice_itself_into_a_wave_that_does_not_satisfy_it() {
        use crate::commands::agent::render::sections::read_wave_acceptance;
        use crate::commands::wave::wave_scaffold::scaffold;

        let dir = tempdir().unwrap();
        let root = dir.path();
        let spec_dir = root.join(".claude").join("spec").join("multi");
        std::fs::create_dir_all(&spec_dir).unwrap();
        std::fs::write(
            spec_dir.join("spec.md"),
            format!(
                "# Multi\n\n## Acceptance Criteria\n\n\
                 - **AC-1** — a onda 1 entrega alpha.\n  Command: `{RED_COMMAND}`\n\
                 - **AC-2** — a onda 2 entrega beta.\n  Command: `{RED_COMMAND}`\n"
            ),
        )
        .unwrap();
        let plan_path = spec_dir.join("plan.json");
        std::fs::write(
            &plan_path,
            serde_json::to_string(&serde_json::json!({
                "total_waves": 2,
                "lang": "en-US",
                "waves": [
                    { "n": 1, "role": "rt", "summary": "s", "tasks": ["do alpha"],
                      "files": ["src/alpha.rs"], "satisfies": ["AC-1"] },
                    { "n": 2, "role": "cli", "summary": "s", "tasks": ["do beta"],
                      "files": ["src/beta.rs"], "satisfies": ["AC-2"] }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let _ = scaffold(&spec_dir, &plan_path);

        // Precondição: as duas ondas materializaram régua, senão o teste mediria
        // o silêncio de um layout que nem existe.
        for (wave, mine) in [("wave-1-rt", "**AC-1**"), ("wave-2-cli", "**AC-2**")] {
            let body = read_wave_acceptance(&spec_dir.join(wave).join("spec.md"));
            assert!(body.contains(mine), "{wave} não materializou a régua dela: {body}");
        }

        let report = add(root, &opts("multi", "AC-9", RED_COMMAND));
        assert!(report.ok, "unexpected refusal: {:?} / {:?}", report.error, report.remedy);

        // O critério novo está onde pertence: no pai, e em NENHUMA onda.
        assert!(
            report.written.contains(&".claude/spec/multi/spec.md".to_string()),
            "o pai é o artefato de onde todo leitor deriva: {:?}",
            report.written,
        );
        assert!(
            !report.written.iter().any(|w| w.contains("/wave-")),
            "nenhum spec de onda podia ser escrito: {:?}",
            report.written,
        );

        // …e o `## ACCEPTANCE` renderizado de CADA onda continua sendo só o dela.
        for (wave, mine) in [("wave-1-rt", "**AC-1**"), ("wave-2-cli", "**AC-2**")] {
            let rendered = read_wave_acceptance(&spec_dir.join(wave).join("spec.md"));
            assert!(rendered.contains(mine), "{wave} perdeu a régua dela: {rendered}");
            assert!(
                !rendered.contains("AC-9"),
                "{wave} não satisfaz AC-9 e recebeu o critério assim mesmo — o ruído \
                 por onda de volta, sob \"JUDGE of this wave\": {rendered}",
            );
        }
    }

    /// Ids are normalised through the amendment door's own rule, so `--ac 3`
    /// and `--ac ac-3` reach the same criterion through either door.
    #[test]
    fn criterion_ids_are_normalised_through_one_rule() {
        let dir = tempdir().unwrap();
        seed(dir.path(), "norm");
        let report = add(dir.path(), &opts("norm", "3", RED_COMMAND));
        assert!(report.ok, "unexpected refusal: {:?}", report.error);
        assert_eq!(report.ac, "AC-3");
        // And the lowercase spelling of an id that now exists is a duplicate.
        let dup = add(dir.path(), &opts("norm", "ac-3", RED_COMMAND));
        assert_eq!(dup.error.as_deref(), Some("duplicate_criterion"));
    }
}
