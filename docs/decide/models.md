# The decision models side by side (#455, DM-04)

Measured 2026-11-15 from mac_air against big's RTX 3090; nothing here is from a leaderboard. A calibration
belongs to one model: `fixtures/calibrations/<model>.yaml` is one model's record, a check carries one record
per model (`Check::calibrations`), and a finding names the model that answered and says
`uncalibrated for <model>` when the check has no record for it.

## 1. The fixture request, one table

The recorded request (`crates/bevy_oracle/fixtures/request.json`: one image, `docs/changelog/media/2026-10-24-S1.5.png`,
and a `noul`, a `choice` and a `score`), five warm calls after the first, by
`python3 tools/decide/fixture_row.py --endpoint <url> --model <label>`.

| model | where | has_ui | dominant | clarity | input tokens | warm latency (median of 5) | cost |
|---|---|---|---|---|---|---|---|
| clef-flash Q4_K_M | local, big | 0.929 | map_view 0.978 | 2.15 | 620 | 0.213 s | 0 |
| d3-flash Q4_K_M | local, big | 0.920 | map_view 0.900 | 2.64 | 1074 | 0.779 s | 0 |
| pplx-decider-v1.3:free | OpenRouter | waiting | waiting | waiting | waiting | waiting | waiting |

* **Latency is not a like-for-like column.** clef-flash was reached through an `ssh -L` tunnel to a server
  started by hand under a GPU lease (the committed measurement on big itself, 2026-11-02, is 0.187 s on 619
  tokens); d3-flash was reached over the LAN through llama-swap (`/upstream/d3-flash`). What is comparable is
  the input: d3-flash reads the same image as 1074 tokens against clef-flash's 620, which is most of why it is
  slower. Both answer the easy fixture the same way (has_ui, map_view); they differ on the `score`
  (clarity 2.64 against 2.15).
* **pplx-decider is `waiting`, and the reason is not technical.** It needs `OPENROUTER_API_KEY`, which no host
  has, and the owner's say-so twice over: that screenshots may leave the LAN (the fixture's image is the
  public changelog still, but the clip set's images are the editor's own clips and never go to a remote
  endpoint), and a quota that fits (the free tier is 20 requests a minute and 50 a day below $10 of credit;
  the clip set is 122 requests). Nothing in that row is guessed.

## 2. Agreement on the repo's own labelled clip set

122 samples (61 clips, each under its own line and under another line's), four 480 px images each, the
`clip_shows_its_title` check, by
`decide_check --calibrate crates/bevy_oracle/fixtures/clip_samples.yaml --model <m> --out crates/bevy_oracle/fixtures/calibrations/<m>.yaml`.

| model | agreement | threshold | correct / wrong | record |
|---|---|---|---|---|
| clef-flash (committed, 2026-11-03) | 0.6148 | 0.241 | 75 / 47 | `clip_calibration.yaml`, untouched |
| clef-flash (this code, 2026-11-15) | 0.5328 | 0.530 | 65 / 57 | `calibrations/clef-flash.yaml` |
| d3-flash (this code, 2026-11-15) | **0.8607** | 0.737 | 105 / 17 | `calibrations/d3-flash.yaml` |
| pplx-decider-v1.3:free | waiting | | | needs the key, the consent and the quota above |

**The 0.6148 is not comparable with the other rows, and the reason is a defect found while taking them.**
`score_set` handed `calibrate` the label "its frames match its line" beside P(yes), but the check's yes means
the frames FAIL to show the line, so a model that answered correctly was scored against the opposite label
and could reach at best 50 % (the all-yes threshold). d3-flash under that pairing: **0.500 at threshold
0.000** (61 / 61), although its P(yes) was higher on the mismatch than on its match in 6 of the first 8 sampled
pairs. The pairing is fixed in this change (`tests/the_clip_check_is_scored_against_its_problems.rs`), the
committed record is left as it was, and clef-flash was measured again with the fixed code so that two models
are compared on one method.

**The local default is d3-flash, by measurement.** It agrees 0.861 against clef-flash's 0.533 (and against
the committed 0.6148, which is the number the issue named to beat) on the same set. That is what the local
default endpoint already serves (DM-01), and `tests/the_per_model_calibrations_are_recorded.rs` keeps it so:
the default endpoint must name the model with the best agreement on record. clef-flash stays an explicit,
labelled choice. **0.861 is under the 0.9 bar**, so the clip check stays advisory for d3-flash too.

Caveats that bear on the reading: one run per model, no repeats; the set is the editor's changelog clips and
a mismatch is often about one clause of a long title (see `checking-captures.md`), which is a hard question
for any 9B model; d3-flash Q4_K_M was measured, not Q8_0 (DM-01 found the two answer alike on the fixture).

## 3. Running it again

```sh
python3 tools/decide/clip_set.py --out crates/bevy_oracle/fixtures/clips     # the images, not committed
# d3-flash: nothing to start, the default endpoint serves it (llama-swap loads it on the first request).
cargo run -q -p bevy_oracle --bin decide_check -- --calibrate crates/bevy_oracle/fixtures/clip_samples.yaml \
    --model d3-flash --out crates/bevy_oracle/fixtures/calibrations/d3-flash.yaml
# clef-flash: a lease evicts d3-flash and olmocr; serve it (tools/decide/serve-clef-flash.sh), point
# GRIMOIRE_DECIDE_ENDPOINT at it, label the run `clef-flash`, release the lease after.
```

`--model` is only a label, and the run refuses when the label and the endpoint disagree: before the first
sample it reads `GET /v1/models` from the endpoint's host and stops unless that list names the label, and it
stops on any response whose `model` is another. A record can no longer be made for a model that did not
answer. From a Mac, name the host `big.local` (a bare `big` only resolves on big); the transport asks for
IPv4 only, because `big.local` also resolves to addresses its firewall drops.
