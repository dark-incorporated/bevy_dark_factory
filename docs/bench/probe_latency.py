"""**The decision model's own latency**, measured against d3-flash over llama-swap, 2026-11-21 (DM-06, #457).

**Why a varied `state` on every call.** The first attempt at this (2026-11-20) sent the *same body* 25 times
and reported a 151 ms median for four questions -- 3.5x faster than the repo's own 531 ms for the same count
(`docs/decide/serving-d3-flash.md`). llama-server keeps a prompt cache, so identical requests after the first
are answered from it and the number measures the cache, not the model. Every call here carries a different
`state`, so each is a cold prompt and the timer sees the forward passes.

Prints one line per (questions, image) cell: median and every sample.
"""
import base64, json, random, subprocess, sys

STILL = sys.argv[1]
PORT = sys.argv[2] if len(sys.argv) > 2 else "8081"
png = open(STILL, "rb").read()
data = "data:image/png;base64," + base64.b64encode(png).decode()

QS = [
    ("q1", {"type": "noul", "instructions": "The hallway floor is covered."}),
    ("q2", {"type": "noul", "instructions": "The middle of the hallway is clear."}),
    ("q3", {"type": "noul", "instructions": "The covering is the shape asked for."}),
    ("q4", {"type": "noul", "instructions": "Nothing floats above the floor."}),
    ("q5", {"type": "noul", "instructions": "The covering reaches every wall."}),
    ("q6", {"type": "noul", "instructions": "The colour matches the request."}),
    ("q7", {"type": "noul", "instructions": "No part of the floor is left bare."}),
    ("q8", {"type": "noul", "instructions": "The person would accept this as done."}),
]


def call(nq, with_image, state):
    body = {"model": "d3-flash", "state": state, "questions": dict(QS[:nq])}
    if with_image:
        body["images"] = [data]
    p = subprocess.run(
        ["curl", "-s", "-m", "60", "-o", "/dev/null", "-w", "%{time_total}",
         "-X", "POST", f"http://127.0.0.1:{PORT}/upstream/d3-flash/v1/systemone",
         "-H", "content-type: application/json", "--data-binary", "@-"],
        input=json.dumps(body).encode(), capture_output=True)
    try:
        return float(p.stdout or 0)
    except ValueError:
        return 0.0


print(f"still: {STILL} ({len(png)} bytes)")
print("| questions | image | median ms | every sample (ms) |")
print("|---|---|---|---|")
for nq in (1, 3, 4, 8):
    for img in (True, False):
        t = [call(nq, img, f"attempt {nq}-{img}-{i}-{random.randint(10**6, 10**7)} a hallway floor")
             for i in range(5)]
        s = sorted(t)
        print(f"| {nq} | {'yes' if img else 'no'} | {s[len(s)//2]*1000:.0f} | {[round(x*1000) for x in t]} |")
