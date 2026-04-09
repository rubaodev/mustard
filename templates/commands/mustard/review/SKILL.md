# /review - Pull Request Review

> Review a PR using Claude's native code-review skill. Auto-detects current branch PR or accepts PR number/URL.

## Trigger

`/review [pr-number-or-url]`

## Configuration

Reads `mustard.json` from the **project root** for `git.provider`.

| Provider | CLI | PR detection |
|----------|-----|--------------|
| `github` | `gh` | `gh pr view --json number,url` |
| `gitlab` | `glab` | `glab mr view` |

## Behavior

- **ZERO confirmations** — detect PR, invoke review, done.
- **ZERO questions** — auto-detect if no argument provided.

---

## Step 1 — Resolve PR

### If argument provided

- Numeric → treat as PR number
- URL → use directly

### If no argument

```bash
gh pr view --json number,url,title,headRefName 2>/dev/null
```

If no PR found for current branch → error:
> No open PR found for current branch. Run `/git merge` first to create one.

---

## Step 2 — Invoke Code Review

Use the Skill tool to invoke Claude's native code-review:

```
Skill({
  skill: "code-review",
  args: "<pr-number-or-url>"
})
```

If the native `code-review` skill is not available, fall back to local review:

```
Task({
  subagent_type: "general-purpose",
  model: "opus",
  description: "Review: PR <number>",
  prompt: "Review the changes in the current branch against $PARENT. Checklist: SOLID, Security, Performance, Patterns, Integration."
})
```

---

## Step 3 — Report

Present the review results as returned by the skill/agent.

---

## Provider Support

| Provider | Auto-detect | Manual URL |
|----------|-------------|------------|
| GitHub | `gh pr view` | yes |
| GitLab | `glab mr view` | yes |
| Bitbucket | no | yes |

---

## Model Selection

**Initial reviews**: always use default model (per `pipeline-config.md § Models`).

**Re-reviews**: apply this decision BEFORE dispatching the re-review Task:

1. Count lines in the previous review's return content matching `^\[(CRITICAL|WARNING)\]`. This is `issue_count`.
2. Count files in the pending fix step. This is `files_changed`.
3. Decision table:

   | issue_count | files_changed | model           |
   |-------------|---------------|-----------------|
   | ≤3          | <5            | `haiku`         |
   | else        | else          | default         |

4. Set `model: "..."` on the re-review Task dispatch per the matching row.

## Rules

- NEVER ask for confirmation before invoking the review
- NEVER attempt both Skill and Task — try Skill first, fall back only if unavailable
- ALWAYS use the PR number or URL directly — do NOT pass branch names to the skill
- If provider CLI is missing, instruct the user to install it; do NOT improvise

## Examples

```bash
/review              # Auto-detect PR for current branch
/review 42           # Review PR #42
/review https://github.com/org/repo/pull/42
```

## Performance Budget

- **Max Bash calls**: 1 (PR detection)
- **Max Skill/Task calls**: 1
- **Max API calls total**: ≤ 4
