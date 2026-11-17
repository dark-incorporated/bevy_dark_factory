---
description: Work every open issue in bevy_dark_factory to closure — parallel workers, reviewed merges, one gate push per round, then a release
argument-hint: "[optional focus area, e.g. volume | decide | bench]"
disable-model-invocation: true
---

# Complete every open issue: one wave, from audit to release

Set this as the goal when you want the whole backlog closed rather than one pass's item. It is written to
survive being handed to a fresh session with no memory of the previous wave: everything that cost a cycle
last time is in here.

**The goal, in the owner's words:** "keep running the fleet until all open issues in
`dark-incorporated/bevy_dark_factory` are resolved", and call `goal complete` **only** after auditing every
issue against current evidence — never because a budget ran out or a phase ended.

## 1. Begin with the truth about the backlog

Do this before dispatching anything; two of the last wave's four remaining issues turned out to be
unclosable because nobody checked first.

- `gh issue list --state open --limit 100 --json number,title,labels` — write the list down.
- For each issue, read its **acceptance block** and classify it as one of:
  - **Workable** — needs code, tests and a gate push.
  - **Waiting on another issue** (`Depends on` / `Needs:` still open) — order it after its dependency.
  - **Waiting on an asset or environment this repo does not have** — e.g. a map that lives in the owner's
    project (`liches_lair`), a GPU frame, a real macOS window. Prove it is absent (`find` on each host)
    before saying so, comment the measurement on the issue, and leave it open.
  - **Waiting on the owner** — a person's login, a physical key press, consent, a key. Put these in one list
    and ask for them **once**, in a single `ask` form, at the start. Do not ask one at a time.
- Trackers (titles like `DM-00`, `BM-00`, `DL-00`) close only when every child is closed; tick their boxes
  as children close, and never tick a box on a child's own claim.

## 2. Fan out: one worker per host, one cluster per worker

Hosts and what each can take (measured, 2026-11-17):

|host|RAM|takes|
|---|---|---|
|mac_air|32 GB|everything except editor test binaries (`harness::build_headless` never returns here)|
|big|32 GB shared|the gate, GPU work, editor tests under Xvfb|
|bmb|48 GB|bevy-crate tests; **disk is the limit** — check `df` first, a cold target needs ~25 GB|
|big_mac|32 GB|offline builds only (`cargo nextest --config ~/vendor-main/config.toml --offline`)|

- **One worker per host.** Two workers on one host fight over the shared target's lock and the cores.
- Each worker gets a branch `fleet/<name>`, its own worktree, a cluster of **one to three** related issues,
  and a brief that names: the files, the acceptance bullets verbatim, a failing-test-first rule, the
  protected-path rule (never edit an existing test except to lower a ratchet), and the changelog rule.
- Every worker reports `FLEET-RESULT <name> FINISHED|PARTIAL|BLOCKED <sha>` as the last line of its result
  file, and that line is how you know a host is free.
- **A worker's request budget is ~200-280 calls.** A worker that spends 25 minutes exploring and then runs
  out mid-implementation wastes the whole slot. Tell it: if it has not made an edit in 20 minutes, start
  editing; if it cannot finish in 2 hours, commit what is green and report PARTIAL.

## 3. Review every finished cluster before it merges

Spawn one `reviewer` subagent per cluster, read-only, with the issue bodies and `git diff <base>..<sha>`.
It answers, with MUST-FIX vs NIT on each:

1. Does it meet **every** acceptance bullet — including the ones that name an asset this repo lacks?
2. Panics, `unwrap`, magic numbers, a second path for one feature?
3. Were existing tests weakened (a ratchet raised, an assertion deleted)?
4. Do the tests test behaviour, or echo the implementation?
5. Does it break a published API (would `cargo semver-checks` complain)?
6. For anything visible: does a prose claim now outrun what the code does?

**A MUST-FIX goes back to the same worker** on the same branch. Do not merge with one outstanding. Two
rounds of review-returns is normal; a third means the brief was wrong — rewrite the brief, not the worker.

## 4. Merge, then gate — one push per round

Merges happen only on **big**, in the push worktree whose `HEAD` becomes `main`:

```sh
ssh big bash -s <<'EOF'
cd ~/worktrees/bevy_dark_factory-fleet-integ2
git fetch -q origin '+refs/heads/main:refs/remotes/origin/main' '+refs/heads/fleet/*:refs/remotes/origin/fleet/*'
git merge -q --ff-only origin/main
for b in <the branches this round>; do
  git merge --no-edit origin/fleet/$b >/dev/null 2>&1 || { echo "$b CONFLICT"; git status --short | grep -E '^(UU|AA)'; }
done
git rev-parse HEAD
EOF
```

**Conflict rules:** `CHANGELOG.md` keeps **every** line from both sides (never pick a side); `Cargo.lock`
takes theirs then `cargo metadata`; `all.rs` keeps all `mod` lines; a generated table
(`docs/guide/reload-contract.md`, `docs/mcp-tools.md`) is regenerated, never hand-merged; anything else is
resolved by reading both intents, and if that is not clear, drop that branch from the round and send it back.

Then the gate. **`/tmp/fleet/gate-nocap.sh` is the script to use** (it exists on big; recreate it as below if
it is gone). Everything in it was learned by failure:

```sh
#!/bin/bash
mkdir -p /var/tmp/gtmp
export TMPDIR=/var/tmp/gtmp RUSTC_WRAPPER= CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 NEXTEST_TEST_THREADS=1 \
  CARGO_TARGET_DIR=$HOME/.cargo-target-gate PATH=$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin:$HOME/.cargo/bin
cd $HOME/worktrees/bevy_dark_factory-fleet-push
/usr/bin/flock /tmp/fleet/big-heavy.lock /usr/bin/timeout 5400 /usr/bin/git push origin HEAD:main > /tmp/fleet/gate-rel.log 2>&1
echo "rc=$?" >> /tmp/fleet/gate-rel.log
```

```sh
ssh big bash -s <<'EOF'
rm -f /tmp/fleet/gate-rel.log
systemd-run --user --collect --unit=fleet-gate-N \
  -p ManagedOOMPreference=omit -p MemoryHigh=20G bash /tmp/fleet/gate-nocap.sh
EOF
```

Rules that are not style:

- **Absolute paths.** A systemd user unit gets a minimal `PATH`: `flock`, `timeout`, `git-lfs` and `ssh`
  must be spelled in full or the gate dies in the launcher with `rc=127`/`rc=128`/`rc=69`.
- **No `MemorySwapMax`**, and `ManagedOOMPreference=omit`. A swap cap makes the gate the largest swap user
  in its slice, and `systemd-oomd` then kills it at 3 GB — the unit's own `MemoryPeak` never looks wrong.
- **oomd's system-wide swap rule is the real enemy.** If gates die in ~3 minutes at ~3 GB peak, read
  `journalctl -u systemd-oomd` and look for `Marked .../fleet-gate-N for killing due to memory used`. The
  fix is host memory, not the unit: find the largest RSS/swap holder (`ps -eo rss,args --sort=-rss`,
  `memory.swap.current` per cgroup) and ask the owner before restarting any of their services.
- **Pause big's CI runners** for a long gate (`systemctl --user list-units 'gh-runner@*' --all --no-legend`,
  stop, then start each **by name** — the glob does not expand under `bash -s`), and restore them after.
- **Clips need their LFS objects on the server** before the push, or the gate refuses and no test runs:
  `git lfs push --all origin` in the push worktree. A pointer whose object is missing locally is a clone
  problem — copy the object into `.git/lfs/objects/<xx>/<yy>/` from a clone that has it.
- A gate takes **25-50 minutes** and the tests take longer than the compile. Do not poll it every minute;
  wait in one loop for `^rc=`.
- **Never `--no-verify`**, never force-push, never `reset --hard` in a tree another worker is using.

## 5. Close with evidence, and only with evidence

For each issue in the round:

- `gh issue view <n> --json state` — if a `Fixes #N` trailer closed it but acceptance is unmet, **reopen it**
  with the reason. A wrong `Fixes` is repaired forward, never by rewriting pushed history.
- Comment: the commits, the failing-first message, the green run, each acceptance bullet with the test that
  proves it, and every assumed decision as its own `Assumed decision (owner may override):` comment.
- Add `--add-label assumed-decision` when a choice was made for the owner.
- Remove `worker:fleet` once the issue is closed.

## 6. Release, when a round has landed something a person can see

- `python3 tools/loop/version.py next` — the version comes from the previous one alone; PATCH and MINOR
  stop at 9.
- A bump is its own commit, `release: <version>`, carrying the manifest and `Cargo.lock` and nothing else.
  The gate takes its **fast path** for a bump-only diff (no test run), which is why it is separate.
- Library crates need `cargo semver-checks --baseline-rev <crate>-v<old> -p <crate>` before a publish; an
  additive change needs no bump, and an editor release needs no publish at all.
- Tag and push **only** the editor tag (`bevy_cauldron-v<version>`); the macOS workflow builds, signs,
  notarizes and publishes, in about 7 minutes. Then **write the release notes** — the workflow leaves only
  a compare link.
- A tag push runs the release workflow's smoke test; a `workflow_dispatch` can skip it with `smoke: false`
  when the runner's screen is held by another app.

## 7. Before you say done

- `gh issue list --state open` and justify **every** remaining number in one line: what it waits on, and
  whether that thing exists anywhere in the home cloud. "It needs an asset that is not here" is a complete
  answer only if you `find` for it first.
- Every tracker's boxes match its children's real states.
- The changelog has a line for every shipped change, with a clip or a reason.
- Cleanup: `git worktree remove` every worktree you made (never `--force`), prune targets you created, and
  leave `fleet/*` branches on the remote (deleting them needs the owner's permission).
- Then `goal complete`, with the audit above as the evidence.

## The rules that are never waived

- **Force pushes and deletions need the owner's permission, by name, in the current conversation.**
- **Never edit a protected test to make a change pass.** A ratchet may only be lowered. A test that forbids
  what the code now must do is a decision for the owner, not a deletion.
- **One path per feature.** No fallbacks, no shims, no "skip the check when labelled".
- **A skipped test must say so out loud.** A green tick from a test that measured nothing is the worst
  outcome available — worse than a red one.
- **Report numbers you measured.** If a run produced no data, say there is no data and why; never reuse an
  older run's figures for a new question.
