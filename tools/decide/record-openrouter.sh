#!/bin/sh
# Record the one OpenRouter Decisions exchange into crates/bevy_oracle/fixtures/openrouter/ (issue #453).
#
# Screenshots LEAVE THE LAN on this call. The request is the PUBLIC changelog image
# (docs/changelog/media/2026-10-24-S1.5.png) and three questions, nothing else: see
# crates/bevy_oracle/tests/support/smoke_request.rs. One request, to perplexity/pplx-decider-v1.3:free.
#
#   OPENROUTER_API_KEY=... tools/decide/record-openrouter.sh
#
# The key comes from the environment only; it is never printed or written. After it succeeds, run
#   cargo nextest run -p bevy_oracle --test openrouter --run-ignored only
# and commit fixtures/openrouter/ together with the numbers it printed (docs/decide/openrouter.md).
set -eu
: "${OPENROUTER_API_KEY:?set OPENROUTER_API_KEY in this shell (the owner provides it); it is read from the environment only}"
cd "$(dirname "$0")/../.."

# The free tier allows 20 requests a minute and 50 a day (1000 with >= $10 of credits ever bought).
# Show the account's own count first, and stop when none is left.
curl -sS -f https://openrouter.ai/api/v1/key -H "Authorization: Bearer $OPENROUTER_API_KEY" |
  python3 -c '
import json, sys
d = json.load(sys.stdin)["data"]
q = d.get("free_model_daily_requests") or {}
print("is_free_tier=%s free_model_daily_requests=%s limit_remaining=%s" % (d.get("is_free_tier"), q, d.get("limit_remaining")))
if q.get("remaining") == 0:
    sys.exit("no free-model requests left today: wait, or ask the owner (credits, or the paid pplx-decider-v1.2)")
'

GRIMOIRE_DECIDE_PROTOCOL=openrouter cargo run -q -p bevy_oracle --example record_openrouter -- \
  crates/bevy_oracle/fixtures/openrouter
