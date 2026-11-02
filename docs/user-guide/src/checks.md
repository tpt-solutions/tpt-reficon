# How checks are decided

The derive makes three decisions for every predicate, in this order.

## 1. Satisfiability (compile error)

The predicate is converted to disjunctive normal form. The derive reports a
compile error only when **every** alternative is unsatisfiable. A single
satisfiable alternative is enough to keep the type.

For primitive integer inner types, each alternative is also checked for
integer feasibility: a single-variable constraint system over ℤ is solved exactly
and intersected with the type's domain. This is what makes `value > 5 && value < 6`
an error, even though the rational relaxation `5 < value < 6` is satisfiable.

## 2. Domain elision (zero cost)

The derive elides the check only when it can **prove** the predicate follows from
the inner type's domain. An alternative that the domain implies is enough: the
disjunction `value <= 10 || value <= 255` on `u8` elides, because the second
alternative is always true.

Elision is branch-wise. A tautology whose alternatives are only jointly
exhaustive, such as `value == 0 || value != 0`, is **not** elided, so it keeps a
check that always succeeds. This is deliberate: when in doubt, the derive emits
the check.

## 3. Runtime check

Otherwise the derive emits the predicate as an expression in `new` and
`try_new`. Integer checks run in `i128` (see [Writing predicates](predicates.md)).

`new` panics on violation with a message naming the type and predicate.
`try_new` returns a `RefinementError` carrying the same text.

## When the solver gives up

Fourier–Motzkin elimination is capped, and any excess becomes "could not prove".
The derive then treats the predicate as unproven and emits a runtime check. It
never reports a spurious unsatisfiable verdict.
