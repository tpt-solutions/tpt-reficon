# Soundness model

The rules below are what the project's tests and reviews defend. Changes to the
solver must keep them.

- **Decided over ℚ, with exact ℤ checks on integer domains.** The solver is an
  exact rational decision procedure. Integer-domain analysis is added on top, for
  single-variable predicates over primitive integers.
- **`i128` evaluation.** Runtime checks on integers never overflow the inner type.
- **Overflow is never a verdict.** Overflow, blow-up, and unsupported shapes
  degrade to "could not prove" rather than to a wrong answer.
- **Zero-cost means proven.** A check is removed only when the solver proves the
  predicate follows from the domain.
- **`ensures` is not yet a post-condition.** `#[ensures(...)]` is currently
  treated as an additional `requires(...)`. Real post-state semantics are tracked
  in `todo.md` under Phase 2.
- **No `unsafe`.** `unsafe_code` is forbidden across the workspace, including in
  generated code.
