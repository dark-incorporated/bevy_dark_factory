#!/usr/bin/env python3
"""The workspace-switch and terminal-raise drive (#677, #678, #541), so a release measures what the last one did.

Starts the editor on a project (under its own Xvfb with --xvfb on Linux), then over BRP:

1. 50 settled frames on the Map;
2. 20 Map<->Kit and 20 Map<->World switches, each through `blueprint/run_command workspace.<id>`, the command the chord
   runs. For each, the switch frame is the heaviest frame in the 1.5 s after it: its `busy_ms`, `frame_ms`, schedule
   split and the systems it names (`blueprint/state.frame.systems`);
3. with one shell open, 20 raises of the terminal from the Map (`terminal.raise`), each timed from the command to the
   first poll that finds a `TerminalRow` laid out with words in it, with the frames that took.

Prints one line per step and a summary, and writes the summary as JSON with --out, so two releases compare like with
like: `--compare old.json` prints the two side by side. The numbers are the host's as much as the editor's: the load
average before and after is in the summary, and two runs are comparable only at a similar load.

    python3 tools/demo/switch_drive.py --bin target/debug/bevy_dark_factory --project ~/scratch/proj --map bedroom \
        --xvfb 77 --out /tmp/switch.json
"""
import argparse, json, os, statistics, subprocess, sys, time, urllib.request


def brp(port, method, params=None, timeout=60):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/", data=body, method="POST", headers={"Content-Type": "application/json"})
    r = json.loads(urllib.request.urlopen(req, timeout=timeout).read())
    if "error" in r:
        raise RuntimeError(f"{method}: {r['error']}")
    return r.get("result")


def med(xs):
    return round(statistics.median(xs), 2) if xs else None


def p90(xs):
    xs = sorted(xs)
    return round(xs[int(0.9 * (len(xs) - 1))], 2) if xs else None


def load():
    try:
        return os.getloadavg()[0]
    except OSError:
        return None


def split(f, top=4):
    s = sorted(f.get("schedules") or [], key=lambda s: -s["ms"])[:top]
    return ", ".join(f"{x['schedule']} {x['ms']:.1f}" for x in s)


def systems(f, top=4):
    return ", ".join(f"{s['system']} {s['ms']:.1f}" for s in (f.get("systems") or [])[:top])


def p50_by(fs, key, name):
    per = {}
    for f in fs:
        for s in f.get(key) or []:
            per.setdefault(s[name], []).append(s["ms"])
    return {k: med(v) for k, v in sorted(per.items(), key=lambda kv: -statistics.median(kv[1]))[:6]}


class Editor:
    def __init__(self, args):
        self.args, self.procs = args, []

    def __enter__(self):
        # **A home of its own**, beside the project copy, so the drive's settings (a raised terminal, a window size)
        # never land in the person's own, and the shell it opens is the same on every host.
        home = os.path.join(os.path.dirname(self.args.project), "home")
        env = dict(os.environ, BEVY_BRP_PORT=str(self.args.port), HOME=home, XDG_DATA_HOME=os.path.join(home, "data"),
                   XDG_CONFIG_HOME=os.path.join(home, "config"), SHELL="/bin/bash")
        if self.args.xvfb is not None:
            xvfb = subprocess.Popen(["Xvfb", f":{self.args.xvfb}", "-screen", "0", "1600x1000x24", "-nolisten", "tcp"],
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            self.procs.append(xvfb)
            env["DISPLAY"] = f":{self.args.xvfb}"
            time.sleep(2)
        log = open(os.path.join(self.args.project, "switch_drive.log"), "w")
        editor = subprocess.Popen([self.args.bin, ".", self.args.map, "--workspace", "map"], cwd=self.args.project, env=env,
                                  stdout=log, stderr=subprocess.STDOUT)
        self.procs.insert(0, editor)
        for _ in range(150):
            try:
                if brp(self.args.port, "blueprint/state", timeout=2):
                    time.sleep(8)
                    return self
            except Exception:
                time.sleep(1)
        raise SystemExit("the editor's BRP never answered; see switch_drive.log in the project")

    def __exit__(self, *exc):
        for p in self.procs:
            p.terminate()
            try:
                p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()


def frame(port):
    return (brp(port, "blueprint/state") or {}).get("frame") or {}


def settled(port):
    seen, rows = set(), []
    while len(rows) < 50:
        f = frame(port)
        c = f.get("frame_count")
        if c not in seen and f.get("busy_ms") is not None:
            seen.add(c)
            rows.append(f)
        time.sleep(0.02)
    bm = [f["busy_ms"] for f in rows]
    print(f"SETTLED n=50 busy_ms p50 {med(bm)} p90 {p90(bm)}")
    return {"busy_ms_p50": med(bm), "busy_ms_p90": p90(bm)}


def switches(port, other):
    out = {other: [], "map": []}
    for i in range(20):
        target = other if i % 2 == 0 else "map"
        f0 = frame(port).get("frame_count")
        brp(port, "blueprint/run_command", {"id": f"workspace.{target}"})
        frames, deadline = {}, time.time() + 1.5
        while time.time() < deadline:
            f = frame(port)
            c = f.get("frame_count")
            if c != f0 and c not in frames and f.get("busy_ms") is not None:
                frames[c] = f
            time.sleep(0.003)
        worst = max(frames.values(), key=lambda f: f["busy_ms"])
        out[target].append(worst)
        print(f"switch {i + 1:2d} -> {target:5s}: busy_ms {worst['busy_ms']:.1f} frame_ms {worst['frame_ms']:.1f} | {split(worst)} | {systems(worst)}")
        time.sleep(0.4)
    both = out[other] + out["map"]
    wb = [f["busy_ms"] for f in both]
    summary = {
        "n": len(both), "busy_ms_p50": med(wb), "busy_ms_p90": p90(wb), "busy_ms_max": max(wb),
        "over_100_ms": sum(1 for b in wb if b > 100),
        "schedules_p50": p50_by(both, "schedules", "schedule"), "systems_p50": p50_by(both, "systems", "system"),
    }
    print(f"SWITCH map<->{other} " + json.dumps(summary))
    return summary


ROWS = {"data": {"components": ["bevy_ui::ui_node::ComputedNode"], "option": ["bevy_ui::widget::text::Text"]},
        "filter": {"with": ["bevy_dark_factory::terminal_pane::TerminalRow"]}}


def words_laid_out(port):
    """Whether a terminal row is laid out (a height) and has words in it."""
    for row in brp(port, "world.query", ROWS) or []:
        comps = row.get("components") or {}
        size = (comps.get("bevy_ui::ui_node::ComputedNode") or {}).get("size") or [0, 0]
        text = comps.get("bevy_ui::widget::text::Text")
        # A newtype reads as its string, or as a one-field list or map, depending on the serializer.
        if isinstance(text, list):
            text = text[0] if text else ""
        elif isinstance(text, dict):
            text = next(iter(text.values()), "")
        height = size[1] if isinstance(size, list) else (size or {}).get("y", 0)
        if height > 0 and isinstance(text, str) and text.strip():
            return True
    return False


def placement(port):
    return ((brp(port, "blueprint/state") or {}).get("terminal") or {}).get("placement")


def raises(port):
    """20 raises from the Map with one shell open; a terminal with no shell is reported, not measured."""
    # **One shell open**: raise it once, let the shell print its prompt, then lower it. A project whose settings choose an
    # agent opens no plain shell (`terminal_view::open_the_first_shell`), and its pane has no words to wait for.
    if placement(port) == "collapsed":
        brp(port, "blueprint/run_command", {"id": "terminal.raise"})
    deadline = time.time() + 20
    while not words_laid_out(port):
        if time.time() > deadline:
            state = (brp(port, "blueprint/state") or {}).get("terminal")
            print(f"RAISE not measured: no shell's words in the pane after 20 s; terminal {json.dumps(state)}")
            return {"error": "no shell's words in the pane", "terminal": state}
        time.sleep(0.2)
    ms, frames = [], []
    for i in range(20):
        brp(port, "blueprint/run_command", {"id": "terminal.raise"})
        while placement(port) != "collapsed":
            time.sleep(0.02)
        time.sleep(0.4)
        f0 = frame(port).get("frame_count")
        t0 = time.perf_counter()
        brp(port, "blueprint/run_command", {"id": "terminal.raise"})
        while not words_laid_out(port):
            if time.perf_counter() - t0 > 5:
                print(f"raise {i + 1:2d}: the shell's words were not laid out within 5 s")
                return {"error": f"raise {i + 1} showed no words within 5 s", "ms": ms}
        dt = (time.perf_counter() - t0) * 1000
        f1 = frame(port).get("frame_count")
        ms.append(dt)
        frames.append((f1 or 0) - (f0 or 0))
        print(f"raise {i + 1:2d}: words laid out after {dt:.1f} ms, {frames[-1]} frames")
        time.sleep(0.4)
    summary = {"n": len(ms), "ms_p50": med(ms), "ms_p90": p90(ms), "frames_p50": med(frames), "over_100_ms": sum(1 for m in ms if m > 100)}
    print("RAISE " + json.dumps(summary))
    return summary


FIELDS = (("map_kit", "busy_ms_p50"), ("map_kit", "busy_ms_p90"), ("map_world", "busy_ms_p50"), ("map_world", "busy_ms_p90"),
          ("terminal_raise", "ms_p50"), ("terminal_raise", "ms_p90"))


def compare(old, new):
    print(f"load: was {old.get('load_before')}, now {new.get('load_before')}")
    for key, field in FIELDS:
        print(f"{key} {field}: was {(old.get(key) or {}).get(field)}, now {(new.get(key) or {}).get(field)}")


def report(directory):
    """**What a release prints**: the newest two committed summaries side by side, by name (`<date>-<sha>.json`)."""
    runs = sorted(f for f in os.listdir(directory) if f.endswith(".json")) if os.path.isdir(directory) else []
    if not runs:
        print(f"switch drive: no summary under {directory}")
        return
    newest = json.load(open(os.path.join(directory, runs[-1])))
    print(f"switch drive: {runs[-1]}" + (f" against {runs[-2]}" if len(runs) > 1 else ""))
    compare(json.load(open(os.path.join(directory, runs[-2]))) if len(runs) > 1 else {}, newest)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--bin")
    ap.add_argument("--project", help="a scratch copy: the editor writes its settings and maps there")
    ap.add_argument("--map", default="bedroom")
    ap.add_argument("--port", type=int, default=15877)
    ap.add_argument("--xvfb", type=int, help="start Xvfb on this display (Linux)")
    ap.add_argument("--out", help="write the summary here as JSON (docs/bench/switch/<date>-<sha>.json for a release)")
    ap.add_argument("--compare", help="a summary an earlier run wrote, printed beside this one")
    ap.add_argument("--report", help="run nothing: print the newest two summaries in this directory side by side")
    ap.add_argument("--skip-terminal", action="store_true")
    args = ap.parse_args()
    if args.report:
        return report(args.report)
    if not (args.bin and args.project):
        ap.error("--bin and --project are needed to run the drive")
    args.bin, args.project = os.path.abspath(args.bin), os.path.abspath(args.project)
    summary = {"bin": args.bin, "load_before": load()}
    with Editor(args):
        summary["settled"] = settled(args.port)
        summary["map_kit"] = switches(args.port, "kit")
        summary["map_world"] = switches(args.port, "world")
        if not args.skip_terminal:
            summary["terminal_raise"] = raises(args.port)
    summary["load_after"] = load()
    print(f"load {summary['load_before']} -> {summary['load_after']}")
    if args.out:
        with open(args.out, "w") as f:
            json.dump(summary, f, indent=2)
    if args.compare:
        compare(json.load(open(args.compare)), summary)


if __name__ == "__main__":
    sys.exit(main())
