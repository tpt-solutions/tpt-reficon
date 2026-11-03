#![no_main]

//! Feeds arbitrary bytes to the QF_LRA decision procedure as linear
//! constraints. The property is that `decide` never panics and that any
//! verdict it returns is internally consistent. Soundness against a reference
//! oracle is covered by the proptest suite in `tpt-reficon-verifier`.

use libfuzzer_sys::fuzz_target;
use tpt_reficon_verifier::ir::{Constraint, Linear, Relation};
use tpt_reficon_verifier::{decide, Verdict};

const VARS: &[&str] = &["x", "y", "z"];
const RELS: &[Relation] = &[
    Relation::Le,
    Relation::Lt,
    Relation::Ge,
    Relation::Gt,
    Relation::Eq,
    Relation::Ne,
];

fuzz_target!(|data: &[u8]| {
    let mut bytes = data.iter().copied();
    let mut next = move || bytes.next().unwrap_or(0);

    let count = (next() % 6) as usize;
    let mut cs = Vec::with_capacity(count);
    for _ in 0..count {
        let mut lin = Linear::constant_only(i64::from(next() as i8));
        let terms = next() % 3;
        for _ in 0..terms {
            let var = VARS[usize::from(next()) % VARS.len()];
            let coeff = i64::from(next() as i8);
            if let Some(next) = Linear::var(var)
                .checked_scale(coeff)
                .and_then(|scaled| lin.checked_add(&scaled))
            {
                lin = next;
            }
        }
        let rel = RELS[usize::from(next()) % RELS.len()];
        cs.push(Constraint(lin, rel));
    }

    match decide(&cs) {
        Ok(Verdict::Sat(model)) => {
            assert!(tpt_reficon_verifier::ir::satisfies_all(&cs, &model));
        }
        Ok(Verdict::Unsat) | Ok(Verdict::Unknown(_)) | Err(_) => {}
    }
});
