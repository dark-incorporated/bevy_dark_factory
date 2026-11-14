# Benchmark run: style-probe

| | |
|---|---|
| Editor and MCP server commit | `c16441584c0eef5cc03aff187ad02124bfa71b28` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | omp omp/18.16.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | with-style |
| Trials per task | 1 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

1 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 0 cache read, 11024 cache written, $0.0276. **Cost of the run, trials and pre-flight turn: $0.2228.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 1 | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1952 | $0.1952 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1952 | $0.1952 | 0 |
| style | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1952 | $0.1952 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 41 | 41 | 18.0 | 4 | 0 | 0 | 22/5774 (cache 329964/41752) | $0.1952 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×12, `grimoire_style` ×2, `grimoire_guide` ×1, `grimoire_list_operations` ×1, `grimoire_present` ×1, `grimoire_state` ×1
- refusals by tool: `grimoire_apply_operation` ×4
- wishes, by the tool wished for: none
