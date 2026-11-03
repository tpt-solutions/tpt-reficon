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
- **`ensures` checks the exit value.** On `#[contract]`, `#[ensures(...)]` is
  checked against `result` when the function returns. It is not a full
  post-state model: `old(x)` names the entry value of an integer parameter, and
  it covers integer returns only. A `mut` parameter disables elision, because the body can
  change it.
- **`unsafe` only where C is called.** `unsafe_code` is forbidden across the
  workspace. The one exception is `ffi_wrappers!`, whose expansion calls C. That
  call is confined to a wrapper marked `#[allow(unsafe_code)]`, and a wrapper
  that takes a raw pointer is `unsafe fn`. The derive and `#[contract]` expand
  to no `unsafe`.
- **No unchecked constructor.** `Refine` has no public constructor that skips the
  predicate. Values enter only through `new`, `try_new`, and the `TryFrom` impl.
