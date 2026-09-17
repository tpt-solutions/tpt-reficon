//! Dependency-free micro-benchmarks for the QF_LRA decision procedure.
//!
//! Run with `cargo bench -p tpt-reficon-verifier` (release profile; the
//! workspace sets `lto = "thin"`). Each scenario reports nanoseconds per
//! `decide` call over enough iterations to fill a short measurement window.
//!
//! These cover the solver core (Fourier–Motzkin elimination, the `!=`
//! splitting budget, and degenerate/unsat shapes) so regressions in `decide`
//! latency are visible. Macro-expansion overhead in `tpt-reficon-macros` is
//! a separate concern and is not measured here; it belongs to a `cargo
//! build --timings`-style harness for the `#[derive(Refine)]` frontend.

use std::hint::black_box;
use std::time::{Duration, Instant};

use tpt_reficon_verifier::ir::{Constraint, Linear, Relation};
use tpt_reficon_verifier::solver::decide;

/// `k * var(name) + offset`, assembled from the public constructor surface.
fn lin(var: &str, k: i64, offset: i64) -> Linear {
    Linear::var(var)
        .checked_scale(k)
        .unwrap()
        .add(&Linear::constant_only(offset))
}

fn c(lin: Linear, rel: Relation) -> Constraint {
    Constraint(lin, rel)
}

fn benchmark(name: &str, f: impl Fn()) {
    f();
    let start = Instant::now();
    let mut iters = 0u64;
    while start.elapsed() < Duration::from_millis(50) {
        f();
        iters += 1;
    }
    let ns = start.elapsed().as_nanos() / iters as u128;
    println!("{name:<40} {ns:>10} ns/op ({iters} iters)");
}

fn main() {
    let scenarios: Vec<(&str, Vec<Constraint>)> = vec![
        ("empty_system", vec![]),
        (
            "bounded_single_variable",
            vec![
                c(lin("x", 1, -2), Relation::Ge),
                c(lin("x", 1, -10), Relation::Le),
            ],
        ),
        (
            "strict_disjoint_bounds_unsat",
            vec![
                c(lin("x", 1, -100), Relation::Gt),
                c(lin("x", 1, -50), Relation::Lt),
            ],
        ),
        (
            "far_disjoint_bounds_unsat",
            vec![
                c(lin("x", 1, -2), Relation::Ge),
                c(lin("x", 1, -1), Relation::Le),
            ],
        ),
        (
            "equality_reference",
            vec![
                c(lin("x", 1, -5), Relation::Eq),
                c(lin("y", 1, 0).add(&lin("x", -2, 0)), Relation::Eq),
            ],
        ),
        (
            "neq_premises_within_budget",
            vec![
                c(lin("x", 1, 0), Relation::Ne),
                c(lin("x", 1, -5), Relation::Ge),
                c(lin("x", 1, -5), Relation::Le),
            ],
        ),
        (
            "neq_premises_budget_exceeded",
            (0..10).map(|i| c(lin("x", 1, -i), Relation::Ne)).collect(),
        ),
        (
            "two_variable_polytope",
            vec![
                c(lin("x", 1, 0), Relation::Ge),
                c(lin("y", 1, 0), Relation::Ge),
                c(lin("x", 1, 0).add(&lin("y", 1, -1)), Relation::Le),
                c(lin("y", 1, 0).add(&lin("x", -2, -1)), Relation::Le),
            ],
        ),
        (
            "four_variable_elimination",
            vec![
                c(lin("v0", 1, 0), Relation::Ge),
                c(lin("v1", 1, 0), Relation::Ge),
                c(lin("v2", 1, 0), Relation::Ge),
                c(lin("v3", 1, 0), Relation::Ge),
                c(lin("v0", 1, 0).add(&lin("v1", 1, -10)), Relation::Le),
                c(lin("v1", 1, 0).add(&lin("v2", 1, -10)), Relation::Le),
                c(lin("v2", 1, 0).add(&lin("v3", 1, -10)), Relation::Le),
                c(lin("v0", 1, 0).add(&lin("v2", 1, -20)), Relation::Le),
            ],
        ),
        (
            "thirty_bounds_one_variable",
            (1..=30).map(|i| c(lin("x", 1, -i), Relation::Le)).collect(),
        ),
    ];

    println!("solver `decide` latency (release, thin LTO)\n");
    for (name, cs) in scenarios {
        benchmark(name, || {
            let _ = black_box(decide(black_box(&cs)));
        });
    }
}
