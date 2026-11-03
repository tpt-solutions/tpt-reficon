# tpt-reficon-verifier

A self-contained decision procedure for quantifier-free linear real arithmetic
(QF_LRA), plus memory-layout and alignment proofs for `repr(C)` structs. It is
the solver behind [`tpt-reficon`](https://crates.io/crates/tpt-reficon), and it
also works on its own.

The Fourier–Motzkin elimination engine, exact rational arithmetic, and the
layout proof logic are ported and adapted from TPT Solutions'
`tpt-telos-verifier` and `tpt-telos-uir-bridge`. See the workspace
[NOTICE](https://github.com/tpt-solutions/tpt-reficon/blob/master/NOTICE) file.

## Contents

- [What it decides](#what-it-decides)
- [Example](#example)
- [Soundness](#soundness)
- [Modules](#modules)
- [Feature flags](#feature-flags)
- [Benchmarks](#benchmarks)
- [Minimum supported Rust version](#minimum-supported-rust-version)
- [License](#license)

## What it decides

Given a set of linear constraints over named real variables, the solver returns
one of three verdicts:

- **Satisfiable**, with a witness: a `Model` assigning a rational to each
  variable. Every returned witness is checked against the constraints before it
  is returned.
- **Unsatisfiable.**
- **Unknown**, meaning "could not prove". This covers overflow, exponential
  elimination blow-up, budget exhaustion for `!=` case-splitting, and shapes the
  engine does not support. `Unknown` is never reported as either of the other
  verdicts.

Constraints use the relations `<`, `<=`, `==`, `>=`, `>`, and `!=`.

## Example

A `Constraint(lin, rel)` asserts `lin rel 0`. So `x <= 100` is the constraint
`x - 100 <= 0`.

```rust
use tpt_reficon_verifier::ir::{Constraint, Linear, Relation};
use tpt_reficon_verifier::{decide, entails, Verdict};

// x > 0 ∧ x ≤ 100 ⊨ x ≥ 0 ?
let premises = [
    Constraint(Linear::var("x"), Relation::Gt),
    Constraint(Linear::var("x").add(&Linear::constant_only(-100)), Relation::Le),
];
let conclusion = Constraint(Linear::var("x"), Relation::Ge);
assert!(entails(&premises, &conclusion));

// Feasibility, with a witness when satisfiable:
match decide(&premises).expect("well-formed system") {
    Verdict::Sat(model) => println!("witness: {model:?}"),
    Verdict::Unsat => println!("no solution"),
    Verdict::Unknown(reason) => println!("could not decide: {reason}"),
}
```

## Soundness

- `unsat` never returns `true` for a satisfiable system over ℚ. The crate's
  property tests check this against a brute-force rational-grid oracle.
- `satisfies` treats evaluation overflow as "satisfied" rather than panicking.
  An adversarial model, such as one containing `i128::MAX`, cannot turn an
  overflow into a spurious UNSAT.
- Witness reconstruction is validated. The bounds-based reconstruction for
  multi-variable systems was a past source of invalid witnesses, and it is now
  covered by regression tests.
- Arithmetic is exact rational arithmetic with checked operations. There is no
  `unsafe`.

## Modules

| Module | Purpose |
| --- | --- |
| `ir` | `Linear`, `Constraint`, `Relation`, `Model`. Negation, substitution, and evaluation over models. |
| `solver` | `decide`, `unsat`, `unsat_checked`, `model`, `entails`, `counterexample`, and the `Verdict` enum. |
| `rational` | Exact rational numbers with checked arithmetic. |
| `layout` | `Alignment`, `StructLayout`, `prove_equivalent` (deterministic match against a foreign layout), and `prove_within_bounds` (solver-backed check that fields fit and do not overlap). |
| `permission` | Research prototype: non-interference check for fractional permissions, via `check_configuration`. |

## Feature flags

| Flag | Effect |
| --- | --- |
| `z3-reserved` | Reserved extension point for a future backend that routes nonlinear arithmetic or very large systems to Z3. It is not implemented, and enabling it changes nothing. |

## Benchmarks

The crate has a dependency-free benchmark, run with `cargo bench`. It measures
solver latency on bounds, `!=` splitting and its budget, equality chains,
multi-variable Fourier–Motzkin, and unsatisfiable or degenerate systems.

```sh
cargo bench -p tpt-reficon-verifier
```

## Minimum supported Rust version

1.75.0.

## License

Licensed under either of [MIT](https://github.com/tpt-solutions/tpt-reficon/blob/master/LICENSE-MIT)
or [Apache-2.0](https://github.com/tpt-solutions/tpt-reficon/blob/master/LICENSE-APACHE)
at your option.
