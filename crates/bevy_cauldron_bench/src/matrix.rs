//! **One suite across several agent and model pairs, success against cost** (BM-06, #416).
//!
//! `cauldron-bench matrix --pairs omp:ollama-cloud/deepseek-v4.1-flash:1017,omp:llama.cpp/bonsai-2-27b,claude-code:sonnet`
//! runs the same tasks, the same trials and the same editor commit once per pair and writes **one report
//! with a row per pair**: success, `pass^k`, wall time, tool calls, edits behind the editor's back and cost.
//! Pairs on the cost-versus-success frontier are marked, because an agent that scores 5 points higher at
//! 20 times the cost is a different answer from one that scores higher at the same price (Kapoor et al.,
//! `10.48550/arXiv.2407.01502`).
//!
//! # What makes the rows comparable
//!
//! **The only difference between two rows is the agent and the model.** [`Matrix::build`] refuses reports
//! that were run at different editor commits, with different guides, suites, task sets or trial counts,
//! and a run that is unfit ([`crate::report::Report::unfit`]): a row from one of them would be a number
//! about something else, set beside numbers it cannot be compared with.
//!
//! # A label does not dirty the tree it measures
//!
//! **Nothing is published until every pair has run** ([`run`]). A pair's commit is read with `git status`, so a
//! report published into `docs/bench/` between two pairs would make the second pair read `<sha>+dirty` and the
//! matrix refuse a comparison it had itself made unequal; measured in #416's run, which dropped `--label` to get
//! past it. The publication happens once, at the end, and no read inside the matrix sees it.
//!
//! # A model with no price is not free
//!
//! A local model's cost is its listed price, often `0`, with its wall time beside it. **A model whose price
//! is `unknown` is shown as `unknown`**, never as free, and is not placed on the frontier: nothing can be
//! said about whether another pair dominates a cost nobody stated.
//!
//! # Where an agent stumbles
//!
//! Each row lists the tools its agent called most, its refusals by tool and the wishes it made, and the
//! matrix says which tools **every** pair was refused on, and which only **one** pair was. That is what
//! separates a hard spot that belongs to the Dark Factory (every pair stumbles there) from one that belongs
//! to a single agent.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::report::{curve, one, pct, secs, top, usd, with_interval, Aggregate, Header, Report, TOP_TOOLS_SHOWN};
use crate::run::Agent;

/// **The version of the matrix's JSON.**
pub const MATRIX_VERSION: u32 = 1;

/// **Where a matrix's `.md` and `.json` are published**, inside `docs/bench/`.
pub const MATRICES_DIR: &str = "matrices";

/// **One agent and model**, as `--pairs` spells it: `omp:<model>`, `claude-code:<model>`, `oracle`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    /// Which harness.
    pub agent: Agent,
    /// The model the harness runs; empty for a scripted agent.
    pub model: String,
}

impl Pair {
    /// **Read one pair.** A harness that has a model and is given none runs its own default.
    pub fn parse(text: &str) -> Result<Pair, String> {
        let (agent, model) = match text.split_once(':') {
            Some((agent, model)) => (agent, Some(model)),
            None => (text, None),
        };
        let agent = Agent::parse(agent)?;
        match (agent.has_model(), model) {
            (true, Some(model)) if !model.trim().is_empty() => Ok(Pair { agent, model: model.to_owned() }),
            (true, _) => Ok(Pair { agent, model: agent.default_model().to_owned() }),
            (false, None) => Ok(Pair { agent, model: String::new() }),
            (false, Some(_)) => Err(format!(
                "`{text}`: `{}` runs no model, so a pair names it alone",
                agent.name()
            )),
        }
    }

    /// **Every pair of a `--pairs` list**, refusing a pair named twice.
    pub fn parse_all(list: &str) -> Result<Vec<Pair>, String> {
        let mut out: Vec<Pair> = Vec::new();
        for text in list.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let pair = Pair::parse(text)?;
            if out.contains(&pair) {
                return Err(format!("`{text}` is named twice, and a row would be one run counted twice"));
            }
            // **Two pairs may not share a directory**: `omp:a/b` and `omp:a-b` both slug to `omp-a-b`, and the
            // second run would overwrite the first's trials under one name.
            if let Some(other) = out.iter().find(|o| o.slug() == pair.slug()) {
                return Err(format!(
                    "`{text}` and `{}:{}` would both write their trials to `{}`: name the models so they differ \
                     in more than punctuation",
                    other.agent.name(),
                    other.model,
                    pair.slug()
                ));
            }
            out.push(pair);
        }
        if out.len() < 2 {
            return Err("a matrix compares at least two pairs; `run` measures one".to_owned());
        }
        Ok(out)
    }

    /// **A name safe for a directory and a label**: letters, digits and dashes.
    pub fn slug(&self) -> String {
        let raw = if self.model.is_empty() {
            self.agent.name().to_owned()
        } else {
            format!("{}-{}", self.agent.name(), self.model)
        };
        raw.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
    }
}

/// **Whether a pair is on the cost-versus-success frontier.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frontier {
    /// No other pair succeeds at least as often for no more cost, and strictly better on one.
    On,
    /// Another pair does.
    Dominated,
    /// **The pair's cost or success is unknown**, so nothing can be said about it.
    Unplaced,
}

impl Frontier {
    /// The words a row writes.
    pub fn words(self) -> &'static str {
        match self {
            Frontier::On => "on the frontier",
            Frontier::Dominated => "dominated",
            Frontier::Unplaced => "cost unknown, not placed",
        }
    }
}

/// **One pair's row.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// `agent:model`.
    pub pair: String,
    /// How this pair's run was made.
    pub header: Header,
    /// The dev tasks as one.
    pub dev: Aggregate,
    /// The held-out tasks as a total.
    pub held_out: Option<Aggregate>,
    /// **Cost per scored trial**, dollars, or `None` for unknown.
    pub cost_per_trial_usd: Option<f64>,
    /// Where it sits against the others.
    pub frontier: Frontier,
    /// The tools it called most.
    pub tool_counts: BTreeMap<String, u32>,
    /// Its refusals by tool.
    pub refusals_by_tool: BTreeMap<String, u32>,
    /// The wishes it made, by the tool wished for.
    pub wishes_by_tool: BTreeMap<String, u32>,
}

/// **A matrix report.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matrix {
    /// The version of the shape.
    pub version: u32,
    /// What the matrix is called in its file name.
    pub label: String,
    /// The commit every pair ran at.
    pub commit: String,
    /// The guide every pair was handed.
    pub guide_sha256: String,
    /// The suite.
    pub suite: String,
    /// Trials per task.
    pub trials: u32,
    /// Dev tasks in the suite.
    pub tasks: u32,
    /// The machine.
    pub host: String,
    /// The day.
    pub date: String,
    /// One row per pair, in the order `--pairs` named them.
    pub rows: Vec<Row>,
    /// **Tools every pair was refused on.**
    pub refused_by_every_pair: Vec<String>,
    /// **Tools only one pair was refused on**, by that pair.
    pub refused_by_one_pair: BTreeMap<String, Vec<String>>,
}

/// **Place every row against the others**: `On` unless a row with known numbers dominates it.
fn place(rows: &mut [Row]) {
    let known: Vec<Option<(f64, f64)>> = rows
        .iter()
        .map(|r| r.dev.mean_success.zip(r.cost_per_trial_usd))
        .collect();
    for (i, row) in rows.iter_mut().enumerate() {
        row.frontier = match known[i] {
            None => Frontier::Unplaced,
            Some((success, cost)) => {
                let beaten = known.iter().enumerate().any(|(j, other)| {
                    j != i
                        && other.is_some_and(|(s, c)| {
                            s >= success && c <= cost && (s > success || c < cost)
                        })
                });
                if beaten { Frontier::Dominated } else { Frontier::On }
            }
        };
    }
}

impl Matrix {
    /// **Build the matrix of reports**, refusing a set that cannot be compared.
    pub fn build(label: &str, reports: &[Report]) -> Result<Matrix, String> {
        let Some(first) = reports.first() else {
            return Err("a matrix needs at least two runs".to_owned());
        };
        if reports.len() < 2 {
            return Err("a matrix needs at least two runs; one is a report".to_owned());
        }
        let h = &first.header;
        let ids = |r: &Report| r.tasks.iter().map(|t| t.task.clone()).collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        for report in reports {
            let o = &report.header;
            if let Some(why) = &report.unfit {
                return Err(format!("`{}` is unfit for comparison and is not put in a matrix: {why}", o.pair()));
            }
            if !seen.insert(o.pair()) {
                return Err(format!("`{}` is in the matrix twice", o.pair()));
            }
            // **Everything but the agent and model must be the same**, or the row is about something else.
            for (what, same) in [
                ("editor commit", o.commit == h.commit),
                ("shipped guide", o.guide_sha256 == h.guide_sha256),
                ("suite", o.suite == h.suite && ids(report) == ids(first)),
                ("trials per task", o.trials == h.trials),
                ("variant", o.variant == h.variant),
            ] {
                if !same {
                    return Err(format!(
                        "the matrix compares pairs on the same tasks, trials and editor commit, so the only \
                         difference is the agent and model; `{}` and `{}` differ in their {what}",
                        h.pair(),
                        o.pair()
                    ));
                }
            }
        }
        let mut rows: Vec<Row> = reports
            .iter()
            .map(|r| Row {
                pair: r.header.pair(),
                header: r.header.clone(),
                dev: r.dev.clone(),
                held_out: r.held_out.clone(),
                cost_per_trial_usd: r.dev.cost_usd.filter(|_| r.dev.trials > 0).map(|c| c / f64::from(r.dev.trials)),
                frontier: Frontier::Unplaced,
                tool_counts: r.tool_counts.clone(),
                refusals_by_tool: r.refusals_by_tool.clone(),
                wishes_by_tool: r.wishes_by_tool.clone(),
            })
            .collect();
        place(&mut rows);

        // **Which tools are a hard spot for everyone, and which for one agent.**
        let refused: Vec<BTreeSet<&String>> = rows
            .iter()
            .map(|r| r.refusals_by_tool.iter().filter(|(_, n)| **n > 0).map(|(t, _)| t).collect())
            .collect();
        let every: Vec<String> = refused
            .first()
            .map(|first| first.iter().filter(|t| refused.iter().all(|s| s.contains(**t))).map(|t| (*t).clone()).collect())
            .unwrap_or_default();
        let mut one_pair: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (i, set) in refused.iter().enumerate() {
            for tool in set {
                if refused.iter().enumerate().all(|(j, other)| j == i || !other.contains(*tool)) {
                    one_pair.entry(rows[i].pair.clone()).or_default().push((*tool).clone());
                }
            }
        }
        Ok(Matrix {
            version: MATRIX_VERSION,
            label: label.to_owned(),
            commit: h.commit.clone(),
            guide_sha256: h.guide_sha256.clone(),
            suite: h.suite.clone(),
            trials: h.trials,
            tasks: first.dev.tasks,
            host: h.host.clone(),
            date: h.date.clone(),
            rows,
            refused_by_every_pair: every,
            refused_by_one_pair: one_pair,
        })
    }

    /// **The file stem a published matrix has**: `<date>-<label>`.
    pub fn stem(&self) -> String {
        format!("{}-{}", self.date, self.label)
    }

    /// **The JSON a published matrix holds.**
    pub fn json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map(|text| text + "\n")
            .map_err(|e| format!("the matrix could not be written as JSON: {e}"))
    }

    /// **The matrix a person reads**, as Markdown.
    pub fn markdown(&self) -> String {
        let mut out = format!("# Benchmark matrix: {}\n\n", self.label);
        out.push_str(&format!(
            "{} pairs run on **the same {} dev task(s), {} trial(s) each, and the same editor commit**, so the only \
             difference between two rows is the agent and model. A matrix of runs at different commits is refused.\n\n",
            self.rows.len(),
            self.tasks,
            self.trials
        ));
        out.push_str("| | |\n|---|---|\n");
        for (name, value) in [
            ("Editor and MCP server commit", format!("`{}`", self.commit)),
            ("Shipped guide SHA-256", format!("`{}`", self.guide_sha256)),
            ("Suite", self.suite.clone()),
            ("Host", self.host.clone()),
            ("Date", self.date.clone()),
        ] {
            out.push_str(&format!("| {name} | {value} |\n"));
        }
        out.push_str("\n## Success against cost\n\n");
        out.push_str("| pair | agent | mean success (95% interval) | mean pass^k | mean wall s | mean tool calls | behind the editor's back | cost per trial | cost per success | cost of the run | frontier |\n");
        out.push_str("|---|---|---|---|---|---|---|---|---|---|---|\n");
        for r in &self.rows {
            out.push_str(&format!(
                "| `{}` | {} {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                r.pair,
                r.header.agent,
                r.header.agent_version,
                with_interval(r.dev.mean_success, r.dev.interval),
                curve(&r.dev.mean_pass_hat),
                secs(r.dev.wall_mean_s),
                one(r.dev.tool_calls_mean),
                r.dev.edits_behind,
                usd(r.cost_per_trial_usd),
                usd(r.dev.cost_per_success_usd),
                usd(r.dev.cost_usd),
                if r.frontier == Frontier::On { format!("**{}**", r.frontier.words()) } else { r.frontier.words().to_owned() },
            ));
        }
        out.push_str(
            "\nA model whose price is unknown costs `unknown`, never free, and is not placed on the \
             frontier. A local model's cost is its listed price (often 0) and its wall time is in the table.\n",
        );
        if self.rows.iter().any(|r| r.held_out.is_some()) {
            out.push_str("\n## Held-out tasks, as a total\n\n| pair | mean success |\n|---|---|\n");
            for r in &self.rows {
                out.push_str(&format!(
                    "| `{}` | {} |\n",
                    r.pair,
                    r.held_out.as_ref().map_or_else(|| "-".to_owned(), |h| pct(h.mean_success))
                ));
            }
        }
        out.push_str("\n## Where each agent stumbles (dev tasks)\n");
        for r in &self.rows {
            out.push_str(&format!("\n### `{}`\n\n", r.pair));
            out.push_str(&format!("- tools called most: {}\n", top(&r.tool_counts, TOP_TOOLS_SHOWN)));
            out.push_str(&format!("- refusals by tool: {}\n", top(&r.refusals_by_tool, TOP_TOOLS_SHOWN)));
            out.push_str(&format!("- wishes, by the tool wished for: {}\n", top(&r.wishes_by_tool, TOP_TOOLS_SHOWN)));
        }
        out.push_str("\n## Whose hard spot it is\n\n");
        if self.refused_by_every_pair.is_empty() {
            out.push_str("- no tool was refused on by every pair\n");
        } else {
            out.push_str(&format!(
                "- **refused to every pair, so the Dark Factory's**: {}\n",
                self.refused_by_every_pair.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", ")
            ));
        }
        if self.refused_by_one_pair.is_empty() {
            out.push_str("- no tool was refused to one pair alone\n");
        }
        for (pair, tools) in &self.refused_by_one_pair {
            out.push_str(&format!(
                "- refused only to `{pair}`, so likely that agent's: {}\n",
                tools.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", ")
            ));
        }
        out
    }
}

/// **What runs one pair**: everything a pair does that needs a model and an editor, reduced to the report it made
/// and the paths it wrote. [`run`] hands it [`crate::run::run`] and [`crate::run::write_reports`]; a test hands it
/// an editor-free, model-free stand-in over recorded trials, which is how the order of the matrix's own steps is
/// proved without either.
pub type PairRun<'a> = &'a dyn Fn(&crate::run::Options, Option<&std::path::Path>) -> Result<(Report, Vec<std::path::PathBuf>), String>;

/// **One pair's options, taken from the matrix's**: its agent and model, its own output directory under the
/// matrix's, and the matrix's label with the pair's slug.
///
/// **It publishes nothing** (`publish` is `None` even when the matrix was labelled). Why it is this way and not
/// checked at read time: the commit a report names is read with `git status`, so *any* file the harness writes into
/// the checkout between two pairs makes the second pair's commit `<sha>+dirty`, and the matrix then refuses a
/// comparison the harness itself made unequal (measured in #416's run, which dropped `--label` to get past it).
/// Teaching the commit read to ignore the harness's own files would be a second rule about which paths a
/// publication may touch, kept in step with `index`'s three writers forever; publishing once every pair has run
/// needs no such rule and no exemption, and a pair's trials, `run.json` and report still land under its own
/// `--out`, which is outside the checkout.
pub fn pair_options(options: &crate::run::Options, pair: &Pair) -> crate::run::Options {
    let mut one = options.clone();
    one.agent = pair.agent;
    one.model = pair.model.clone();
    one.out = options.out.join(pair.slug());
    one.label = format!("{}-{}", options.label, pair.slug());
    one.publish = None;
    one
}

/// **Run one suite once per pair and build the matrix**, answering it and the paths written.
///
/// Every pair runs on the same tasks, trials, variant and editor binary, **one pair after another** (each
/// pair's own `--jobs` apply inside it), under `<out>/<pair>/`. Each pair's report is written under its own
/// output; **nothing is published until every pair has run**, and then each pair's report and the matrix are
/// published together ([`pair_options`] says why the publication is here and not inside the loop). A pair whose
/// run was unfit, or one that ran at a different commit, makes the matrix refuse, naming it, and every pair's
/// report is still on disk.
pub fn run(
    options: &crate::run::Options,
    held_out: Option<&std::path::Path>,
    pairs: &[Pair],
) -> Result<(Matrix, Vec<std::path::PathBuf>), String> {
    run_with(options, held_out, pairs, &|one, held_out| {
        crate::run::run(one, held_out)?;
        crate::run::write_reports(one)
    })
}

/// **[`run`] with the pair's own work passed in**, which is what lets a test prove the matrix's order of steps
/// with recorded trials and no agent. `pair_run` gets the pair's options ([`pair_options`], so its `publish` is
/// `None`) and the held-out directory, and answers the report built from what it wrote.
pub fn run_with(
    options: &crate::run::Options,
    held_out: Option<&std::path::Path>,
    pairs: &[Pair],
    pair_run: PairRun<'_>,
) -> Result<(Matrix, Vec<std::path::PathBuf>), String> {
    let mut reports = Vec::with_capacity(pairs.len());
    let mut written = Vec::new();
    for pair in pairs {
        let one = pair_options(options, pair);
        let (report, paths) = pair_run(&one, held_out)?;
        reports.push(report);
        written.extend(paths);
    }
    let matrix = Matrix::build(&options.label, &reports)?;
    let md = options.out.join("matrix.md");
    let json = options.out.join("matrix.json");
    bevy_grimoire_core::atomic_file::write(&md, matrix.markdown()).map_err(|e| format!("{}: {e}", md.display()))?;
    bevy_grimoire_core::atomic_file::write(&json, matrix.json()?).map_err(|e| format!("{}: {e}", json.display()))?;
    written.extend([md, json]);
    // **The label publishes here, after the last pair**, so no pair's commit read saw a file this matrix wrote.
    if let Some(bench) = &options.publish {
        for (md, json) in crate::index::publish_reports(bench, &reports)? {
            written.extend([md, json]);
        }
        let (md, json) = crate::index::publish_matrix(bench, &matrix)?;
        written.extend([md, json]);
    }
    Ok((matrix, written))
}
