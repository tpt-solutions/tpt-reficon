---
name: Bug report
about: Report incorrect behavior in the verifier, macros, or generated code
title: ''
labels: ['bug']
assignees: []
---

**Summary**
<!-- One or two sentences describing the bug. -->

**Reproducer**

```rust
// Minimal code / constraint set that triggers the issue.
```

**Expected vs actual**
<!-- What did you expect to happen, what happened instead? -->

**Verifier verdict involved**
- [ ] Solver said UNSAT for a satisfiable system
- [ ] Solver said SAT for an unsatisfiable system
- [ ] Wrong elision decision (check removed although not provable)
- [ ] Layout proof accepted/rejected incorrectly
- [ ] Macro diagnostic problem (span/message)
- [ ] Other

**Environment**
- OS:
- Rust version (`rustc --version`):
- crate version:
