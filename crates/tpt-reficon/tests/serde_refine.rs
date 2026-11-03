//! `#[refine(serde)]` routes deserialization through `try_new`.

use serde::de::value::{Error, U32Deserializer};
use serde::de::IntoDeserializer;
use serde::Deserialize;
use tpt_reficon::prelude::*;

#[derive(Refine, Debug)]
#[refine(serde)]
#[requires(value >= 1)]
pub struct NonZero(u32);

#[test]
fn deserialize_accepts_a_valid_value() {
    let input: U32Deserializer<Error> = 5u32.into_deserializer();
    assert_eq!(*NonZero::deserialize(input).unwrap(), 5);
}

#[test]
fn deserialize_rejects_a_value_that_breaks_the_refinement() {
    // Untrusted input must not build an invalid value, so the refinement is
    // checked on the way in, and the failure becomes a deserialization error.
    let input: U32Deserializer<Error> = 0u32.into_deserializer();
    let err = NonZero::deserialize(input).unwrap_err();
    assert!(err.to_string().contains("must hold"), "{err}");
}
