---
description: File the filed-but-unfiled debug captures as GitHub issues on bevy_dark_factory, in the shape docs/loop/captures/ISSUE-FORMAT.md states, with their pictures on a branch first. Works the queue oldest first and stops when it is empty or when it has filed one batch. Run headless or by hand; one fresh context per run is safe.
argument-hint: "[optional capture stem, or 'all' to file every unread capture]"
disable-model-invocation: true
---

# File the captures as issues

A `Cmd+D` capture is evidence on one machine. An issue is what the fleet can read. This command
turns the first into the second — **in the one shape `docs/loop/captures/ISSUE-FORMAT.md` states** —
and it is deliberately separate from `.omp/commands/debug-response.md`, which *answers* a capture.
Filing and answering are different jobs: filing is cheap and can be done in a batch by a person or by
a loop; answering needs a build and a reproduction.

**Read `docs/loop/captures/ISSUE-FORMAT.md` first, in full.** It is the contract: the seven sections,
the two URLs, the order, and the rule that an issue exists only after its pictures resolve. This file
is the procedure around it.

## 0. The floor

1. `python3 tools/loop/captures.py check` exits 0. A ledger that will not validate is not a queue.
2. The clone you are in is on `main`, clean, and up to date (`git status --short` empty,
   `git fetch origin main && git merge-base --is-ancestor origin/main HEAD`).
3. `gh auth status` is fine and `gh repo view dark-incorporated/bevy_dark_factory` answers.

If any of these fails, say which and stop. Do not file an issue from a clone you cannot push from: the
pictures would 404.

## 1. Which captures to file

- `python3 tools/loop/captures.py sync` — copies any capture filed since the last run (idempotent),
  and adds its ledger row unread.
- Then take the ones with **no issue yet**: a row whose `reason` does not name an issue number, and
  which is `unread` or `triaged`. Oldest `taken_at_ms` first. With an argument naming a stem, that one
  only. With `all`, every such row.

A row already filed (its `reason` names `#<n>`, or a `gh issue list --search "<stem>"` finds one) is
skipped, not filed twice — and if two issues exist for one stem, the newer is closed as a duplicate
with one comment naming the other.

## 2. Read one capture

```sh
python3 tools/loop/captures.py show <stem>
```

Then, in this order, and writing down what each gives:

1. **The note**, verbatim, from the record. If it is cut off (four characters, a trailing `"Cmd "`),
   that truncation is itself a finding and becomes an issue of its own if one does not exist
   (`#491` is the example).
2. **The pick's node path and `rect_px`.** A scene pick gives a map cell and the camera's pose; an
   element pick gives the path from the root and the text it carries.
3. **The frame**, cropped to the pick, at native resolution, then zoomed. **Copy the crop to a fresh
   path before reading it** — the image reader caches by path.
4. **The log's warnings and errors** (`grep -E " (WARN|ERROR) " <stem>.log`), and the two lines that
   date the app (`SystemInfo`, `AdapterInfo`).
5. **The code that owns it**, once the note names a symptom: the file, the function, and the constant
   or role the frame's geometry points at. This is what makes the issue cheap to answer.
6. **A measurement from the pixels** where the note is about size, alignment, thickness or overflow:
   a row or column scan with the exact span, so a later pass can re-take it. Say which row or column.

## 3. Group, then file

Two captures that describe one defect are **one issue** (`#482` is `feedback-1794783743883` and
`feedback-1794783776166`; `#478` is four stems across three versions). File the group once, name both
stems in the title, and say in the body which capture shows what.

Then, per issue:

1. **Copy and commit the capture's four files** into `docs/loop/captures/<stem>/` if `sync` has not
   already, and commit them together with those of every other capture in the batch:

   ```sh
   git add docs/loop/captures/<stem> docs/loop/captures/ledger.json
   git commit -m "captures: sync <n> debug capture(s) from <host>"
   git push origin HEAD:main
   ```

   The push gate's fast path treats `docs/` and media as "no test run", so this costs seconds. If
   `main` moved, merge it and push again — never force.

2. **Verify both URLs** before creating anything:

   ```sh
   for f in <stem>_picks.png <stem>.png; do
     curl -s -o /dev/null -w "%{http_code} .../$f\n" \
       "https://raw.githubusercontent.com/dark-incorporated/bevy_dark_factory/main/docs/loop/captures/<stem>/$f"
   done
   ```

   Two `200`s or nothing is filed. A `404` means the commit did not land on the branch the URL names.

3. **Build the body** from the template in `ISSUE-FORMAT.md`. Title, labels and the seven sections.
   The body goes in a file and is passed with `--body-file`; `--body "$(cat <<'EOF' …)"` in a shell
   loses the heredoc's newlines on some shells and mangles the quote characters the note carries.

4. **Create it**, then **write the number back into the ledger** so the next run skips it:

   ```sh
   gh issue create -R dark-incorporated/bevy_dark_factory \
     --label <bug|enhancement> --label ui --label debug \
     --title "<defect> (feedback-<stem>)" --body-file /tmp/issue-<stem>.md
   python3 tools/loop/captures.py set <stem> --status triaged --kind <bug|enhancement> \
     --reason "filed as #<n>"
   git commit -m "captures: <stem> filed as #<n>" -- docs/loop/captures/ledger.json
   ```

5. **Say what was filed** — one line per issue: number, title, and which stems it covers.

## 4. Stop

File the batch, push the ledger, and stop. Filing one capture well is the whole job; filing twenty
badly is worse than filing five well. A stem whose note is unreadable gets a `blocked` row saying so
rather than a guessed issue.

**Never invent a measurement**, and never state a cause the frame does not show: an issue that names
the wrong file costs the pass that answers it more than the issue saved. Mark an inference
`[INFERENCE]` and say what would settle it.

## What not to do

- **Do not file an issue for a capture that is already fixed** — check the ledger's status and the
  changelog for the stem first.
- **Do not file an issue for an upstream defect in this repo.** A cause in `bevy_gooey` or another
  family crate is filed there (`consumer`, plus `consumer:bevy_grimoire` on `bevy_gooey` or `consumer:bevy_dark_factory`
  on any other family crate) and the editor's row is closed
  `blocked` with the issue named, which is `debug-response.md` §7's rule and applies here too.
- **Do not edit a capture's files.** They are the evidence; a crop of one belongs in the issue, not in
  the capture's folder.
