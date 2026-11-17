# A capture becomes an issue in this shape

A person presses `Cmd+D` (report to the people who make DarkFactory) or `Cmd+Shift+D` (report about the
open project) and the editor writes one capture: a record, the frame, the picture with the picks
outlined, and the tail of the log. That is evidence, not a bug report. This file is the shape that
turns one into the other — the same seven sections every time, so an agent reading the issue list can
work it and a person reading it later can see what was on screen.

**The one rule that cannot be broken: an issue is filed only after the capture's files are on the
branch the issue links.** An issue whose pictures 404 is worse than no issue: it reads as complete and
has lost the evidence. The order is always copy → commit → push → **verify both raw URLs answer 200** →
create the issue.

## Where the files go, and the two URLs

The capture's four files live in the repository beside their record:

```
docs/loop/captures/<stem>/<stem>.json        the record (note, picks, window, versions)
docs/loop/captures/<stem>/<stem>.png         the frame, as it was
docs/loop/captures/<stem>/<stem>_picks.png   the same frame with the picks outlined
docs/loop/captures/<stem>/<stem>.log         the editor's last 200 log lines
```

`python3 tools/loop/captures.py sync` copies them from the machine they were filed on (over `ssh` by
default, or `--from <dir>` locally) and adds one unread row to `docs/loop/captures/ledger.json`.
Nothing else may copy them: a file with no row, or a row with no file, is what `show` refuses by name.

The two URLs an issue embeds are

```
https://raw.githubusercontent.com/dark-incorporated/bevy_dark_factory/main/docs/loop/captures/<stem>/<stem>_picks.png
https://raw.githubusercontent.com/dark-incorporated/bevy_dark_factory/main/docs/loop/captures/<stem>/<stem>.png
```

and when the capture is synced on a branch instead of `main`, the same path with the branch's name in
place of `main` — verified to answer `200` with `curl -s -o /dev/null -w '%{http_code}'` before the
issue is created. `main` is the better home: a branch deleted after merge leaves the issue's pictures
dead.

## The body, seven sections, in this order

Every filed issue follows it. Omit a section only when the capture genuinely has nothing for it, and
say why in the section's place.

1. **Provenance**, as the first line, read from the record and never typed by hand:

   ```
   **From the owner's capture, `feedback-<stem>`** — DarkFactory <app_version>, <host>, <backend>, <window logical> at `ui_scale` <n>, <screen> <project> / <map>, <date and time>. The note: *"<the note, verbatim>"* — <what was picked, by its node path and `rect_px`>.
   ```

   The backend and the OS are in the capture's `.log` (`AdapterInfo { … backend: … }`,
   `SystemInfo { os: … }`); the record has neither. The clock is `taken_at_ms`.

2. **The picked picture**, embedded, and the frame beneath it for context:

   ```markdown
   ![the thing the note points at](<raw URL of _picks.png>)

   Frame: ![whole window](<raw URL of .png>)
   ```

3. **The note, verbatim**, in a blockquote of its own when it is longer than a line. Never summarised,
   never paraphrased: the owner's words are the evidence, and the wording is often the defect (a note
   that reads `"Cmd "` is a report about being cut off).

4. **Measured facts.** What the pick names — node path, `rect_px`, the text it carries — plus whatever
   the record's own numbers give: the camera pose and cell of a scene pick, the `at` of a point pick,
   and the log lines that explain the frame. A measurement taken from the pixels is stated with the
   row or column it was taken on, so it can be re-taken. A claim that was not measured is marked
   `[INFERENCE]`.

5. **Current behaviour**, naming the file and the symbol responsible where it is known
   (`crates/bevy_dark_factory/src/<file>.rs`, `<fn>`), and what the constant or the role is today. An
   issue that says "the divider is too thick" costs a pass an hour; one that says "`chrome::DIVIDER_W`
   is 24 logical px and gooey's `SPLIT_GRIP_THICKNESS` is 4, measured as a 33 px band at row y=700"
   costs it five minutes.

6. **Wanted**: what the words ask for, stated so a test can tell whether it happened. When the note
   asks a question rather than naming a change (a research ask), this section states the question and
   its options instead, and the issue's acceptance is the write-up.

7. **Acceptance**: three or four checkboxes, each something **this repo can prove with its own test** —
   a relation between two frames or two runs, or an invariant on one run (the oracle table in
   `.omp/commands/debug-response.md` §6). A box that cannot be checked by a test is not acceptance.

## Title, labels and the rest

- **Title**: the defect, then the capture. `Panning with WASD walks the camera out of its own depth
  range, cutting a straight edge across the map (feedback-1794921159460)`. When two captures are one
  defect, both stems are in the title and the issue says which is which.
- **Labels**: `bug` or `enhancement`; `ui` when a person sees it; `owner` when the owner asked for it
  and it is theirs to order; `debug` for a defect found by the owner's own session; `blocked` when
  something else must close first. A research ask is `enhancement owner ui`.
- **One issue per defect.** Before filing, search the open issues for the capture's stem and for the
  symptom (`gh issue list -R dark-incorporated/bevy_dark_factory --search "<words>"`). A second
  capture of the same defect is a comment or an edit to the first — the 2026-11-15 LED-glow complaint
  is four captures across three versions, and one issue is what gets them fixed together.
- **A duplicate that already exists is closed, not deleted**, with one comment naming the issue that
  supersedes it.
- **The ledger follows.** After the issue exists, its row in `docs/loop/captures/ledger.json` carries
  the issue's number in `reason` (or the row is set `triaged` with the issue named), so the next pass
  knows a capture has been filed rather than filing it again.

## The editor's own path

`#495` is the editor doing this itself, at the moment of the capture: the same seven sections built
from the record by `blueprint_model::feedback::issue`, the same copy → commit → push → verify →
create sequence, one settings row naming the checkout that receives captures, and an MCP tool
(`blueprint_feedback_file_issue`) so an agent can file one too. Until it lands, the procedure in
`.omp/commands/file-captures.md` is how a pass does it by hand, and this file is what it follows.
