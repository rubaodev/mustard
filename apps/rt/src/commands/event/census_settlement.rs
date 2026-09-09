//! `census_settlement` — THE question every door that is about to move, or
//! write into, a dirty tree has to answer: asked once, answered in one place,
//! and ACTED ON here rather than by the door.
//!
//! ## The question, stated whole
//!
//! It has three inputs and exactly one answer.
//!
//! 1. **What is dirty** — harness scratch (discarded), census artefacts (the
//!    tool's own output), the operator's work. Measured by
//!    [`checkout_work`], EXACTLY ONCE per settlement, and carried from there.
//! 2. **Where the checkout stands** ([`CheckoutPosition`]) — the resolved base,
//!    another unit's branch, a protected branch, or a position this decision
//!    cannot attribute at all.
//! 3. **What is about to happen** ([`CensusDoor`]) — an explicit open the
//!    operator typed, a branch cut, or a write-hook pass.
//!
//! One answer, of three shapes ([`CensusSettlement`]):
//!
//! - **Refuse**, naming the paths that are in the way.
//! - **Refresh the base from origin, record the census on it, then proceed.**
//! - **Proceed**, nothing owed.
//!
//! ## Why this is ONE function and not a condition at each door
//!
//! Six review rounds found nine instances of the same defect, and every round
//! answered it the same way: it added a condition at a call site.
//! `DirtyPathKind` and `CheckoutWork::CensusOnly`; then a guard before the cut;
//! then a second guard at the recording; then the positional predicate
//! `census_commit_belongs_here`; then the refresh/record ORDER — in two of the
//! three doors. Each round guarded the writers it could see, and the next round
//! found the writer it had missed. There were FOUR writers of this decision:
//! the explicit `emit-pipeline` door, the census re-mine's own recording (never
//! guarded at all), `spec-draft`'s cut, and the write hook.
//!
//! So the doors stopped deciding AND stopped acting. A door states the three
//! inputs and obeys the answer; it performs no step, so there is no step left
//! for it to get wrong. The base refresh, the re-mine and the census commit all
//! happen HERE, in that order, once — the ordering that used to be a comment
//! repeated in each door is now the shape of a single function body.
//!
//! ## The one difference between doors
//!
//! [`CensusDoor::ExplicitOpen`] is the command the operator typed, and it is
//! the one place in the flow where a clean tree on the base is a premise of the
//! command itself: it records onto a PROTECTED base, and it always did. The two
//! cutting doors do not — a hook must not create a commit on a protected branch
//! behind the operator's back. That difference is a NAMED INPUT here, never an
//! `if` in a door.
//!
//! The explicit door also owns the deterministic RE-MINE, for the reason
//! [`super::base_gate`] gives: a freshly updated base, before the first edit, is
//! the one moment in the flow where a clean tree holds by construction. A cut
//! does not re-mine — it happens moments after that open, and re-mining inside a
//! `PreToolUse` hook would put the grain sidecar in front of every first edit.
//!
//! ## Fail-open throughout
//!
//! A census git cannot see, a git that declines the commit, an unreachable
//! remote: every one of them leaves the write where it fell and answers
//! `Proceed`. Only a POSITIVE observation of somebody's work ever refuses.

use std::path::Path;

use mustard_core::ProjectConfig;

use super::base_gate;
use super::work_branch::{
    checkout_work, holds_other_work, is_protected, refresh_integration_bases, BusyCheckout,
    CheckoutWork,
};

/// WHAT IS ABOUT TO HAPPEN — the third input, and the only thing the doors
/// disagree about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CensusDoor {
    /// `emit-pipeline --kind pipeline.kind` — the operator typed the command
    /// that opens the unit. Nothing is checked out here, so nothing can ride
    /// along and nothing is ever refused; the tree is on the base by the
    /// premise of the command, and a PROTECTED base records like any other.
    ExplicitOpen,
    /// `spec-draft`'s cut of the pending work branch
    /// ([`super::work_branch::cut_pending_work_branch`]).
    BranchCut,
    /// The write hook's first in-repo mutation
    /// ([`crate::hooks::write::work_branch_gate`]).
    WriteHookPass,
}

impl CensusDoor {
    /// May the census commit land on a PROTECTED base at this door?
    ///
    /// The ONE legitimate difference between the doors. `true` only where the
    /// operator typed the command: a hook that committed on a protected branch
    /// would be writing history nobody asked for.
    fn may_record_on_a_protected_base(self) -> bool {
        matches!(self, Self::ExplicitOpen)
    }

    /// Does this door re-mine the deterministic census before recording it?
    ///
    /// Only the explicit open. See the module doc.
    fn remines_the_census(self) -> bool {
        matches!(self, Self::ExplicitOpen)
    }
}

/// WHERE THE CHECKOUT STANDS — the second input, stated by the door and never
/// re-derived here.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CheckoutPosition<'a> {
    /// The branch the tree really sits on — `None` on a detached HEAD or a
    /// probe that did not answer. Used BOTH to judge attribution and to drive
    /// the git steps, which is why it is never masked: see
    /// [`Self::attributable`].
    current: Option<&'a str>,
    /// The branch about to be cut, when one is. `None` at a door that cuts
    /// nothing — the explicit open — where no work can ride anywhere and so
    /// nothing is ever refused.
    target: Option<&'a str>,
    /// The base this open or cut resolved to. `None` when the door could not
    /// establish it: a base nobody knows authorises no commit, and nothing is
    /// going to move either, so the census is not what the operator needs to
    /// read about.
    base: Option<&'a str>,
    /// Whether the position can be ATTRIBUTED at all — see
    /// [`Self::attributable`].
    attributable: bool,
}

impl<'a> CheckoutPosition<'a> {
    /// The ordinary position: where the tree sits, what is about to be cut (if
    /// anything), and the base that was resolved for it.
    pub(crate) fn at(
        current: Option<&'a str>,
        target: Option<&'a str>,
        base: Option<&'a str>,
    ) -> Self {
        Self {
            current,
            target,
            base,
            attributable: true,
        }
    }

    /// Declare whether "whose work is this?" has an answer in this tree.
    ///
    /// `false` for a SUBMODULE reached through the write hook: its HEAD is
    /// judged against the SUPERproject's bases, which misreads its position
    /// outright — so it refuses nothing and records nothing, exactly as the
    /// `if !in_submodule` at that door did before the decision collapsed here.
    ///
    /// The branch name itself is NOT dropped: the base refresh is a git step
    /// that never depended on attribution, and it needs to know whether the
    /// tree is standing on the base it is about to fast-forward.
    pub(crate) fn attributable(mut self, attributable: bool) -> Self {
        self.attributable = attributable;
        self
    }

    /// `true` when the census commit BELONGS on this checkout: the position was
    /// measured, the base is a fact, the position IS that base, and — at every
    /// door but the explicit one — that base is not protected.
    ///
    /// Positional and not a list of exclusions, because the list is what kept
    /// missing a position: two earlier iterations excluded "protected", "HEAD"
    /// and "unmeasured", and both let through the one nobody had named — ANOTHER
    /// unit's branch, which is none of those three. The census describes the
    /// whole project, so its commit belongs to the base and to nothing else;
    /// anywhere else it enters some unit's diff and some unit's pull request as
    /// if that unit had rewritten the repository's map.
    fn is_the_base(&self, root: &Path, config: &ProjectConfig, door: CensusDoor) -> bool {
        if !self.attributable {
            return false;
        }
        let Some(branch) = self.current.filter(|b| *b != "HEAD") else {
            return false;
        };
        let Some(base) = self.base.map(str::trim).filter(|b| !b.is_empty()) else {
            return false;
        };
        branch == base
            && (door.may_record_on_a_protected_base() || !is_protected(root, branch, config))
    }

    /// `true` when taking this checkout would carry work that is not this
    /// unit's onto the branch about to be cut — the plain `git checkout -b`
    /// this settlement stands in front of moves everything uncommitted with it.
    ///
    /// `false` wherever nothing is going to be checked out (no target), and
    /// wherever the position cannot be attributed.
    fn would_carry_work_off(&self, root: &Path, config: &ProjectConfig) -> bool {
        self.attributable
            && self
                .target
                .is_some_and(|target| holds_other_work(root, self.current, target, config))
    }
}

/// THE ANSWER — three shapes and no more. Everything the answer describes has
/// ALREADY HAPPENED when it is returned; the caller only obeys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CensusSettlement {
    /// Do NOT proceed. The tree holds work this move would carry off, and the
    /// refusal carries the measured paths so each door can say the same
    /// sentence in its own shape ([`BusyCheckout::reason`]).
    ///
    /// Nothing was fetched, mined or committed before this answer: a refused
    /// move must leave the tree exactly as it found it.
    Refuse(BusyCheckout),
    /// The base was refreshed from `origin`, the census was re-mined where that
    /// door does so, and the artefacts named here were recorded on the base —
    /// in that order. Proceed.
    Recorded(Vec<String>),
    /// Nothing owed. The base was refreshed when there was a base to refresh;
    /// there was nothing of the tool's to record here, or nowhere for it to go.
    Proceed,
}

/// Settle the census question for one door, one position and one tree.
///
/// The whole decision AND the whole effect. In order, and the order is the
/// point:
///
/// 1. **Measure the tree, once** ([`checkout_work`]). Every later question
///    reads this one value; nothing measures again.
/// 2. **Refuse** when work that is not this unit's would ride along.
///    Refusing FIRST is what keeps a refused move from leaving a fetch, a
///    re-mine or a commit behind it.
/// 3. **Refresh the base from `origin`.** Before the census commit, never
///    after: the advance is a `merge --ff-only`, and a census commit written on
///    the base ahead of it makes the base diverge, so the advance is refused —
///    silently, since the result is best-effort per base — and the unit is cut
///    from a stale base. It is also the invariant the root `CLAUDE.md` states in
///    its own words: `--ff-only` only passes while the integration base carries
///    no commit of its own.
/// 4. **Re-mine**, at the door that does.
/// 5. **Record** what the tool wrote — what was already dirty plus what the
///    re-mine just produced — as ONE commit on the base.
///
/// The caller does none of those steps and cannot reorder them.
pub(crate) fn settle(
    root: &Path,
    position: CheckoutPosition<'_>,
    config: &ProjectConfig,
    door: CensusDoor,
) -> CensusSettlement {
    // An explicit `vcs: ""` opt-out (or a tree git does not manage) has no
    // base to refresh and no commit to write.
    let Some(vcs) = config.vcs() else {
        return CensusSettlement::Proceed;
    };

    // 1. WHAT IS DIRTY — measured here and NOWHERE else in this settlement.
    let work = checkout_work(root);
    let on_the_base = position.is_the_base(root, config, door);

    // 2. REFUSE. `CensusOnly` is nobody's work, but it can only be waved
    //    through where it has somewhere to land: standing off the base it
    //    would ride into the new branch exactly as the operator's work would,
    //    and there it is named and refused like any other dirty path.
    //
    //    A base that is not a fact (`None`) is the third reading, and not the
    //    same question: nothing is going to move at all — the cut answers
    //    `BaseUnknown` and the hook refuses or warns, both before any
    //    `git checkout -b` — so the census cannot travel, and the sentence the
    //    operator needs is about the base, not about the census.
    if position.would_carry_work_off(root, config) {
        let travels = match &work {
            CheckoutWork::ProvenClean => false,
            CheckoutWork::CensusOnly(_) => !(on_the_base || position.base.is_none()),
            // Somebody's work, or a tree that could not be measured. An
            // unmeasured tree is not an empty one: reading it as empty is how
            // another unit's work rides off in silence.
            CheckoutWork::Holds(_) | CheckoutWork::Unproven => true,
        };
        if travels {
            return CensusSettlement::Refuse(BusyCheckout {
                current: position.current.unwrap_or_default().to_string(),
                target: position.target.unwrap_or_default().to_string(),
                work,
            });
        }
    }

    // 3. REFRESH THE BASE — before anything can be committed onto it. A base
    //    nobody established cannot be refreshed, and nothing that follows it
    //    will happen either.
    if let Some(base) = position.base.map(str::trim).filter(|b| !b.is_empty()) {
        refresh_integration_bases(
            &vcs,
            &root.to_string_lossy(),
            config,
            position.current,
            Some(base),
        );
    }

    // 4. RE-MINE, at the door that owns it, reading the tree measured at step 1.
    if door.remines_the_census() {
        let _ = base_gate::mine_census_if_stale(root, &work, on_the_base);
    }

    // 5. RECORD. Only on the base, and only when nothing of the operator's is
    //    in the tree — `ProvenClean` and `CensusOnly` are the two readings that
    //    say the index holds nothing of theirs for a commit to sweep up, which
    //    is exactly the fact `record_written_path` needs to be told.
    if !on_the_base {
        return CensusSettlement::Proceed;
    }
    let mut paths: Vec<String> = match &work {
        CheckoutWork::CensusOnly(dirty) => dirty.clone(),
        CheckoutWork::ProvenClean => Vec::new(),
        // Their work is in the tree: it is theirs to commit, beside ours.
        CheckoutWork::Holds(_) | CheckoutWork::Unproven => return CensusSettlement::Proceed,
    };
    // What the re-mine writes is DERIVED, not measured again: the model and its
    // sidecar are written at a known path, and a pathspec for a file that did
    // not change is a no-op for the recorder.
    for mined in base_gate::mined_census_paths(root) {
        if !paths.contains(&mined) {
            paths.push(mined);
        }
    }
    if paths.is_empty() {
        return CensusSettlement::Proceed;
    }
    if base_gate::commit_census(root, &paths) {
        CensusSettlement::Recorded(paths)
    } else {
        // Ignored by git, invisible to it, or a commit git refused: the write
        // stays where it fell, exactly as the deterministic mine already
        // degrades.
        CensusSettlement::Proceed
    }
}
