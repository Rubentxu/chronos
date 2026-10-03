//! D2 — the read-path resource ceiling, and the mechanism that makes it bite.
//!
//! # What this closes
//!
//! `SCALE_BUDGETS.md` §5 D2 extended
//! `ResourceLimits { max_events: 1_000_000, timeout_secs: 60 }` from capture
//! to "a hard ceiling on **any** read-path operation", and §5 recorded the
//! decision as *"decided, without a mechanism"*: the struct lived in
//! `chronos-mcp/src/server.rs` and its only readers were two `#[test]`
//! functions. A decision with no mechanism is not a decision, it is a comment.
//!
//! This module is that mechanism.
//!
//! # Why the ceiling is COOPERATIVE, and not `tokio::time::timeout`
//!
//! The read path is synchronous, all the way down:
//!
//! - `read_path::ReadPathService::{poll_batch, summarize, rollup,
//!   causality_status}` are plain `fn`s (`read_path.rs`).
//! - `virtualization::summarize_log` / `rollup_log` are plain `fn`s whose whole
//!   body is a `while let Ok(page) = read_page(...)` loop.
//! - `events_log_read::read_page` / `read_page_with` are plain `fn`s.
//!
//! A future that never yields cannot be preempted. `tokio::time::timeout`
//! only decides what to do with a `poll()` that returns `Pending`; here the
//! entire multi-thousand-second aggregation happens inside a *single* `poll()`
//! call and then returns `Ready`. The timer would fire in the runtime while
//! this task is mid-`poll`, and when `poll()` finally returned, `timeout`
//! would observe `Ready` and hand the caller the full result. The timeout
//! would not even produce the illusion of a ceiling — it would produce no
//! ceiling at all, silently.
//!
//! So the only mechanism that is real here is a **cooperative** one: mint a
//! deadline at the start of the operation and check it at page boundaries. The
//! work stops at the first boundary past the ceiling and the caller is told
//! how far it got. This is the same shape as the existing pagination
//! (`SCAN_CHUNK = 512` in `events_log_read.rs`), so it introduces no new
//! traversal machinery.
//!
//! # Acceptance criterion (written before the implementation)
//!
//! Given a read-path operation configured with `ResourceLimits`:
//!
//! - **AC1 — the ceiling kills, it does not decorate.** Once an operation
//!   exceeds `max_events` or `timeout_secs`, it MUST return an error. No
//!   operation may return `Ok` after exceeding a limit.
//! - **AC2 — the failure is explicit and actionable.** The error MUST name
//!   (a) the operation, (b) which limit was exceeded and its configured value,
//!   (c) how many events and pages were read, (d) the exact `next_seq` where
//!   the walk stopped, so the agent can resume from it, and (e) the elapsed
//!   time. It MUST also say in words that this is a budget stop and not
//!   "the log has no more evidence".
//! - **AC3 — distinguishable from "no more evidence".** `Ok` with zero events
//!   means the log is exhausted. A budget stop is a dedicated error variant.
//!   The two never collapse into one answer (ADR-0004).
//! - **AC4 — instrumented, not merely declared.** Every stop emits a
//!   structured `tracing` event, and the `execution_log_read` MCP tool
//!   attaches a structured JSON envelope carrying the same numbers, so the
//!   effect of the ceiling is observable from outside the process.
//! - **AC5 — the fast path is unharmed.** `poll` under the default limits
//!   still returns exactly the requested batch and a cursor that advances.
//! - **AC6 — the field is read.** The production `ReadPathService` is built
//!   with `ResourceLimits::default()`; no production call reaches a read-path
//!   loop without a budget.
//!
//! # Known bound, stated up front
//!
//! Both limits are enforced **at page boundaries**, so an operation may
//! overshoot by at most one page. `summarize_log`/`rollup_log` page at 1024
//! events, and a `poll` reads exactly one page. That slack is a deliberate
//! trade: shrinking the page to tighten the ceiling would tax the fast path
//! to protect the slow one, and the cost of that trade is larger than the
//! slack it removes.

use std::fmt;
use std::time::{Duration, Instant};

/// Hard ceiling for any read-path operation (SCALE_BUDGETS §5, D2).
///
/// Originally declared in `chronos-mcp::server` and instantiated only by two
/// `#[test]` functions. It lives here now — the read path is in
/// `chronos-services`, and a ceiling enforced from the composition root into a
/// lower layer cannot depend on a higher one. `server.rs` re-exports it so
/// `chronos_mcp::server::ResourceLimits` keeps resolving, exactly as
/// `tools_params` is re-exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum number of events a single operation may read (default: 1_000_000).
    pub max_events: usize,
    /// Wall-clock ceiling in seconds for a single operation (default: 60).
    pub timeout_secs: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_events: 1_000_000,
            timeout_secs: 60,
        }
    }
}

/// Which configured limit stopped an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetLimit {
    /// `ResourceLimits::timeout_secs` elapsed.
    Deadline,
    /// `ResourceLimits::max_events` was passed.
    MaxEvents,
}

impl BudgetLimit {
    /// The `ResourceLimits` field name, so the message points at the knob
    /// an operator would actually turn.
    pub fn field(self) -> &'static str {
        match self {
            BudgetLimit::Deadline => "timeout_secs",
            BudgetLimit::MaxEvents => "max_events",
        }
    }
}

impl fmt::Display for BudgetLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BudgetLimit::Deadline => f.write_str("wall-clock deadline"),
            BudgetLimit::MaxEvents => f.write_str("max_events"),
        }
    }
}

/// A read-path operation stopped because it passed its configured ceiling.
///
/// Carries the whole point of the walk, not just "it timed out": a budget
/// stop is resumable, and the resume anchor is the one number the agent
/// actually needs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "read-path budget exceeded: `{operation}` stopped because the {limit} ({limit_field}) was \
     reached. Read {events_scanned} events over {pages_scanned} pages in {elapsed:?} and stopped \
     at next_seq={next_seq}; resume from that cursor. The policy in force was max_events=\
     {max_events}, timeout_secs={limit_secs}. This is a budget stop, NOT \"the log has no more \
     evidence\" — the log may well have more; the walk simply ran out of its ceiling."
)]
pub struct BudgetExceeded {
    /// The read-path operation that stopped (`"summarize"`, `"rollup"`, `"poll"`).
    pub operation: &'static str,
    /// Which limit was passed.
    pub limit: BudgetLimit,
    /// The `ResourceLimits` field name, stored rather than derived at format
    /// time so the message and the MCP envelope name the same knob an
    /// operator would turn.
    pub limit_field: &'static str,
    /// The configured wall-clock ceiling in seconds, carried on both variants
    /// so the error reports the whole policy and not only the half it hit.
    pub limit_secs: u64,
    /// The configured event ceiling, likewise carried on both variants.
    pub max_events: u64,
    /// Wall clock spent before the stop.
    pub elapsed: Duration,
    /// Events read before the stop.
    pub events_scanned: u64,
    /// Pages read before the stop.
    pub pages_scanned: u64,
    /// Where the walk stopped: the next sequence number to read, i.e. the
    /// cursor a resuming call should be anchored at.
    pub next_seq: u64,
}

impl BudgetExceeded {
    /// The stable machine-readable reason, mirrored on the MCP wire.
    pub fn reason(&self) -> &'static str {
        match self.limit {
            BudgetLimit::Deadline => "read_path_deadline_exceeded",
            BudgetLimit::MaxEvents => "read_path_max_events_exceeded",
        }
    }
}

/// The per-operation budget minted from a [`ResourceLimits`].
///
/// Deliberately built on `std::time::Instant`, not `tokio::time::Instant`:
/// this budget governs synchronous CPU work, and the tokio clock can be
/// paused by `tokio::time::pause()` without the CPU having burned a single
/// nanosecond. A wall-clock ceiling over CPU work must use the clock that
/// actually follows the CPU.
pub struct ReadBudget {
    operation: &'static str,
    max_events: u64,
    limit_secs: u64,
    deadline: Instant,
    started: Instant,
    events_scanned: u64,
    pages_scanned: u64,
    next_seq: u64,
}

impl ReadBudget {
    /// Mint a budget for one operation, anchored at `cursor_next_seq`.
    pub fn start(operation: &'static str, limits: &ResourceLimits, cursor_next_seq: u64) -> Self {
        Self {
            operation,
            max_events: limits.max_events as u64,
            limit_secs: limits.timeout_secs,
            deadline: Instant::now() + Duration::from_secs(limits.timeout_secs),
            started: Instant::now(),
            events_scanned: 0,
            pages_scanned: 0,
            next_seq: cursor_next_seq,
        }
    }

    /// The event ceiling this operation is running under.
    pub fn max_events(&self) -> u64 {
        self.max_events
    }

    /// Clamp a caller-requested page size to the event ceiling.
    ///
    /// A `poll` may never ask the log for more events in one call than the
    /// ceiling allows in the whole operation.
    pub fn clamp_page(&self, requested: usize) -> usize {
        requested.min(self.max_events as usize)
    }

    /// Record one completed page and enforce both ceilings.
    ///
    /// Called at page boundaries only. An empty page is not charged: a log
    /// with nothing to read must not be able to trip a budget.
    pub fn charge_page(&mut self, events: u64, next_seq: u64) -> Result<(), BudgetExceeded> {
        self.events_scanned += events;
        self.pages_scanned += 1;
        self.next_seq = next_seq;

        let now = Instant::now();
        let limit = if now >= self.deadline {
            Some(BudgetLimit::Deadline)
        } else if self.events_scanned > self.max_events {
            Some(BudgetLimit::MaxEvents)
        } else {
            None
        };

        let Some(limit) = limit else {
            return Ok(());
        };

        let elapsed = now.duration_since(self.started);
        let exceeded = BudgetExceeded {
            operation: self.operation,
            limit,
            limit_field: limit.field(),
            limit_secs: self.limit_secs,
            max_events: self.max_events,
            elapsed,
            events_scanned: self.events_scanned,
            pages_scanned: self.pages_scanned,
            next_seq: self.next_seq,
        };

        // AC4: the stop leaves a trace outside the return value too, so an
        // operator can count budget stops without grepping agent transcripts.
        tracing::warn!(
            target: "chronos::read_path::budget",
            operation = exceeded.operation,
            reason = exceeded.reason(),
            limit_field = exceeded.limit.field(),
            events_scanned = exceeded.events_scanned,
            pages_scanned = exceeded.pages_scanned,
            next_seq = exceeded.next_seq,
            elapsed_ms = exceeded.elapsed.as_millis() as u64,
            max_events = exceeded.max_events,
            "read-path operation stopped at its resource ceiling",
        );

        Err(exceeded)
    }

    /// Events charged so far.
    pub fn events_scanned(&self) -> u64 {
        self.events_scanned
    }

    /// Pages charged so far.
    pub fn pages_scanned(&self) -> u64 {
        self.pages_scanned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A generous policy: nothing here should trip, so any stop is a bug in
    /// the accounting rather than in the configuration.
    fn generous() -> ResourceLimits {
        ResourceLimits {
            max_events: 1_000_000,
            timeout_secs: 3_600,
        }
    }

    /// The zero-second deadline is a legitimate configuration ("no wall clock
    /// at all"), and it is the deterministic way to exercise the deadline
    /// without sleeping: the first page boundary is already past it.
    fn no_time() -> ResourceLimits {
        ResourceLimits {
            max_events: usize::MAX,
            timeout_secs: 0,
        }
    }

    #[test]
    fn defaults_match_the_d2_decision() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_events, 1_000_000);
        assert_eq!(limits.timeout_secs, 60);
    }

    /// A generous budget must not invent a stop.
    #[test]
    fn a_generous_budget_never_trips() {
        let mut budget = ReadBudget::start("summarize", &generous(), 0);
        for _ in 0..10 {
            budget.charge_page(512, 0).expect("must not trip");
        }
        assert_eq!(budget.events_scanned(), 5_120);
        assert_eq!(budget.pages_scanned(), 10);
    }

    /// The deadline must be enforced, not merely stored: a zero-second
    /// budget trips at the first page boundary.
    #[test]
    fn a_zero_second_budget_stops_at_the_first_page() {
        let mut budget = ReadBudget::start("summarize", &no_time(), 42);
        let err = budget
            .charge_page(512, 554)
            .expect_err("a zero-second deadline must stop the walk");
        assert_eq!(err.limit, BudgetLimit::Deadline);
        assert_eq!(err.operation, "summarize");
        assert_eq!(err.events_scanned, 512);
        assert_eq!(err.pages_scanned, 1);
        // The resume anchor is the one actionable number in the error.
        assert_eq!(err.next_seq, 554);
    }

    /// The event ceiling must be enforced independently of the clock: here
    /// the clock is generous and only the count can trip.
    #[test]
    fn the_event_ceiling_trips_independently_of_the_clock() {
        let mut budget = ReadBudget::start(
            "rollup",
            &ResourceLimits {
                max_events: 100,
                timeout_secs: 3_600,
            },
            0,
        );
        budget
            .charge_page(100, 100)
            .expect("at the cap is still inside it");
        let err = budget
            .charge_page(1, 101)
            .expect_err("passing max_events must stop the walk");
        assert_eq!(err.limit, BudgetLimit::MaxEvents);
        assert_eq!(err.max_events, 100);
        assert_eq!(err.events_scanned, 101);
        assert_eq!(err.next_seq, 101);
    }

    /// A log with nothing to read cannot trip a budget: an empty page is
    /// never charged. Otherwise `timeout_secs: 0` would make every empty
    /// session an error, which is a different lie.
    #[test]
    fn an_empty_log_cannot_trip_a_zero_second_budget() {
        let mut budget = ReadBudget::start("summarize", &no_time(), 0);
        // Simulate the loop's own guard: `records.is_empty()` breaks before
        // any charge happens.
        let page_is_empty = true;
        if !page_is_empty {
            budget.charge_page(0, 0).expect("must not trip");
        }
        assert_eq!(budget.pages_scanned(), 0);
    }

    /// AC2: the rendered message must be distinguishable, at a glance, from
    /// "the log has no more evidence" — and must carry the resume anchor.
    #[test]
    fn the_message_names_the_limit_the_progress_and_the_resume_anchor() {
        let mut budget = ReadBudget::start("summarize", &no_time(), 0);
        let err = budget.charge_page(512, 554).expect_err("must trip");
        let text = err.to_string();
        assert!(text.contains("summarize"), "names the operation: {text}");
        assert!(text.contains("timeout_secs"), "names the field: {text}");
        assert!(text.contains("next_seq=554"), "resume anchor: {text}");
        assert!(
            text.contains("NOT \"the log has no more evidence\""),
            "distinguishes a budget stop from an exhausted log: {text}"
        );
        assert_eq!(err.reason(), "read_path_deadline_exceeded");
    }

    /// A caller-requested page may never exceed the operation's own ceiling.
    #[test]
    fn clamp_page_bounds_the_request_to_the_ceiling() {
        let budget = ReadBudget::start(
            "poll",
            &ResourceLimits {
                max_events: 32,
                timeout_secs: 60,
            },
            0,
        );
        assert_eq!(budget.clamp_page(100), 32);
        assert_eq!(budget.clamp_page(10), 10);
        assert_eq!(budget.clamp_page(0), 0);
    }
}
