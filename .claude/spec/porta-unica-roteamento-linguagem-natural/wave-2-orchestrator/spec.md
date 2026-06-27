---
id: wave.porta-unica-roteamento-linguagem-natural.2-orchestrator
---

# wave-2-orchestrator

## Summary

Porta unica: CLAUDE.md Intent Routing vira roteador (classifica, narra, confirma na duvida, despacha, emite kind); neutraliza descricoes dos 4 fluxos; ajuda /mustard; doc em duas audiencias

## Network

- Parent: [[porta-unica-roteamento-linguagem-natural]]
- Depends on: [[wave-1-backend]]

## Tasks

- [ ] Reescrever CLAUDE.md secao Intent Routing (working + template) como roteador: classifica intencao+escopo, SEMPRE narra a leitura, CONFIRMA so na ambiguidade, despacha o fluxo interno e emite pipeline.kind
- [ ] Neutralizar o frontmatter description de feature/bugfix/task/tactical-fix (x2 trees): de 'Use when the user asks...' para 'internal flow — dispatched by the orchestrator router'
- [ ] Adicionar entrada de ajuda /mustard ('descreva o que quer')
- [ ] Doc em duas audiencias: user-facing (uma porta) vs interna (roteador+fluxos); validar end-to-end que linguagem natural ainda roteia (fallback de auto-trigger fraco se preciso)

## Files

- `.claude/CLAUDE.md`
- `apps/cli/templates/CLAUDE.md`
- `apps/cli/templates/commands/mustard/feature/SKILL.md`
- `apps/cli/templates/commands/mustard/bugfix/SKILL.md`
- `apps/cli/templates/commands/mustard/task/SKILL.md`
- `apps/cli/templates/commands/mustard/tactical-fix/SKILL.md`
