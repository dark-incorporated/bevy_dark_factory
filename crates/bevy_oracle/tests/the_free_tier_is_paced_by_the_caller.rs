//! **The free tier's pace lives in the caller, not in `send`** (#453, DM-02).
//!
//! OpenRouter allows `perplexity/pplx-decider-v1.3:free` [`FREE_REQUESTS_PER_MINUTE`] requests a minute. The
//! transport never sleeps and never retries — a transport that waited would hide the wait inside a request
//! nobody can see — so the thing that makes 122 requests back to back paces itself: [`Paced`] waits out one
//! slot before each request it forwards. No test here calls a model: the inner transport is a stand-in that
//! records the instant it was asked.

use bevy_oracle::{
    Answer, Config, DecisionModel, Error, FREE_REQUESTS_PER_MINUTE, Live, Paced, Protocol, Question, Request,
    Response, Transport, Usage, ask,
};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// The slot a free-tier pace works out to: a minute split over `FREE_REQUESTS_PER_MINUTE`.
const SLOT: Duration = Duration::from_secs(60 / FREE_REQUESTS_PER_MINUTE as u64);

/// A transport that answers at once and remembers the instant it was asked.
struct Clocked {
    /// Each call's offset from [`Clocked::start`].
    seen: RefCell<Vec<Duration>>,
    /// The test's clock started here.
    start: Instant,
    /// Every request the stand-in was handed, in order.
    asked: RefCell<Vec<Request>>,
}

impl Clocked {
    fn new() -> Self {
        Clocked { seen: RefCell::new(Vec::new()), start: Instant::now(), asked: RefCell::new(Vec::new()) }
    }
}

impl Transport for Clocked {
    fn endpoint(&self) -> String {
        "clocked".into()
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        self.seen.borrow_mut().push(self.start.elapsed());
        self.asked.borrow_mut().push(request.clone());
        let answers = request.questions.keys().map(|id| (id.clone(), Answer::Noul { noul: 0.5 })).collect();
        Ok(Response { model: Some("perplexity/pplx-decider-v1.3-free".into()), answers, usage: Some(Usage::default()) })
    }
}

fn asked(number: usize) -> Request {
    Request::new("s").with_question(format!("q{number}"), Question::noul("?"))
}

/// **A pace holds the spacing it promises, and the first request waits for nothing**: a run that has asked
/// nothing yet has nothing to catch up on.
#[test]
fn requests_are_spaced_a_slot_apart_and_the_first_is_not_delayed() {
    let inner = Clocked::new();
    let paced = Paced::every(Duration::from_millis(50), &inner);

    for n in 0..4 {
        ask(&paced, &asked(n)).expect("the stand-in answers");
    }
    let seen = inner.seen.borrow().clone();
    assert_eq!(seen.len(), 4, "every request reached the inner transport");
    assert!(seen[0] < Duration::from_millis(30), "the first request waited {:?} before asking", seen[0]);
    for (i, pair) in seen.windows(2).enumerate() {
        let gap = pair[1] - pair[0];
        assert!(gap >= Duration::from_millis(50), "request {} followed one {gap:?} behind, the slot is 50 ms", i + 1);
    }
}

/// **The pace names its own period**, so a caller reads the number off the type instead of copying 20:
/// a minute split by the requests a minute the tier allows is a 3 s slot.
#[test]
fn the_period_of_the_free_tier_pace_is_a_minute_split_by_the_requests_it_allows() {
    assert_eq!(Paced::period_for_per_minute(FREE_REQUESTS_PER_MINUTE), SLOT);
    assert_eq!(Paced::period_for_per_minute(6), Duration::from_secs(10));
}

/// **The wrapper is not a second path to a model**: it forwards the endpoint, says where the answers are made,
/// and every answer is the inner transport's own error or answer.
#[test]
fn pacing_forwards_the_transport_it_is_given_and_changes_nothing_else() {
    let remote = Config {
        protocol: Protocol::OpenRouter,
        endpoint: "http://127.0.0.1:1/x".into(),
        model: None,
        key: None,
        timeout_secs: 5,
    };
    let live = Live::new(remote.clone());
    let paced = Paced::every(SLOT, &live);
    assert_eq!(paced.endpoint(), remote.endpoint);
    assert_eq!(paced.site(), DecisionModel::Remote, "the wrapper keeps where the answers are made");
    match paced.send(&asked(0)) {
        // The bare transport's own refusal, not one the wrapper wrote.
        Err(Error::Invalid(m)) => assert!(m.contains("OPENROUTER_API_KEY"), "{m}"),
        other => panic!("a paced transport must not answer a call it cannot make: {other:?}"),
    }
}

/// A stand-in that **says** where its answers are made, so forwarding can be told from the trait's default.
struct Local(Clocked);

impl Transport for Local {
    fn endpoint(&self) -> String {
        self.0.endpoint()
    }
    fn site(&self) -> DecisionModel {
        DecisionModel::Local
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        self.0.send(request)
    }
}

/// **A paced transport forwards `site()`** (#455, review F). `Clocked` never overrides `site`, so asserting on it
/// would have tested the trait's default, which reads `Remote`, and would have passed whatever `Paced` did. This
/// one says `Local` and the wrapper must say it too: a wrapper that forgot would make a LAN-only run report that
/// pictures left the machine.
#[test]
fn a_pace_forwards_where_its_inner_transport_says_the_answers_are_made() {
    let inner = Local(Clocked::new());
    let paced = Paced::every(Duration::ZERO, &inner);
    assert_eq!(paced.site(), DecisionModel::Local, "the wrapper must not read as Remote when its inner is Local");
    ask(&paced, &asked(0)).expect("the stand-in answers");
    assert_eq!(inner.0.asked.borrow().len(), 1, "the wrapper asked the inner transport exactly once");
}
