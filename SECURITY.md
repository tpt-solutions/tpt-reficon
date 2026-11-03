# Security policy

## Supported versions

tpt-reficon has not published a release yet. Security fixes land on `master`
and are released with the next version.

## Reporting a vulnerability

Report vulnerabilities privately, through GitHub's private vulnerability
reporting: open the **Security** tab of this repository and choose **Report a
vulnerability**, or go directly to
<https://github.com/tpt-solutions/tpt-reficon/security/advisories/new>.

Please do not open a public issue for a security finding. For a verifier
soundness bug (the solver or the macros reporting a wrong verdict), include the
smallest constraint set or predicate that reproduces it.

We aim to acknowledge a report within a few working days, and to publish a fix
and an advisory together once a fix exists.

## Scope

In scope: wrong verdicts from the solver, a refinement or contract that is
accepted when it should be rejected, unsound generated code, and memory-safety
problems in the `ffi_wrappers!` output.

Out of scope: a failing refinement check that is reported correctly, and
problems in the C libraries or headers that `ffi_contract` and `ffi_wrappers!`
read.
