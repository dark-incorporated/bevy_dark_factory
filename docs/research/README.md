# Research

Measurements the project made of itself, each committed with its method before its data, so a result cannot move its own goalposts. Research notes stay out of the README; they live here.

## Does the style help an agent? (SG-10, #409)

The question: on the sample game (`games/sample`), does an agent that can call `blueprint_style` satisfy more of the game's written style lines than the same agent without it? Three tasks, two arms (with and without the tool), the same model (`anthropic/claude-sonnet-5-5`), graded from the files the agent leaves by `dark_factory-bench` with no model in the loop.

| Document | What it is |
|---|---|
| [`2026-11-14-style-helps-an-agent.md`](2026-11-14-style-helps-an-agent.md) | **The registration**, committed before any run: the question, the success rule, the instrument, the 24 style lines, the arms and what they differ by, the analysis and the limits. Its commit is the registration. |
| [`2026-11-14-style-helps-an-agent-analyse.py`](2026-11-14-style-helps-an-agent-analyse.py) | The registered analysis script, with a self-test. It has not been edited since the registration. |
| [`2026-11-14-style-helps-an-agent-results.md`](2026-11-14-style-helps-an-agent-results.md) | **The first run.** 18 of its 48 runs were harness errors (14 of the 24 without the style), because the runner discarded a trial whose agent first called a tool name omp refuses. Kept as it was, in the record. |
| [`2026-11-14-style-helps-an-agent-results-2.md`](2026-11-14-style-helps-an-agent-results-2.md) | **The replication**, on the fixed runner (BM-11, #441): all 30 registered runs scored. The stronger evidence. With the style the agent satisfied 24.0 of 24 lines and without it 10.4, a difference of +13.60 with a 95% bootstrap interval of +13.20 to +14.00. |
| [`2026-11-14-style-helps-an-agent-analyse-2.py`](2026-11-14-style-helps-an-agent-analyse-2.py) | A second, post-hoc analysis script, labelled as such: it reads tool use from each trial's `tool_log` so a tool called by its direct name is counted. It imports the registered statistic and does not re-implement it. |
| [`data/`](data/) | Every run's `trial.json` and `run.json`, the analysis output, the logs and the wrappers, for the first run and the replication. Transcripts are not committed. |

The published benchmark reports for these runs are listed in [`docs/bench/README.md`](../bench/README.md).

**What it does not show.** One game, one model and three tasks. The with-style arm is told by the shipped guide to read the style and cite it, so a ceiling of 24 of 24 is the guide at work as much as a finding, and whether a style helps a person's game is not measured. Each report states its own limits and deviations from the registration.
