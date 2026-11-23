#!/usr/bin/env python3
"""**The exclusivity filter names the tests that measure, and only those** (#577).

`wall_clock_filter.py` reads the real `.config/nextest.toml` and the real test sources. These tests prove the rule
against fixtures, so a change to the checker itself is visible without waiting for a real budget test to be added or
removed:

1. a clock-reading test named by the filter passes;
2. a clock-reading test the filter does not name is refused, naming it;
3. a named test that reads no clock is refused, naming it;
4. a name in the filter that is not a test in its module is refused;
5. a helper reached from a test is a clock read for that test, so a test that calls it is not missed.

    python3 -m unittest tools/loop/test_wall_clock_filter.py
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "loop" / "wall_clock_filter.py"

BUDGET_TEST = """
#[test]
fn a_settled_frame_stays_inside_the_budget() {
    let frame = *app.world().resource::<UiFrame>();
    assert!(frame.last_ms < 2.0);
}
"""

PLAIN_TEST = """
#[test]
fn a_piece_is_placed() {
    assert_eq!(map.placements.len(), 1);
}
"""

HELPER = """
fn worst_ms(app: &mut App) -> f64 {
    app.world().resource::<UiFrame>().last_ms
}
"""

CALLS_HELPER = """
#[test]
fn a_change_costs_one_frame() {
    assert!(worst_ms(&mut app) < 2.0);
}
"""


class WallClockFilter(unittest.TestCase):
    """Each case builds a repository fixture and runs the real checker against it."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="wall-clock-"))
        self.addCleanup(__import__("shutil").rmtree, self.tmp, True)
        (self.tmp / "tools/loop").mkdir(parents=True)
        (self.tmp / ".config").mkdir()
        (self.tmp / "crates/bevy_dark_factory/tests").mkdir(parents=True)

    def write(self, relative, text):
        path = self.tmp / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def run_check(self):
        # The checker resolves REPO from its own file, so a copy beside the fixture is what makes it read the fixture.
        local = self.tmp / "tools/loop/wall_clock_filter.py"
        local.write_text(SCRIPT.read_text())
        return subprocess.run([sys.executable, str(local)], capture_output=True, text=True, timeout=60)

    def test_a_named_clock_reader_passes(self):
        self.write("crates/bevy_dark_factory/tests/a_frame_module.rs", BUDGET_TEST)
        self.write(".config/nextest.toml",
                   "[[profile.default.overrides]]\nfilter = 'test(=a_frame_module::a_settled_frame_stays_inside_the_budget)'\n"
                   'threads-required = "num-cpus"\n')
        r = self.run_check()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_an_unnamed_clock_reader_is_refused_and_named(self):
        self.write("crates/bevy_dark_factory/tests/a_frame_module.rs", BUDGET_TEST)
        self.write(".config/nextest.toml", "[[profile.default.overrides]]\nfilter = 'test(=nothing)'\n")
        r = self.run_check()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("a_frame_module::a_settled_frame_stays_inside_the_budget", r.stderr)

    def test_a_named_test_with_no_clock_is_refused_and_named(self):
        src = "[[profile.default.overrides]]\nfilter = 'test(=the_history_scrolls_five_thousand_lines::a_click_on_a_line_steps_back_to_it)'\n"
        self.write(".config/nextest.toml", src)
        self.write("crates/bevy_dark_factory/tests/the_history_scrolls_five_thousand_lines.rs", PLAIN_TEST.replace(
            "a_piece_is_placed", "a_click_on_a_line_steps_back_to_it"))
        r = self.run_check()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("reads no clock or budget", r.stderr)

    def test_a_name_that_is_not_a_test_is_refused(self):
        self.write(".config/nextest.toml",
                   "[[profile.default.overrides]]\nfilter = 'test(=a_missing_module::no_such_test)'\n")
        r = self.run_check()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("is not a test in a_missing_module.rs", r.stderr)

    def test_a_helper_a_test_calls_counts_as_a_clock_read(self):
        """A budget test reads the number through a helper; the helper's read belongs to the test that calls it."""
        self.write("crates/bevy_dark_factory/tests/a_frame_module.rs", HELPER + CALLS_HELPER)
        self.write(".config/nextest.toml", "[[profile.default.overrides]]\nfilter = 'test(=nothing)'\n")
        r = self.run_check()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("a_frame_module::a_change_costs_one_frame", r.stderr)


if __name__ == "__main__":
    unittest.main()
