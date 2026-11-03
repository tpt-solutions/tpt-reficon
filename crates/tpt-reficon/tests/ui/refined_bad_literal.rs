use tpt_reficon::prelude::*;

#[derive(Refine, Debug)]
#[requires(value >= 1)]
pub struct Port(u16);

fn main() {
    // Zero breaks `value >= 1`, so this must not compile.
    let _ = tpt_reficon::refined!(Port, 0);
}
