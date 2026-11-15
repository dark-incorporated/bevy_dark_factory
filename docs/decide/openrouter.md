# OpenRouter Decisions as the remote decision model (DM-02, #453)

`bevy_oracle` can ask OpenRouter's Decisions API instead of a local System One server. **One model per run, chosen by the person, never tried in turn:** a failing endpoint fails loudly and is never swapped for the other. **Screenshots leave the LAN on this path, so it is opt-in**: nothing selects it but the person's own `decision_model: remote` in their own `settings.yaml` (Settings, Decisions, Decision model: Remote; DM-03, #454).

|Where|Meaning|
|---|---|
|`decision_model` in the person's `settings.yaml`|`local` (default: d3-flash, System One on the LAN) or `remote` (this page). Read by the editor and by the MCP server from the same file (`bevy_oracle::Endpoints::resolve`). A project's `editor.yaml` that names the key is a settings problem naming that file, and is ignored: the choice is the person's.|
|`OPENROUTER_API_KEY`|The key. **From the process environment only**: never read from a settings file, a project's `.env` or omp's database; never logged, written or recorded. With Remote chosen and no key, decisions are refused with a sentence naming it; the local model is not asked instead.|
|Endpoint and model|Fixed: `https://openrouter.ai/api/alpha/decisions` and `perplexity/pplx-decider-v1.3:free`. The paid sibling `perplexity/pplx-decider-v1.2` is not selectable.|

```sh
# decide_check asks the model the settings choose; set decision_model: remote in your settings.yaml first.
OPENROUTER_API_KEY=... cargo run -q -p bevy_oracle --bin decide_check -- ...
```

**Migration (2026-11-15):** the old decision environment variables (endpoint, model, key, timeout, protocol) and the project `.env` that held them are retired, with no compatibility read. Every caller builds its transport with `bevy_oracle::Live::new(config)`, which picks `Http` (System One) or `OpenRouterDecisions` by `Config::protocol`, and the config comes from `Endpoints::config(choice)`. `Http` and `OpenRouterDecisions` each refuse a config of the other protocol.

## Why the local endpoint alone cannot reach it

|Difference|Where it is handled|
|---|---|
|The path is `/api/alpha/decisions`.|`Endpoints::config` gives the Remote choice its own endpoint.|
|A top-level `images` field is a 400. Images are `image_url` parts inside `state`.|`bevy_oracle::wire_request` (in `openrouter.rs`). `Request` keeps one shape (text in `state`, images in `images`), so `Replay` and `Recorder` wrap the transport unchanged and `fixtures/request.json` is untouched.|
|An answer can be a `refusal`.|`Answer::Refusal`, turned by `ask` into `Error::Refusal { question }`: never a number.|
|A `provider` block pins who may see the screenshot.|`wire_request`, for `perplexity/*` models only.|
|The free tier is paced (20 a minute).|By the caller (the calibration run). The transport never sleeps and never retries.|

## The request

`POST https://openrouter.ai/api/alpha/decisions` (alpha: the shape may change), `Authorization: Bearer $OPENROUTER_API_KEY`.

```json
{
  "model": "perplexity/pplx-decider-v1.3:free",
  "state": [
    "A screenshot of the bevy_grimoire map editor.",
    {"type": "image_url", "image_url": {"url": "data:image/png;base64,<...>"}}
  ],
  "questions": { "has_ui": {"type": "noul", "instructions": "..."}, "dominant": {"type": "choice", "instructions": "...", "criteria": {"...": "..."}}, "clarity": {"type": "score", "instructions": "...", "criteria": ["..."]} },
  "provider": {"only": ["perplexity"], "allow_fallbacks": false, "data_collection": "deny", "zdr": true}
}
```

* Text in `state` is a plain string, not a `{type: "text"}` part. With no images, `state` stays a plain string (the documented text-only form).
* Images are base64 `data:image/{png,jpeg,webp};base64,...` URLs; OpenRouter does not fetch remote URLs, so anything else is an `Error::Invalid` naming the image's position before a byte is sent. `MAX_IMAGES = 4` still applies. The Perplexity adapter states no image limit of its own; the real ceiling is a row of the table "To fill when a key is available" below.
* Question types: `noul`, `choice`, `score`. `box` is not offered by this model and `bevy_oracle` has no `box` question.
* The `provider` block is fixed: only Perplexity, no fallbacks, no data collection, zero-data-retention routing. A `perplexity/*` request answered by any other provider is an error (the pin did not hold). Other models get no `provider` block: nothing is invented for a provider that is not known.

## The response

```json
{"id": "gen-dec-...", "model": "perplexity/pplx-decider-v1.3-...", "provider": "Perplexity",
 "answers": {"has_ui": {"type": "noul", "noul": 0.93, "confidence": 0.93}, "...": {"type": "refusal"}},
 "usage": {"input_tokens": 612, "output_tokens": 0, "cost": 0}}
```

`id`, `provider` and `usage.cost` are read and kept in `Response::usage` (`Usage { id, provider, cost, input_tokens, output_tokens }`), and `Response::model` is the model that answered: every recorded exchange names who answered and what it cost. Every answer is typed. A `refusal` becomes `Error::Refusal` naming the question; a missing answer, a missing or unexpected `type` (including `box`), or a malformed answer is `Error::BadResponse` naming the question. `check_answers` still enforces the type asked and the option set. OpenRouter's docs say the same: a refusal, a missing answer or an unexpected type is "not answered", never a safe verdict.

## Errors

No hidden retries: whatever OpenRouter says comes back once, named, with its reason (`error.message`, then `error.metadata` when it sends any).

|Status|`bevy_oracle` error|What to do|
|---|---|---|
|400|`Error::Refused { status: 400, body }`|The body is wrong (a top-level `images`, a part that is not an `image_url`, an unused `assets` key). Deterministic: fix it.|
|401|`Error::Refused { status: 401 }`|No or a bad key. Check `OPENROUTER_API_KEY` in this shell.|
|402|`Error::OpenRouter { failure: CreditsExhausted }`|Credits or the key's limit (`metadata.limit_source` says which). Report; do not retry blindly.|
|404|`Error::OpenRouter { failure: NoEndpoint }`|No endpoint satisfies the `provider` filter (`zdr`, `data_collection`, `only`) or the account's privacy settings (https://openrouter.ai/settings/privacy). **Report to the person; the filter is never relaxed.**|
|429|`Error::OpenRouter { failure: RateLimited { retry_after } }`|Free tier: 20 requests a minute, 50 a day (1000 once $10 of credits were ever bought). `Retry-After` is carried as sent. The caller paces (3 s apart is under the minute limit) and waits; nothing here sleeps.|
|502/503/524/529|`Error::Refused`|Provider error, overload or timeout. Report; at most one explicit, logged retry by the person.|
|no key|`Error::RemoteWithoutKey` from `Endpoints::config` (its sentence, `REMOTE_WITHOUT_KEY`, names `OPENROUTER_API_KEY`); a `Config` built by hand without a key is `Error::Invalid` naming it|Nothing was sent, and the local model is not asked instead.|

## Quota check

Before a batch (the calibration run is at least 122 requests with up to four images each), ask the account:

```sh
curl -s https://openrouter.ai/api/v1/key -H "Authorization: Bearer $OPENROUTER_API_KEY" |
  python3 -c 'import json,sys; d=json.load(sys.stdin)["data"]; print(d["is_free_tier"], d["free_model_daily_requests"], d["limit_remaining"])'
```

`free_model_daily_requests` carries `used`, `limit` and `remaining`. A daily limit below the batch is for the owner to decide (credits, or the paid `pplx-decider-v1.2`); a run is never split across days silently. A negative credit balance can break even free models.

## The recorded exchange

`tools/decide/record-openrouter.sh` checks the quota, then makes **one** request (the public image `docs/changelog/media/2026-10-24-S1.5.png`, three questions, `perplexity/pplx-decider-v1.3:free`) through `Recorder` into `crates/bevy_oracle/fixtures/openrouter/`, and prints the model, provider, id, cost, tokens and wall time. No other capture is ever sent: the request is built by `tests/support/smoke_request.rs`, shared by the recorder and the replay test so they cannot drift.

`the_recorded_openrouter_exchange_replays_with_no_network` (`tests/openrouter.rs`) replays it with no network. It is `#[ignore]`d with the command that makes it runnable, and fails loudly with that command if run before the fixture exists. After recording: `cargo nextest run -p bevy_oracle --test openrouter --run-ignored only`.

**Status 2026-11-15: not recorded.** No `OPENROUTER_API_KEY` exists on any host, so no live call was made and no number below was measured. `fixtures/openrouter/` does not exist yet.

### To fill when a key is available

|Field|Value|
|---|---|
|`provider` (expect `Perplexity`)||
|`model` as answered||
|`usage.cost` (expect 0)||
|`usage.input_tokens`, for the smoke request||
|Wall time, first call and warm||
|Answers to `has_ui`, `dominant`, `clarity` against `fixtures/response.json` (clef-flash: 0.929, `map_view` 0.977, 2.14)||
|Does a `score` answer carry `legend` (`Answer::Score.legend` defaults to empty when it does not)||
|Image size or count ceiling (Perplexity states none)||
|`zdr: true` honoured (a 404 here means the filter excluded the free endpoint)||

## Sources

* Decisions guide: https://openrouter.ai/docs/guides/features/decisions
* Request and response schema: https://openrouter.ai/docs/api/api-reference/alphadecisions/submit-a-decisions-request
* Per-provider adapters (Perplexity: `noul`/`choice`/`score`, images as `image_url` parts in top-level `state`): https://openrouter.ai/docs/guides/community/multimodal-decisions
* Limits: https://openrouter.ai/docs/api-reference/limits; provider preferences: https://openrouter.ai/docs/guides/routing/provider-selection
