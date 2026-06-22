# Tactical Fix: digest-adherence-finalize cego ao sink do spec: com binding sessao-spec ativo o route::emit re-roteia analyze.digest.used para .claude/spec/<spec>/.events e o finalize (que le so .claude/.session/<id>/.events) reporta digestUsed=false — falso negativo no badge; fix: varrer tambem o sink do spec filtrando por session_id

## Contexto

Tactical fix derivado de [[instrumentar-adesao-ao-digest-no]].

## Critérios de Aceitação

- **AC-1** — finalize funde os eventos do sink do spec (filtrados por session_id) com os do sink da sessão; digest usado pós-binding deixa de ser falso negativo
  Command: `cargo test -p mustard-rt digest_adherence`
- **AC-2** — suíte de classificação de fonte segue verde
  Command: `cargo test -p mustard-rt source_class`

## Arquivos

- `apps/rt/src/commands/agent/digest_adherence_finalize.rs`

<!-- wikilinks-footer-start -->
- [instrumentar-adesao-ao-digest-no](?) ⚠ unresolved
<!-- wikilinks-footer-end -->