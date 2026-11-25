#!/usr/bin/env python3
"""**The exclusivity filter and the rule above it say the same thing** (#577).

`.config/nextest.toml` gives a test `threads-required = "num-cpus"` when it measures the host — the comment there
states the rule, "add a test here when it asserts elapsed time against a budget or waits on the wall clock". It used
to be six *module* patterns, so it took every test in those files: 28 tests owned all four slots for 52.7 s of gate
#10's 581.8 s test wall, and 9 of the 28 read no clock at all. Naming the tests is only half a fix; without a check the
list drifts away from the rule the first time somebody adds a budget test.

**What counts, and why the scope is what it is.** A frame-budget assert reads `UiFrame`/`MAX_MS_PER_FRAME`, and those
live in exactly five test files. `Instant::now`/`.elapsed()` are *not* counted on their own: 97 test files call them,
and nearly all are an `until(...)` timeout around a condition, which is waiting for something to happen rather than
asserting how long the machine took. Counting them would make 224 tests exclusive, which is the defect this issue is
about in the other direction. So the check watches:

1. the six modules the override named (its whole history, so a test added there is still classified), and
2. any test anywhere that reads `UiFrame`/`MAX_MS_PER_FRAME` — a new frame-budget assert is exactly the case the rule
   exists for, and it must not silently measure the host.

It refuses when a watched test that reaches a clock or budget read is not named by the filter, or when a test the filter
names reads none of them (it holds every thread for nothing).

    python3 tools/loop/wall_clock_filter.py            check; exit 1 naming each mismatch
    python3 tools/loop/wall_clock_filter.py --list     print the filter's tests and what each reads

Exit 0 when every clock-reading watched test is named and every named test reads a clock. Stdlib only.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
NEXTEST = REPO / ".config/nextest.toml"
TESTS = REPO / "crates/bevy_dark_factory/tests"

#: A clock or a budget read, reached directly or through a helper in the same module.
CLOCK = re.compile(r"\b(UiFrame|MAX_MS_PER_FRAME)\b|\.elapsed\(\)|\bInstant::now\b")
#: The six modules the override named before #577 narrowed it, kept so a test added to one is still classified.
WATCHED = (
    "the_uis_budget_is_two_milliseconds_a_frame",
    "the_ui_frame_covers_a_change_and_the_input_path",
    "the_frame_a_change_lands_on_stays_inside_two_milliseconds",
    "the_history_scrolls_five_thousand_lines",
    "a_slow_load_shows_a_spinner_then_progress_then_the_editor",
    "the_animation_watch_follows_the_project_index",
    # **A frame budget timed with a bare `Instant::now()`** (#643): the file matches neither `WATCHED` nor `BUDGET`,
    # so it was never inspected and its budget test failed beside 2,000 others while this script said the tree was
    # clean. Naming the module is the script's own mechanism for "inspect every test here".
    "the_assessor_grades_each_turns_questions",
)
#: A frame-budget assert. Read by any test file, watched module or not: this is the rule's own subject.
BUDGET = re.compile(r"\b(UiFrame|MAX_MS_PER_FRAME)\b")
_FN = re.compile(r"^fn ([a-z0-9_]+)\s*\(", re.M)
#: `test(=module::test)` — the exact form the filter uses.
_NAMED = re.compile(r"test\(=([a-z0-9_]+)::([a-z0-9_]+)\)")


def functions(src: str) -> dict[str, tuple[str, bool]]:
    """`fn name -> (body, is_test)`. A function is a test when the text just before `fn` ends with `#[test]`."""
    out: dict[str, tuple[str, bool]] = {}
    for m in _FN.finditer(src):
        brace = src.find("{", m.end())
        if brace < 0:
            continue
        depth, i = 0, brace
        while i < len(src):
            if src[i] == "{":
                depth += 1
            elif src[i] == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        is_test = src[max(0, m.start() - 120) : m.start()].rstrip().endswith("#[test]")
        out[m.group(1)] = (src[brace : i + 1], is_test)
    return out


def clock_tests(path: Path) -> dict[str, bool]:
    """Every test in one module file, mapped to whether it reaches a clock/budget read."""
    src = path.read_text()
    fns = functions(src)
    names = set(fns)
    reads = {n for n, (body, _) in fns.items() if CLOCK.search(body)}
    # A helper that calls a reader is a reader, transitively: `worst_settled_ms` is where the budget test reads it.
    grew = True
    while grew:
        grew = False
        for name, (body, _) in fns.items():
            if name not in reads and any(re.search(r"\b" + re.escape(r) + r"\s*\(", body) for r in reads):
                reads.add(name)
                grew = True
    return {n: n in reads for n, (_, is_test) in fns.items() if is_test}


def named_by_filter() -> set[tuple[str, str]]:
    text = NEXTEST.read_text()
    override = text[text.index("[[profile.default.overrides]]") :]
    return set(_NAMED.findall(override))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--list", action="store_true", help="print the filter's tests and what each reads")
    args = ap.parse_args()

    named = named_by_filter()
    everywhere: dict[tuple[str, str], bool] = {}
    # **What this pass did not look at** (#643): a file matching neither `WATCHED` nor `BUDGET` is *skipped*, not
    # "found clean", and the old message -- "no watched clock-reading test is unnamed" -- was true of the population
    # it inspected and false of the tree. A frame-budget test timed with a bare `Instant::now()` lived in exactly
    # such a file and failed beside 2,000 others while this script said the tree was clean.
    skipped: list[str] = []
    for path in sorted(TESTS.glob("*.rs")):
        if path.name == "all.rs":
            continue
        src = path.read_text()
        if path.stem not in WATCHED and not BUDGET.search(src):
            skipped.append(path.stem)
        # A watched module: every test, clock-read or not, so a narrowed name can be shown to still read one.
        # Any other file: only its tests, and only if the file reads a frame budget at all.
        if path.stem in WATCHED or BUDGET.search(src):
            for test, reads in clock_tests(path).items():
                if path.stem in WATCHED or reads:
                    everywhere[(path.stem, test)] = reads

    if args.list:
        for module, test in sorted(named):
            state = everywhere.get((module, test))
            print(f"{'CLOCK' if state else 'NO CLOCK' if state is False else 'MISSING':8} {module}::{test}")
        return 0

    problems: list[str] = []
    for (module, test), reads in sorted(everywhere.items()):
        if reads and (module, test) not in named:
            problems.append(
                f"{module}::{test} reads a clock or a budget but is not in .config/nextest.toml's exclusivity filter, "
                f"so it measures the host beside 23 other Bevy apps (add `test(={module}::{test})`)"
            )
    for module, test in sorted(named):
        state = everywhere.get((module, test))
        if state is None:
            problems.append(f".config/nextest.toml names {module}::{test}, which is not a test in {module}.rs")
        elif state is False:
            problems.append(
                f".config/nextest.toml names {module}::{test}, which reads no clock or budget, so it holds every "
                f"thread for nothing (remove it)"
            )
    # **Say the size of the population it checked, not only that it found nothing** (#643).
    print(
        f"{len(everywhere)} test(s) inspected across {len({m for m, _ in everywhere})} module(s); "
        f"{len(skipped)} module(s) not inspected (no `WATCHED` entry, no `BUDGET` marker)",
        file=sys.stderr,
    )
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        print(f"\n{len(problems)} mismatch(es) between the rule and the filter", file=sys.stderr)
        return 1
    print(f"{len(named)} tests named, all read a clock or a budget; no watched clock-reading test is unnamed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
