# Decision fixtures

`request.json` / `response.json`: one recorded `POST /v1/systemone` exchange with clef-flash (Q4_K_M, mmproj Q8_0) served by llama.cpp b11982. One image (`docs/changelog/media/2026-10-24-S1.5.png`, 640x390; the request holds a placeholder instead of the base64 data URL) and three questions, one of each type (`noul`, `choice`, `score`). Every option has a probability. Recorded 2026-10-29 on big. Tests read these files and never call a live model.

`openrouter/`: one recorded `POST https://openrouter.ai/api/alpha/decisions` exchange with
`perplexity/pplx-decider-v1.3:free` (#453), made 2026-11-16 by `tools/decide/record-openrouter.sh`. The same
public changelog image and the same three questions as `request.json`, sent with `Recorder` around `Live`; the
response names `perplexity/pplx-decider-v1.3-20261112`, `Perplexity`, 418 input tokens, cost 0.
`tests/openrouter.rs` replays it with no socket and no key. The key is in neither file.

`calibrations/<model>.yaml`: the clip check's calibration on `clip_samples.yaml`, one file per model (#455), written by `decide_check --calibrate ... --model <model> --out`; the run refuses when the endpoint does not serve `<model>`. `clip_calibration.yaml` is the earlier clef-flash record, made before the samples were paired with the right answer, and is kept as it was. See `docs/decide/models.md`.
