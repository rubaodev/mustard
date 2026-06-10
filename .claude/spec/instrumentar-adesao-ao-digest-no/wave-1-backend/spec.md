# wave-1-backend

## Resumo

Instrumentar a adesão ao digest no rt: marco analyze.digest.used em feature.rs, comando digest-adherence-finalize que dobra a janela da sessão num analyze.digest.summary escopado ao spec, classificador is_source_file, classify_kind para analyze.*, extensão do metrics_observer para gravar path em Grep/Glob, e prosa SKILL.

## Rede

- Pai: [[instrumentar-adesao-ao-digest-no]]

## Tarefas

- [ ] Criar apps/rt/src/util/source_class.rs com is_source_file(path)->bool: true para fonte (.rs/.ts/.tsx/.js/.jsx/.py/.go/.svelte/etc.), false para config/doc/lock (.md/.json/.lock/.toml/.yaml/.yml e nomes como Cargo.toml, mustard.json, pnpm-lock.yaml); decidir por extensão + nomes-âncora; declarar pub mod source_class em apps/rt/src/util/mod.rs.
- [ ] Em apps/rt/src/commands/feature.rs: no braço Ok(q) do digest_query, construir um HarnessEvent event="analyze.digest.used" (session_id via shared::context::session_id, spec via current_spec — pode ser None), payload com queryTerms/miss, e rotear via shared::events::route::emit(&project_dir, &ev); manter a saída stdout byte-estável intacta (emitir o evento ANTES do println).
- [ ] Criar apps/rt/src/commands/agent/digest_adherence_finalize.rs: resolver session_id, ler os eventos da sessão de .claude/.session/<id>/.events/, achar o primeiro analyze.digest.used por ts (digestUsed = existe?), contar tool.use com tool in {Read,Grep,Glob} cujo target.file (Read) ou target.path (Grep/Glob) seja is_source_file e ts < ts do primeiro digest (ou todos se digest nunca rodou) = sourceReadsBeforeDigest, sourceReadsTotal = todos os source reads; emitir analyze.digest.summary {spec,digestUsed,sourceReadsBeforeDigest,sourceReadsTotal} com --spec via route::emit; imprimir o mesmo JSON ordenado em stdout; sem panic quando não há eventos (digestUsed=false, contagens 0).
- [ ] Declarar o módulo novo em apps/rt/src/commands/agent/mod.rs.
- [ ] Em apps/rt/src/commands/mod.rs: adicionar a variante DigestAdherenceFinalize { spec: String } ao enum RunCmd com #[command(name = "digest-adherence-finalize")], e o braço correspondente em dispatch() chamando agent::digest_adherence_finalize::run(&spec).
- [ ] Em apps/rt/src/shared/events/route.rs: adicionar else if event_name.starts_with("analyze.") { "analyze" } em classify_kind e um teste que classifica analyze.digest.summary/analyze.digest.used como "analyze".
- [ ] Em apps/rt/src/hooks/task/metrics_observer.rs: estender o bloco target com tool_input.get("path") (Grep) e o diretório de Glob, gravando target.path, para que o finalize classifique Grep/Glob por caminho; adicionar teste do payload.
- [ ] Em apps/cli/templates/commands/mustard/feature/SKILL.md: após o passo 2 do PLAN (spec-draft), inserir uma linha que roda mustard-rt run digest-adherence-finalize --spec <slug> (slug recém-nascido), documentando que é fire-and-forget e não bloqueia.
- [ ] Em apps/cli/templates/commands/mustard/bugfix/SKILL.md: no Full Path (passo 3), após o spec existir e ter slug, inserir a mesma chamada digest-adherence-finalize --spec <slug>; deixar explícito que o Fast Path (sem spec) não emite o resumo.

## Arquivos

- `apps/rt/src/util/source_class.rs`
- `apps/rt/src/util/mod.rs`
- `apps/rt/src/commands/feature.rs`
- `apps/rt/src/commands/agent/digest_adherence_finalize.rs`
- `apps/rt/src/commands/agent/mod.rs`
- `apps/rt/src/commands/mod.rs`
- `apps/rt/src/shared/events/route.rs`
- `apps/rt/src/hooks/task/metrics_observer.rs`
- `apps/cli/templates/commands/mustard/feature/SKILL.md`
- `apps/cli/templates/commands/mustard/bugfix/SKILL.md`
