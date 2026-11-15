# bevy_grimoire_solver

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

A Boolean satisfiability (SAT) back-end for generators that must answer the same way every run, on every machine: no clock, no threads, no entropy, and a give-up budget counted in conflicts rather than seconds. SAT asks whether some true/false assignment to a set of variables satisfies every one of a set of clauses.

![A dark chessboard cycling through board sizes: at 2x2 and 3x3 the squares turn dull red and the caption reads "no arrangement exists", then from 4x4 up to 12x12 amber discs appear, one per row and column with none sharing a diagonal, the grid growing finer at each step](https://raw.githubusercontent.com/dark-incorporated/bevy_dark_factory/main/crates/bevy_grimoire_solver/docs/n_queens.gif)

*The `n_queens` example: no arrangement exists on a 2x2 or 3x3 board, and the solver says so instead of returning the nearest miss.*

**Status:** usable · MSRV 1.97.1

**Why:** a procedural world generator's contract is that a seed reproduces a run, and a solver that is only usually deterministic breaks that contract intermittently, months later, in content rather than in a crash. This one is a function of its input and nothing else.

## Quick start

```sh
cargo add bevy_grimoire_solver
```

```rust
use bevy_grimoire_solver::{Answer, Budget, Solver};

let mut s = Solver::new(3)?;
s.add_clause(&[1, 2])?;      // (x1 ∨ x2)
s.add_clause(&[-1, 3])?;     // (¬x1 ∨ x3)
s.add_clause(&[-2, -3])?;    // (¬x2 ∨ ¬x3)

match s.solve(&[], Budget::conflicts(10_000))? {
    Answer::Satisfied(m) => println!("x1={} x2={} x3={}", m.get(1), m.get(2), m.get(3)),
    Answer::Unsatisfiable { core } => println!("no arrangement exists; blamed {core:?}"),
    Answer::Exhausted => println!("gave up, at the same point it will give up next time"),
}
# Ok::<(), String>(())
```

Literals are plain `i32` numbers in the DIMACS convention (the file format SAT solvers share): `3` is variable 3 and `-3` is its negation.

## Why "deterministic" is in the name

Because it is the property being bought, and it is not the property most solvers optimise for.

This crate exists to decide what a procedural world generator emits. That generator's contract is that a seed reproduces a run: the same seed on a laptop, on a Raspberry Pi, in CI, next year. Three ways a SAT solver quietly stops being a function of its input:

- **It gives up on a wall clock.** A busy machine answers differently from an idle one. `splr` returns `TimeOut` off `Instant::now()`, and its timeout cannot be disabled.
- **It gives up not at all.** Then a pathological instance has no reproducible outcome, because your only recourse is to kill it from outside. `varisat` has no resource limit.
- **It iterates a hash map** whose iteration order is seeded from the environment.

`Budget` is counted in conflicts for the first reason. There is no `Duration` anywhere in this API, and that is a deliberate omission rather than an oversight.

## What it does not do

It decides satisfiability. It does not optimise: there is no MaxSAT loop here, and `Answer` has no notion of a cost.

That is on purpose, because optimisation belongs to whoever knows what the weights mean. What this crate provides is the three things such a loop is built from:

- **assumptions**: literals forced true for one question only;
- an **unsatisfiable core**: the subset of those assumptions that suffices for the refusal;
- **`add_var`**: so the loop can introduce counter variables mid-search, which it could not have known to ask for up front.

Guard each soft constraint with an indicator variable, assume the indicators, and price whatever the core names. That is enough to build core-guided MaxSAT (the OLL and RC2 algorithms) on top without this crate learning what a weight is. The `soft_constraints` example shows the shape.

## Preferences: steering the answer without constraining it

`Solver::with_preferences` gives each variable an optional value to try first. It changes which model comes back, never which models exist: the search backtracks out of a preference like any other decision.

This is the cheap way to get variety. Expressing "I'd like this cell to be that tile" as a soft constraint makes the solver prove it found the arrangement closest to the whole wish-list, which is core-guided search over hundreds of units. The crate's own measurement on a 12x12 tile problem: about 9 seconds as soft constraints, 15 ms as preferences.

Determinism is untouched: the preferences are part of the input, so the answer is still a function of what the solver was given.

## Budgets bound a question, not a lifetime

The conflict count resets on every `solve`, so a budget means "this question may cost this much". Counting from construction would make the tenth question in a loop answerable only if the first nine happened to be cheap, which is not a bound anyone could predict. `Budget::conflicts(0)` is the same as `Budget::UNLIMITED`, and an unlimited solve on a hard instance does not return.

## One caveat about the underlying solver

This wraps [`batsat`](https://crates.io/crates/batsat) (MiniSat's algorithm, in Rust, one transitive dependency). `batsat` 0.6.0 has `conflict_budget` and `propagation_budget` fields and no public way to set them, so its documented budget is unreachable from outside.

What is reachable is `Callbacks::stop`, so this crate counts conflicts itself, through the callback interface, and stops there. The unit is therefore learnt clauses, which tracks conflicts closely but is not promised to equal them. That does not matter for the property that counts, since it is a deterministic function of the search either way.

`SolverOpts`' four randomness settings are pinned explicitly rather than left at their defaults: a default is a decision someone else can change in a patch release. `tests/frozen_model.rs` pins an exact model, measured on aarch64 macOS and checked on x86_64 Linux in CI, so a solver that is consistent on each machine and different between them fails there.

## Try it

From a clone of the [repository](https://github.com/dark-incorporated/bevy_dark_factory):

```sh
cargo run -p bevy_grimoire_solver --example n_queens            # boards for n = 2..=12; pass a number for one board
cargo run -p bevy_grimoire_solver --example graph_colouring     # colour a map, and watch it refuse an impossible one
cargo run -p bevy_grimoire_solver --example soft_constraints    # assumptions and unsat cores: the shape a MaxSAT loop is built from
```

All three print to the terminal and need no GPU.

## Learn more

- API reference: [docs.rs/bevy_grimoire_solver](https://docs.rs/bevy_grimoire_solver).
- The examples: [`examples/`](https://github.com/dark-incorporated/bevy_dark_factory/tree/main/crates/bevy_grimoire_solver/examples).
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_grimoire.html) says how they fit together. `bevy_grimoire_core` delegates its constraint solving to this crate.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/bevy_grimoire_solver/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/bevy_grimoire_solver/LICENSE-APACHE).
