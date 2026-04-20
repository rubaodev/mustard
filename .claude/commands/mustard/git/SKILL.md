# /git - Git Operations

> Commit, push, sync, and merge. Reads `mustard.json` for branch flow. Handles monorepo (submodules) and single repo automatically. Uses **only reversible operations** (stash, cached unlinking, exclude appends) — never destructive filesystem or history rewrites.

## Trigger

`/git <action> [--scope=all|staged|<path-pattern>]`

## Actions

| Action | Description |
|--------|-------------|
| `sync` | Pull parent branch into current branch |
| `commit` | Create commit (no push). Accepts `--scope=all\|staged\|<path-pattern>` |
| `push` | Commit + push to remote |
| `merge` | Sync + fast-forward merge to parent (single hop, always to dev) |
| `merge main` | Fast-forward merge dev → main (explicit promotion, must be on dev) |

## Configuration

Reads `mustard.json` from the **project root**. If not found, falls back to defaults.

```json
{
  "git": {
    "flow": {
      "*": "dev",
      "dev": "main"
    },
    "submodules": true
  }
}
```

### Flow Resolution

Match current branch against `flow` keys. Exact match first, then glob. `*` is the default fallback for any branch not explicitly listed.

| Current branch | Pattern matched | Parent resolved |
|---------------|----------------|-----------------|
| `feature/login` | `*` | `dev` |
| `fix/bug-123` | `*` | `dev` |
| `dev` | `dev` | `main` (only via `/git merge main`) |
| `main` | no match | **error**: terminal branch, no operations allowed |

**Rule**: Exact keys (`dev`, `main`) are matched first. `*` catches everything else. `main` and `dev` are never matched by `*`.

## Behavior

- **ZERO confirmations by default** — analyze, execute, done. The ONLY exception: `commit` without `--scope` asks once per session and memorizes the choice (see **Commit Scope Policy**).
- **Minimize Bash calls** — chain EVERYTHING with `&&` / `;`. One Bash call per repo max whenever possible.
- **No investigation** — if a submodule is dirty, commit it (scoped per **Commit Scope Policy**).
- Submodules BEFORE parent (always).
- **Single repo**: skip all submodule steps — just operate on the root.
- **Local fast-forward merge** — no PRs, no merge commits, 100% linear history.
- **Only reversible operations** — see **Forbidden Operations** below.

---

## Ephemeral Paths (Claude/RTK runtime)

Claude Code and RTK write continuously to these paths **during skill execution**. They are not code, must never be tracked, and must never block checkout.

Canonical list (`$EPHEMERAL_PATHS`):

```
.claude/.agent-state/
.claude/.metrics/
.claude/.pipeline-states/
.claude/.detect-cache.json
.claude/.knowledge-seen.json
```

### Submodule-safe exclude resolution

`.git` is a **file** in submodules (pointer `gitdir: ../../.git/modules/<name>`), so `.git/info/exclude` paths fail there. ALWAYS resolve the real exclude path first:

```bash
EXCLUDE=$(git rev-parse --git-path info/exclude)
```

This works in parent repo, submodules, and worktrees uniformly.

### Silent ensure-excluded step

At the **start of every write-touching action** (`commit`, `push`, `merge`, `merge main`) and in **each repo operated** (parent + every submodule), run:

```bash
EXCLUDE=$(git rev-parse --git-path info/exclude)
for p in ".claude/.agent-state/" ".claude/.metrics/" ".claude/.pipeline-states/" ".claude/.detect-cache.json" ".claude/.knowledge-seen.json"; do
  grep -qxF "$p" "$EXCLUDE" 2>/dev/null || echo "$p" >> "$EXCLUDE"
done
```

This is **idempotent** (grep guard before append). No commit, no worktree change — just ensures the paths are ignored by git going forward in that repo.

### Detection of already-tracked ephemerals

After ensure-excluded, check if any ephemeral is already tracked:

```bash
TRACKED_EPH=$(git ls-files -- .claude/.agent-state/ .claude/.metrics/ .claude/.pipeline-states/ .claude/.detect-cache.json .claude/.knowledge-seen.json 2>/dev/null)
```

If `$TRACKED_EPH` is non-empty → trigger **Ephemeral Tracked Sub-flow** (below) BEFORE the action's main commit.

---

## Auto-stash Protocol

EVERY checkout operation in this skill (sync, merge feature→dev, merge main step 1, merge main step 2, `checkout $ORIGIN` at end) MUST be wrapped by the auto-stash protocol.

### Sentinel format

Each skill invocation generates ONE sentinel per action attempt:

```
mustard-git-autostash-<action>-<unix_timestamp_ns>
```

Examples:
- `mustard-git-autostash-sync-1744934400123456789`
- `mustard-git-autostash-merge-1744934401987654321`
- `mustard-git-autostash-merge-main-step2-1744934402000000000`

Generate once per action entry (`SENTINEL="mustard-git-autostash-<action>-$(date +%s%N)"`) and reuse for push/pop within that action. Different actions get different sentinels so parallel-ish submodule ops do not collide.

### Protected stash push

```bash
git stash push -u -m "$SENTINEL"
# untracked included (-u) because runtime-regenerated files may be untracked
```

### Retry loop on checkout race

Race scenario: between `git stash push` and `git checkout <target>`, Claude/RTK rewrite `.claude/.agent-state/*` etc. → checkout aborts with `"would be overwritten by checkout"` or `"local changes would be overwritten"`.

Protocol (max 3 attempts, then abort with descriptive error):

```bash
ATTEMPT=1
MAX=3
while [ $ATTEMPT -le $MAX ]; do
  git stash push -u -m "$SENTINEL" 2>/dev/null
  CO_OUT=$(git checkout "$TARGET" 2>&1)
  CO_RC=$?
  if [ $CO_RC -eq 0 ]; then
    break
  fi
  if echo "$CO_OUT" | grep -qE "would be overwritten|local changes"; then
    ATTEMPT=$((ATTEMPT+1))
    continue
  fi
  # different failure class — stop and surface
  echo "checkout failed: $CO_OUT" >&2
  exit 1
done
[ $ATTEMPT -gt $MAX ] && { echo "checkout race unresolved after $MAX attempts. Offending paths:"; echo "$CO_OUT"; exit 1; }
```

### Safe stash pop (preserving pre-existing user stashes)

**NEVER** run `git stash pop` without first locating the exact sentinel. Pre-existing user stashes at `stash@{0}` must not be disturbed.

```bash
IDX=$(git stash list | grep -F "$SENTINEL" | head -n1 | sed -E 's/^stash@\{([0-9]+)\}.*$/\1/')
if [ -n "$IDX" ]; then
  git stash pop "stash@{$IDX}"
fi
```

If `$IDX` is empty (sentinel not found — nothing was stashed this run), do nothing. This preserves all pre-existing user stashes intact.

---

## Forbidden Operations

These operations are **irreversible** at filesystem or history level and are **BANNED** from this skill under any condition. Environments with data-safety hooks will block them anyway, causing abrupt aborts.

| Forbidden | Reversible alternative |
|-----------|------------------------|
| `rm -f <path>`, `rm -rf <path>` | `git rm --cached <path>` (preserves file on disk) |
| `git clean -fd`, `git clean -fdx` | Append to `$(git rev-parse --git-path info/exclude)` instead |
| `git checkout -f`, `git checkout --force` | Auto-stash Protocol with retry (above) |
| `git reset --hard` | `git stash push` to snapshot state, then `git checkout <ref>` |
| Forced unlink of lock files | Investigate process holding lock; never delete blindly |

**Rationale**: all skill state transitions must be recoverable via `git reflog` / `git stash list`. Filesystem-destructive shortcuts silently lose user work.

---

## Ephemeral Tracked Sub-flow

Triggered automatically by `commit`/`push` when `$TRACKED_EPH` (see **Ephemeral Paths**) is non-empty, BEFORE the main commit.

Order (per repo that has tracked ephemerals):

1. Ensure-excluded (already ran — confirm):
   ```bash
   EXCLUDE=$(git rev-parse --git-path info/exclude)
   # append missing paths (idempotent guard from Ensure-Excluded step)
   ```
2. Unlink from index without deleting files:
   ```bash
   git rm --cached -r --ignore-unmatch \
     .claude/.agent-state/ .claude/.metrics/ .claude/.pipeline-states/ \
     .claude/.detect-cache.json .claude/.knowledge-seen.json
   ```
3. Dedicated commit:
   ```bash
   git commit -m "chore: ignore ephemeral runtime state

Untracks Claude/RTK runtime paths that should not be versioned.

Co-Authored-By: Claude <noreply@anthropic.com>"
   ```
4. THEN proceed to the user-requested main commit (with resolved `--scope`).

This prevents ephemerals from being dragged into the user-intended commit diff. Without this sub-flow, "commit everything" interleaves runtime noise with real code.

---

## Commit Scope Policy

The `commit` action accepts `--scope`:

| `--scope` value | Behavior |
|-----------------|----------|
| `all` (default when unambiguous) | `git add -A` in every dirty repo |
| `staged` | Commit only what is already staged (`git commit` with no add) |
| `<path-pattern>` | `git add <pattern>` then commit (glob or directory) |

### Decision flow when `--scope` is NOT passed

1. Run `git status --short` in parent + every dirty submodule.
2. Categorize output inline (see **Final Status Report** categorizer below).
3. If output has a **single obvious category** (e.g., only ephemerals → skip; only code changes in one dir → infer that dir): propose the inferred scope in a 5-line preview.
4. Use `AskUserQuestion` **EXACTLY ONCE** per session: _"Scope for this commit? [all / staged / <inferred path>]"_.
5. **Memoize** the answer on `pipeline-state`-style session cache (e.g., env `MUSTARD_GIT_SCOPE_DEFAULT` or a file-local marker) so subsequent `commit`/`push` actions in the same session skip the prompt. Only re-prompt if the user passes `--scope=ask` explicitly.

This resolves the observed ambiguity where "commitar `.claude`" iterated 3× (only `.claude/` → `.claude/` + `CLAUDE.md` → `git add -A`).

---

## Step 0 — Resolve Parent (all actions except commit)

```bash
cat mustard.json 2>/dev/null
git rev-parse --abbrev-ref HEAD
```

Match the current branch against `git.flow` patterns. Store as `$PARENT`.
If no match and no `mustard.json`: `$PARENT` = default branch (detect via `git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null || echo main`).

---

## Step 0b — Branch Protection Check

Before any operation (commit, push, merge, sync) check the current branch:

- If current branch is `main` → **REFUSE** with error: `Cannot operate directly on protected branch 'main'. Create a feature branch first.`
- If current branch is `dev` AND action is `commit`, `push`, or `sync` → **REFUSE** with error: `Cannot operate directly on protected branch 'dev'. Create a feature branch first.`
- If current branch is `dev` AND action is `merge main` → **ALLOW** (this is the only permitted operation on dev).

**Exception**: `/git merge main` is the sole operation allowed on the dev branch — it is the explicit promotion path.

---

## Step 0c — Submodule HEAD state check (monorepo only)

Before any merge or sync that traverses submodules, emit a readable state line per submodule:

```bash
for sm in $(git config --file .gitmodules --get-regexp path | awk '{print $2}'); do
  ( cd "$sm" && echo "$sm: $(git rev-parse --abbrev-ref HEAD) ($(git rev-parse --short HEAD))" )
done
```

If any submodule is in **detached HEAD** (`HEAD` as branch name), report clearly BEFORE attempting any checkout on that submodule. The user must decide (manual fix or proceed via `/git` stash protocol).

---

## sync

Pull the parent branch changes into the current branch.

### Per-repo procedure (parent + each submodule)

1. **Ensure-excluded** (ephemerals) — silent, idempotent.
2. **Auto-stash Protocol**: `SENTINEL="mustard-git-autostash-sync-$(date +%s%N)"`.
3. Fetch + rebase in one chain:
   ```bash
   git fetch origin "$PARENT" && git rebase "origin/$PARENT"
   ```
4. **Safe stash pop** (by sentinel index).
5. If rebase has conflicts → abort rebase, report to user, STOP.

Submodules run in parallel (one Bash call each). Parent repo runs after.

---

## commit

**Branch check**: If on `main` or `dev` → refuse with error (see Step 0b).

### 1. Analyze all changes (single parallel batch)

Run in **one parallel batch**:
- `git status --short`
- `git submodule status` (skip if no `.gitmodules`)
- `git diff --stat`
- `git log --oneline -5`

### 2. Ensure-excluded + detect tracked ephemerals (per repo)

For parent + each dirty submodule:
- Run the **Ensure-Excluded** append loop.
- Compute `$TRACKED_EPH`. If non-empty → run **Ephemeral Tracked Sub-flow** first.

### 3. Resolve scope (see Commit Scope Policy)

Resolve `--scope` → `$SCOPE_EXPR` ∈ {`-A`, staged-only, `<pattern>`}. Prompt once per session if needed.

### 4. Commit dirty submodules (if any — monorepo only)

Launch **ONE parallel Task agent per dirty submodule** (`model: "haiku"`). Each agent runs ONE chained Bash command:

```bash
cd <SUBMODULE_ABSOLUTE_PATH> && git add $SCOPE_EXPR && git diff --cached --stat && git commit -m "<message>"
```

For `staged` scope: skip the `git add` step.

### 5. Commit parent repo

```bash
git add $SCOPE_EXPR && git diff --cached --stat && git commit -m "<message>"
```

### 6. Final Status Report (see below)

### Message Format

```
<type>: <short description>

<detailed description if needed>

Co-Authored-By: Claude <noreply@anthropic.com>
```

Types: feat, fix, refactor, docs, chore, test

---

## push

**Branch check**: If on `main` or `dev` → refuse with error (see Step 0b).

Sequential: **sync first**, then commit + push.

### Phase 1 — Sync

Execute `sync` action. If conflicts → STOP.

### Phase 2 — Commit & Push

Run `commit` flow (including Ensure-Excluded, Ephemeral Tracked Sub-flow, scope resolution). Then push:

#### Submodules (PARALLEL — monorepo only, one Bash call each)

```bash
cd <SUBMODULE_ABSOLUTE_PATH> && git add $SCOPE_EXPR && git commit -m "<message>" && git push origin <branch>
```

#### Parent / Root (ONE Bash call)

```bash
git add $SCOPE_EXPR && git commit -m "<message>" && git push origin <branch>
```

### Phase 3 — Final Status Report

---

## merge

Promote current branch into its parent via **local fast-forward merge** — no PRs, no merge commits, 100% linear history. Single hop only — always merges into `dev` (via `*` wildcard). Never cascades.

**Branch check**: If on `main` → refuse (terminal branch). If on `dev` → refuse (use `/git merge main` instead).

### Step 1 — Sync (mandatory)

Execute `sync` action to rebase from `dev`. If conflicts → STOP. Do not proceed to merge.

### Step 2 — Ensure pushed

Check if local is ahead of remote. If yes, execute `push` first.

### Step 3 — Merge into parent (auto-stashed, retry-capable, compact output)

`$SOURCE` = current branch, `$TARGET` = `$PARENT` (resolved in Step 0, always `dev` for feature/fix branches).

Per-repo procedure (submodules parallel first, then parent):

1. Generate `SENTINEL="mustard-git-autostash-merge-$(date +%s%N)"`.
2. **Ensure-excluded** (ephemerals).
3. Auto-stash Protocol push (`-u`).
4. Checkout chain with retry (Auto-stash Protocol) to `$SOURCE`, pull, then to `$TARGET`, pull:
   ```bash
   git fetch origin && \
     git checkout "$SOURCE" && git pull origin "$SOURCE" && \
     git checkout "$TARGET" && git pull origin "$TARGET"
   ```
5. Fast-forward merge with compact output:
   ```bash
   git merge --ff-only -q "$SOURCE" && git --no-pager diff --stat HEAD@{1} HEAD | tail -3
   ```
6. Push:
   ```bash
   git push origin "$TARGET"
   ```
7. Return to `$SOURCE`:
   ```bash
   git checkout "$SOURCE"
   ```
8. **Safe stash pop** by sentinel index.

Skip submodules with no commits ahead (nothing to merge).

### Fast-forward failure

If `--ff-only` fails (branches diverged), STOP and report to user. This means someone pushed directly to the target branch — resolve manually. **NEVER** fall back to `git reset --hard` or `git checkout -f`.

### Example: `/git merge` from `feature/login`

```
feature/login → dev
  ├── SubprojectA:  ff-merged + pushed
  ├── SubprojectB:  ff-merged + pushed
  └── Parent:       ff-merged + pushed
```

---

## merge main

Full promotion to `main` — cascades through the entire flow chain, then returns to the original branch.

**Branch check**: If on `main` → refuse (terminal branch).

### Behavior

`$ORIGIN` = current branch (saved for return at end).

1. If NOT on `dev`: first execute `merge` action (current branch → dev). If it fails → STOP.
2. Then promote `dev → main`.
3. Return to `$ORIGIN`.

This means from ANY feature/fix branch: `/git merge main` does `feature → dev → main → back to feature` in one shot.

### Step 1 — Merge current branch into dev (if not already on dev)

Execute the full `merge` action (see above — includes auto-stash, retry, compact output, ensure-excluded).

### Step 2 — Merge dev into main (auto-stashed, retry-capable, compact output)

`$SOURCE` = `dev`, `$TARGET` = `main`.

Per-repo procedure (submodules parallel first, then parent):

1. Generate `SENTINEL="mustard-git-autostash-merge-main-$(date +%s%N)"`.
2. **Ensure-excluded** (ephemerals).
3. Auto-stash Protocol push (`-u`).
4. Checkout chain with retry via Auto-stash Protocol:
   ```bash
   git fetch origin && \
     git checkout dev && git pull origin dev && \
     git checkout main && git pull origin main
   ```
5. Compact ff-merge + push:
   ```bash
   git merge --ff-only -q dev && git --no-pager diff --stat HEAD@{1} HEAD | tail -3 && \
     git push origin main
   ```
6. Return to `$ORIGIN` (parent uses `$ORIGIN`; submodules return to `dev`):
   ```bash
   git checkout "$ORIGIN"   # parent
   git checkout dev         # submodule
   ```
7. **Safe stash pop** by sentinel index.

Skip submodules with no commits ahead.

### Fast-forward failure

If `--ff-only` fails at any step, STOP and report. Resolve manually. **NEVER** fall back to destructive operations (see **Forbidden Operations**).

### Example: `/git merge main` from `feature/login`

```
feature/login → dev → main → back to feature/login
  Step 1: feature/login → dev  (ff-merged + pushed)
  Step 2: dev → main           (ff-merged + pushed)
  Return: checkout feature/login
```

### Example: `/git merge main` from `dev`

```
dev → main → back to dev
  Step 1: skipped (already on dev)
  Step 2: dev → main (ff-merged + pushed)
  Return: checkout dev
```

### Output (merge main summary)

Print a summary table at the end AND the **Final Status Report** (below):

```
| Step                    | Status             |
|-------------------------|--------------------|
| feature/login → dev     | ff-merged + pushed |
| dev → main              | ff-merged + pushed |
| Return to feature/login | done               |
```

---

## Final Status Report

**MANDATORY** at the end of every write action (`commit`, `push`, `merge`, `merge main`). Categorizes `git status --short` per repo.

### Per-repo categorizer

For parent + each submodule:

```bash
echo "=== $(basename "$PWD") (branch: $(git rev-parse --abbrev-ref HEAD)) ==="
git status --short | while IFS= read -r line; do
  path=$(echo "$line" | awk '{print $NF}')
  case "$path" in
    .claude/.agent-state/*|.claude/.metrics/*|.claude/.pipeline-states/*|.claude/.detect-cache.json|.claude/.knowledge-seen.json)
      echo "  [ephemeral] $line" ;;
    *)
      if [ "${line:0:2}" = "??" ]; then
        echo "  [untracked] $line"
      else
        echo "  [pending]   $line"
      fi
      ;;
  esac
done
```

### Interpretation legend (printed once at the top)

```
  [ephemeral] — Claude/RTK runtime state; safe to ignore (excluded going forward).
  [pending]   — real code change still in worktree; decide whether to commit.
  [untracked] — new file not yet added; may be real or intentional scratch.
```

If a category is empty, omit its lines. If ALL repos are clean, print a single line: `All repos clean.`

---

## Cautions

- Aborts if ANY repo has merge conflicts (sync or push)
- Aborts if `--ff-only` fails (branches diverged) — **NEVER** fall back to destructive ops
- Submodules BEFORE parent (in sync, push, commit, and merge)
- NEVER use `git add .` — use `git add -A` / `git add <pattern>` from the correct directory
- If any operation fails, stop and report
- After merge, return to the original branch (`$SOURCE` / `$ORIGIN`)
- NEVER commit, push, or sync directly on `main` or `dev`
- `/git merge main` cascades the full chain (branch → dev → main → back to branch)
- NEVER `git stash pop` without locating sentinel index — preserves user's pre-existing stashes
- NEVER touch `.git/info/exclude` directly — always resolve via `git rev-parse --git-path info/exclude` (submodule-safe)

## Performance Budget

- **Max Task agents**: 1 per dirty submodule
- **Max Bash calls per agent**: 1 (all commands chained)
- **Max Bash calls for merge**: 1 per submodule + 1 for parent
- **Max checkout retries**: 3 per repo, then abort with descriptive error
