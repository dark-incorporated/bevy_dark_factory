# The benchmark's runs

Each row is one committed run of `cauldron-bench` (BM-03, #413): a report beside the JSON `cauldron-bench compare` reads. The raw trial directories are not committed. This file is written by `cauldron-bench report --label`, `run --label` and `matrix --label`, newest first, so edit the reports and not this list.

```sh
cauldron-bench report <run directory> --label baseline   # writes docs/bench/runs/<date>-baseline.md
cauldron-bench compare docs/bench/runs/<a>.json docs/bench/runs/<b>.json
```

| run | date | agent and model | variant | commit | dev tasks: mean success (95% interval) | mean pass^k | cost per success | harness errors | held-out total |
|---|---|---|---|---|---|---|---|---|---|
| [2026-11-14-rep-with-style](runs/2026-11-14-rep-with-style.md) | 2026-11-14 | omp:anthropic/claude-sonnet-5-5 | with-style | `dc3a2d26ed` | 100% [100%, 100%] over 3 task(s) | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $0.2575 | 0 | none |
| [2026-11-14-rep-without-style](runs/2026-11-14-rep-without-style.md) | 2026-11-14 | omp:anthropic/claude-sonnet-5-5 | without-style | `dc3a2d26ed` | 0% [0%, 0%] over 3 task(s) | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | unknown | 0 | none |
| [2026-11-14-style-with-style](runs/2026-11-14-style-with-style.md) (unfit) | 2026-11-14 | omp:anthropic/claude-sonnet-5-5 | with-style | `c16441584c` | 100% [100%, 100%] over 3 task(s) | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $0.2530 | 4 | none |
| [2026-11-14-style-without-style](runs/2026-11-14-style-without-style.md) (unfit) | 2026-11-14 | omp:anthropic/claude-sonnet-5-5 | without-style | `c16441584c` | 0% [0%, 0%] over 3 task(s) | 1:0.00 | unknown | 14 | none |
