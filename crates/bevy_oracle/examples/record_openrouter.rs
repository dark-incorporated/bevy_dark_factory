//! **Record the one OpenRouter exchange** into `fixtures/openrouter/` (run it through
//! `tools/decide/record-openrouter.sh`, which checks the quota first).
//!
//! The request is the public changelog image and three questions (`tests/support/smoke_request.rs`).
//! Records the Remote choice whatever the settings say (that is what it is for) and needs `OPENROUTER_API_KEY`
//! in the environment; the key is
//! never printed or written. Prints what `docs/decide/openrouter.md` records: model, provider, cost,
//! tokens and wall time.

#[path = "../tests/support/smoke_request.rs"]
mod smoke_request;

use bevy_oracle::{DecisionModel, Endpoints, Live, Recorder, ask};
use std::time::Instant;

fn main() {
    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("usage: record_openrouter <fixture dir>   (tools/decide/record-openrouter.sh passes it)");
        std::process::exit(2);
    };
    let config = match Endpoints::from_env().config(DecisionModel::Remote) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let transport = Recorder::new(Live::new(config), dir);
    let started = Instant::now();
    let response = match ask(&transport, &smoke_request::request()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let wall = started.elapsed();
    let usage = response.usage.clone().unwrap_or_default();
    println!("model     {}", response.model.as_deref().unwrap_or("(none sent)"));
    println!("provider  {}", usage.provider.as_deref().unwrap_or("(none sent)"));
    println!("id        {}", usage.id.as_deref().unwrap_or("(none sent)"));
    println!("cost      {}", usage.cost.map_or("(none sent)".to_owned(), |c| c.to_string()));
    println!("tokens    {} in, {} out", usage.input_tokens, usage.output_tokens);
    println!("wall time {:.2} s (includes the connection)", wall.as_secs_f64());
    for (question, answer) in &response.answers {
        println!("answer    {question}: {answer:?}");
    }
}
