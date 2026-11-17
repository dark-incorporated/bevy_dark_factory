# The benchmark's runs

Each row is one committed run of `cauldron-bench` (BM-03, #413): a report beside the JSON `cauldron-bench compare` reads. The raw trial directories are not committed. This file is written by `cauldron-bench report --label`, `run --label` and `matrix --label`, newest first, so edit the reports and not this list.

```sh
cauldron-bench report <run directory> --label baseline   # writes docs/bench/runs/<date>-baseline.md
cauldron-bench compare docs/bench/runs/<a>.json docs/bench/runs/<b>.json
```

| run | date | agent and model | variant | commit | dev tasks: mean success (95% interval) | mean pass^k | cost per success | harness errors | held-out total |
|---|---|---|---|---|---|---|---|---|---|
| [2026-11-17-omp-real](runs/2026-11-17-omp-real.md) | 2026-11-17 | omp:anthropic/claude-sonnet-5-5 | none | `6b5162eab2` | 67% [0%, 100%] over 3 task(s) | 1:0.67 | $0.2170 | 0 | none |
| [2026-11-16-ask-without-guide](runs/2026-11-16-ask-without-guide.md) | 2026-11-16 | omp:anthropic/claude-sonnet-5-5 | none | `c21f8ff35a` | 95% [85%, 100%] over 20 task(s) | 1:0.95 2:0.92 3:0.90 | $0.1762 | 0 | none |
| [2026-11-15-ask-with-guide](runs/2026-11-15-ask-with-guide.md) | 2026-11-15 | omp:anthropic/claude-sonnet-5-5 | none | `c21f8ff35a` | 92% [78%, 100%] over 20 task(s) | 1:0.92 2:0.88 3:0.85 | $0.1867 | 0 | none |
| [2026-11-15-baseline](runs/2026-11-15-baseline.md) | 2026-11-15 | omp:anthropic/claude-sonnet-5-5 | none | `5c270a72c8` | 89% [77%, 98%] over 20 task(s) | 1:0.89 2:0.83 3:0.80 4:0.77 5:0.75 | $0.1967 | 0 | 100% |
| [2026-11-14-rep-with-style](runs/2026-11-14-rep-with-style.md) | 2026-11-14 | omp:anthropic/claude-sonnet-5-5 | with-style | `dc3a2d26ed` | 100% [100%, 100%] over 3 task(s) | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $0.2575 | 0 | none |
| [2026-11-14-rep-without-style](runs/2026-11-14-rep-without-style.md) | 2026-11-14 | omp:anthropic/claude-sonnet-5-5 | without-style | `dc3a2d26ed` | 0% [0%, 0%] over 3 task(s) | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | unknown | 0 | none |
| [2026-11-14-style-with-style](runs/2026-11-14-style-with-style.md) (unfit) | 2026-11-14 | omp:anthropic/claude-sonnet-5-5 | with-style | `c16441584c` | 100% [100%, 100%] over 3 task(s) | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $0.2530 | 4 | none |
| [2026-11-14-style-without-style](runs/2026-11-14-style-without-style.md) (unfit) | 2026-11-14 | omp:anthropic/claude-sonnet-5-5 | without-style | `c16441584c` | 0% [0%, 0%] over 3 task(s) | 1:0.00 | unknown | 14 | none |

## Matrices

One suite across several agent and model pairs, on the same tasks, trials and editor commit (BM-06, #416).

| matrix | date | pairs | commit | on the frontier |
|---|---|---|---|---|
| [2026-11-17-bench-omp](matrices/2026-11-17-bench-omp.md) | 2026-11-17 | 2 | `6b5162eab2` | omp:anthropic/claude-sonnet-5-5, omp:anthropic/claude-haiku-4-5 |
