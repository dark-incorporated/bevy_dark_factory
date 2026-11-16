#!/usr/bin/env python3
"""Measure one decision endpoint on the recorded fixture request (#455), as a `docs/decide/models.md` row.

    python3 tools/decide/fixture_row.py --endpoint http://big:8093/v1/systemone --model d3-flash

Sends `crates/bevy_oracle/fixtures/request.json` with the public changelog image
(`docs/changelog/media/2026-10-24-S1.5.png`, the only image that may go to a remote endpoint) once cold and
`--warm` times warm, and prints the three answers (has_ui, dominant, clarity), the input tokens and the median
warm latency. **Local System One endpoints only**: a remote endpoint speaks another protocol (#453) and is not
measured here. `--model` is a label that must equal the model the endpoint reports and the model each
response names, or the script stops: a row for a model that did not answer is worse than no row.
"""
import argparse
import base64
import json
import os
import statistics
import sys
import time
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
IMAGE = os.path.join(REPO, "docs/changelog/media/2026-10-24-S1.5.png")
REQUEST = os.path.join(REPO, "crates/bevy_oracle/fixtures/request.json")
WARM_CALLS = 5
TIMEOUT_S = 120


def call(url, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(url, data=data, headers={"content-type": "application/json"})
    t = time.monotonic()
    with urllib.request.urlopen(req, timeout=TIMEOUT_S) as r:
        out = json.loads(r.read())
    return out, time.monotonic() - t


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--endpoint", required=True, help="the full /v1/systemone URL")
    ap.add_argument("--model", required=True, help="the label; must be the model the endpoint serves")
    ap.add_argument("--warm", type=int, default=WARM_CALLS)
    args = ap.parse_args(argv)

    models_url = args.endpoint.rsplit("/", 1)[0] + "/models"
    served = [m["id"] for m in call(models_url)[0]["data"]]
    if args.model not in served:
        sys.exit(f"the endpoint serves {served}, not `{args.model}`: refusing to label a row for another model")

    body = json.load(open(REQUEST))
    with open(IMAGE, "rb") as fh:
        body["images"] = ["data:image/png;base64," + base64.b64encode(fh.read()).decode()]
    first, cold_s = call(args.endpoint, body)
    times = []
    for _ in range(args.warm):
        out, s = call(args.endpoint, body)
        times.append(s)
        if out.get("model") not in (None, args.model):
            sys.exit(f"a response names `{out.get('model')}`, not `{args.model}`")
    a = first["answers"]
    row = {
        "model": args.model,
        "has_ui": a["has_ui"]["noul"],
        "dominant": [a["dominant"]["choice"], a["dominant"]["probabilities"][a["dominant"]["choice"]]],
        "clarity": a["clarity"]["score"],
        "input_tokens": first.get("usage", {}).get("input_tokens"),
        "cold_s": round(cold_s, 3),
        "warm_median_s": round(statistics.median(times), 3),
        "warm_s": [round(t, 3) for t in times],
    }
    print(json.dumps(row))
    print(f"| {args.model} | local | {row['has_ui']:.3f} | {row['dominant'][0]} {row['dominant'][1]:.3f} | "
          f"{row['clarity']:.2f} | {row['input_tokens']} | {row['warm_median_s']:.3f} s | 0 |")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
