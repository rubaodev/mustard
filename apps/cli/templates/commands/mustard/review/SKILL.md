---
name: mustard-review
description: Use when the user runs /review or asks to review, code-review, or audit a pull request. Auto-detects current branch PR or accepts PR number/URL.
source: manual
---
<!-- mustard:generated -->
# /review - Pull Request Review

> ZERO confirmations, ZERO questions — detect PR, invoke review, done.

`/review [pr-number-or-url]` — reads `mustard.json#git.provider` (`github`/`gitlab`).

## Action

### 1. Resolve + prefetch

Argument: numeric = number, URL = used directly. No argument: `gh pr view --json number,url,title,headRefName`. No PR → *"No open PR found for current branch. Run `/git merge` first."*

```bash
rtk mustard-rt run review-prefetch <pr-ref> --format json
mustard-rt run diff-context --phase execute --subproject {sub}
```

Prefetch returns `title`/`body`/`author`/`base`/`head`/`additions`/`deletions`/`changedFiles`/`files[]`/`comments[]`/`reviews[]` — source of truth, do NOT re-fetch. Fallback: `gh pr view --json title,body,...` + `gh pr diff`.

### 2. Emit + invoke

`mustard-rt run emit-event --event review.start --spec "$MUSTARD_SPEC" --payload "spec=$MUSTARD_SPEC" --payload "target=$PR_TARGET"` → paste diff as `## DIFF` block → `Skill({ skill: "code-review", args: "<pr-ref>" })`. Fallback (skill unavailable): `Task(general-purpose, opus)` with DIFF as source of truth (agent reads source only when ambiguous; records each Read). Checklist: SOLID, Security, Performance, Patterns, Integration.

### 3. Emit complete + report

`mustard-rt run emit-event --event review.complete --spec "$MUSTARD_SPEC" --payload "spec=$MUSTARD_SPEC" --payload "target=$PR_TARGET"` → present results verbatim.

### 4. Tactical-fix discovery (advisory)

Scan return for `## Tactical Fix Candidates` / `## Candidatos a Tactical Fix`. Per entry print *"Tactical fix candidate: <descrição>\nRun: /mustard:tactical-fix <parent-spec> \"<descrição>\""*. Does NOT block APPROVED or trigger fix-loop. REJECTED still routes through normal fix-loop. Qualification: `pipeline-config.md § Tactical Fix Discovery`.

## Model

Initial reviews: per `pipeline-config.md § Models`. **Re-reviews always `model: "sonnet"`**.

## INVIOLABLE RULES

- NEVER confirm before invoking. NEVER try both Skill and Task — Skill first, Task only as fallback.
- ALWAYS pass PR number/URL — never branch names.
- Budget: ≤1 Bash for PR detection, ≤1 Skill/Task call, ≤4 API calls total.
