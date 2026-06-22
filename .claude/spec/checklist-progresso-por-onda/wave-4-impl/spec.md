# wave-4-impl

## Summary

Templates/SKILL (apps/cli/templates + .claude/skills): instruir o PLAN de onda a popular a checklist por onda; remover os cabecalhos de lifecycle legados que a SKILL pipeline-execution ainda menciona no markdown. Subprojeto: apps/cli.

## Network

- Parent: [[checklist-progresso-por-onda]]
- Depends on: [[wave-2-impl]]

## Arquivos

- `apps/cli/templates/skills/pipeline-execution/SKILL.md` — instruir o PLAN de onda a popular a checklist por onda; remover os cabeçalhos de lifecycle legados (`Status:`/`Phase:`/`Scope:` como texto no markdown).
- `.claude/skills/pipeline-execution/SKILL.md` — espelhar a mesma mudança (cópia local).

## Tarefas

1. **Instruir checklist por onda:** atualizar a SKILL para que, no Full, o PLAN de cada onda popule a checklist (agora no `meta.json` da onda, conforme Ondas 1-2).
2. **Remover cabeçalhos legados:** tirar as menções a `Status: draft` / `Phase: PLAN` / `Scope: full` como cabeçalhos no markdown — o lifecycle vive só no `meta.json` (alinhar ao design atual).
3. **Espelhar:** aplicar a mesma edição na cópia `.claude/skills/pipeline-execution/SKILL.md` (template é a fonte; a cópia local é efêmera mas mantida coerente).

## Critérios de Aceitação

- **AC-1** — A SKILL pipeline-execution (template) instrui checklist por onda e não menciona mais `Status: draft`/`Phase: PLAN`/`Scope: full` como cabeçalhos.
  Command: `rg -q "[Cc]hecklist" apps/cli/templates/skills/pipeline-execution/SKILL.md`

<!-- wikilinks-footer-start -->
- [checklist-progresso-por-onda](?) ⚠ unresolved
- [wave-2-impl](?) ⚠ unresolved
<!-- wikilinks-footer-end -->