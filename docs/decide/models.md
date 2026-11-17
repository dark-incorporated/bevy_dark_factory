# The decision models side by side (#455, DM-04)

Measured 2026-11-15 (the two local models) and 2026-11-16 (pplx-decider, over OpenRouter) from mac_air;
nothing here is from a leaderboard. A calibration
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
| perplexity/pplx-decider-v1.3:free | OpenRouter | 0.987 | map_view 0.775 | 2.62 | 418 | 0.66 s (one call) | 0 |

* **Latency is not a like-for-like column.** clef-flash was reached through an `ssh -L` tunnel to a server
  started by hand under a GPU lease (the committed measurement on big itself, 2026-11-02, is 0.187 s on 619
  tokens); d3-flash was reached over the LAN through llama-swap (`/upstream/d3-flash`). What is comparable is
  the input: d3-flash reads the same image as 1074 tokens against clef-flash's 620, which is most of why it is
  slower. Both answer the easy fixture the same way (has_ui, map_view); they differ on the `score`
  (clarity 2.64 against 2.15).
* **pplx-decider's row is one call, not five warm ones**, so its latency is not the median of 5 the local
  columns are: `tools/decide/fixture_row.py` speaks System One only, and the remote path is a different
  protocol, so the number is from the one recorded exchange (`fixtures/openrouter/`, 0.66 s including TLS;
  `docs/decide/openrouter.md`). A second call 3 s later was not made: the free tier is 20 a minute and the
  fixture is the same request the recorder already answered. Its answers are the *same* answers as the
  local models' on this easy fixture (has_ui yes, map_view) and it reads the image as **418** input tokens
  against d3-flash's 1074 — the image reaches Perplexity's adapter as far less text.

## 2. Agreement on the repo's own labelled clip set

122 samples (61 clips, each under its own line and under another line's), four 480 px images each, the
`clip_shows_its_title` check, by
`decide_check --calibrate crates/bevy_oracle/fixtures/clip_samples.yaml --model <m> --out crates/bevy_oracle/fixtures/calibrations/<m>.yaml`.

| model | agreement | threshold | correct / wrong | record |
|---|---|---|---|---|
| clef-flash (committed, 2026-11-03) | 0.6148 | 0.241 | 75 / 47 | `clip_calibration.yaml`, untouched |
| clef-flash (this code, 2026-11-15) | 0.5328 | 0.530 | 65 / 57 | `calibrations/clef-flash.yaml` |
| d3-flash (this code, 2026-11-15) | **0.8607** | 0.737 | 105 / 17 | `calibrations/d3-flash.yaml` |
| perplexity/pplx-decider-v1.3:free | **blocked at sample 46 of 122** | | | see below |

**pplx-decider's agreement is not measured, and the reason is a provider's content filter, not a quota.**
Two runs of `decide_check --calibrate` stopped with the crate's own new named-sample error, and the answer is the
same both times:

```
sample `103-light-buttons` could not be scored: decision endpoint https://openrouter.ai/api/alpha/decisions
refused the request with status 403: {"error":{"message":"Request blocked by content filter:
[SECRET:bitcoin-wif-compressed]","code":403}}
```

The trigger was isolated to one image, not the text: the sample's title alone answers, the still answers,
`frame0.png` answers, and **`frame1.png` of `crates/bevy_oracle/fixtures/clips/103-light-buttons` is refused on
its own**. It is a false positive — a secret scanner reading PNG entropy as a Bitcoin key — and it is stable, so
no retry clears it. The set therefore cannot be run to completion against this provider as it stands, and a
record made from the 45 samples before it would be a `set_size: 122` file whose counts came from 45. **No
`calibrations/pplx-decider-v1.3.yaml` was written**: the row stays unmeasured rather than wrong.

What it needs from the owner is one of: drop or replace that one sample in `clip_samples.yaml` (it is one of 122,
and its match half `103-light-buttons-mismatch` is refused too for the same frame), or accept a record over 121
samples written with its own `set_size`, or leave the remote row unmeasured and keep the local default. The
crate's part is done and tested: `tests/a_refused_sample_is_named.rs` makes the next such stop name the sample
instead of a bare 403, and `tests/a_snapshot_is_the_model_it_names.rs` lets a remote run be recorded at all
(OpenRouter answers with `perplexity/pplx-decider-v1.3-20261112` for the `:free` slug the run is labelled with).

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
# clef-flash: the endpoint is the `decision_model: local` setting's (bevy_oracle::LOCAL_ENDPOINT), not a flag,
# so clef-flash has to be what that route reaches: under a gpu-tenant lease (which evicts d3-flash and olmocr)
# serve tools/decide/serve-clef-flash.sh's server behind it (alias clef-flash), label the run `clef-flash`,
# release the lease after. (The 2026-11-15 clef-flash record was taken through an ssh tunnel to a server started
# by hand, before DM-03 retired the endpoint variable; the run refused every label but the served model's.)
```

`--model` is only a label, and the run refuses when the label and the endpoint disagree: before the first
sample it reads `GET /v1/models` from the endpoint of the `decision_model` setting and stops unless that list names the label (for `remote`, the one model it sends, `perplexity/pplx-decider-v1.3:free`), and it
stops on any response whose `model` is another. A record can no longer be made for a model that did not
answer. From a Mac, name the host `big.local` (a bare `big` only resolves on big); the transport asks for
IPv4 only, because `big.local` also resolves to addresses its firewall drops.
