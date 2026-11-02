# Build-time daemon and caching (Phase 6 design)

Status: **not built, and not planned.** The measurement below shows the solver
is too cheap for a daemon to pay for itself. The design is kept in case that
changes.

## Measurement

`cargo bench -p tpt-reficon-verifier` (release, thin LTO), ns per `decide` call:

| Scenario | ns/op |
| --- | --- |
| empty system | 54 |
| bounded single variable | 1,825 |
| strict disjoint bounds (UNSAT) | 1,482 |
| equality reference | 3,737 |
| `!=` premises within budget | 2,441 |
| two-variable polytope | 4,715 |
| four-variable elimination | 11,290 |
| thirty bounds, one variable | 15,870 |

Even the heaviest scenario costs about 16 µs. A crate with 1,000 refined types
spends on the order of 10 ms in the solver. A cold `rustc` proc-macro load
costs far more than that, and it is paid per crate, not per predicate.

**Decision:** do not build the daemon, and do not build the in-process fallback
that would sit beside it. Solver latency does not justify the IPC, the
trust-boundary work, or the cache store. Revisit only if a measured expansion
profile shows the solver dominating on a real workload.

## Recommendation: measure before building

The daemon only pays off when solver time dominates macro-expansion time. The
workspace benchmarks cover `decide` latency (`crates/tpt-reficon-verifier/benches/solver.rs`),
but not expansion overhead in `tpt-reficon-macros`. Before any IPC work, measure
the end-to-end cost of expanding a crate with many `#[derive(Refine)]` types,
using `cargo build --timings` on a crate with a realistic number of refined
types. If solving is a small share, do not build the daemon; a per-process
memoization is enough.

## Cache key

Key = hash of:

1. the predicate AST in canonical form (the rendered `Constraint` list, sorted
   and variable-renamed),
2. the inner type's domain,
3. the verifier crate version and an explicit IR format version.

Any change to these produces a new key, so no separate invalidation pass is
needed. Stale entries are never read.

## Trust and soundness

The cache is a performance feature. It must never change a verdict. Threats and
responses:

- **Corrupt or tampered entry.** Each entry stores the verdict plus a checksum.
  A `Sat` verdict is re-checked against its stored model before use (cheap). An
  `Unsat` or elision verdict is not re-proved on load, so it is trusted only
  from a file owned by the current user with owner-only permissions. On any
  doubt, the entry is dropped and the solver runs in-process.
- **Poisoned daemon.** Local-user socket / named pipe only. The daemon refuses
  connections from other users.
- **Version skew.** The key includes the verifier version, so an old daemon
  cannot answer for a new solver.

## IPC

- Unix: a domain socket under the user's runtime directory.
- Windows: a named pipe scoped to the user.
- Protocol: one request per key, one response with the verdict or "miss". No
  request is ever sent for a predicate the compiler has not produced.

## Fallback

The macro always tries the in-process solver if the daemon is absent, slow, or
returns an error. The daemon is an optimization, so the build must succeed with
it disabled. Timeout for the daemon round-trip: a small fixed budget, after which
the macro solves locally.

## Open items (only if the daemon is revisited)

- Whether `Unknown` results should be cached. Recommendation: no. They are cheap
  to recompute and caching them hides budget changes.
- Storage format. Recommendation: an append-only file with per-record checksums,
  using no external dependency, matching the workspace's dependency policy.
