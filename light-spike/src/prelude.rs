pub use crate::components::demuxer::*;
pub use crate::components::io::*;
pub use crate::components::scheduler::*;
pub use crate::components::verifier::*;
pub use crate::errors::*;
pub use crate::event::*;
pub use crate::operations::*;
pub use crate::predicates::errors::*;
pub use crate::predicates::VerificationPredicates;
pub use crate::trusted_store::*;
pub use crate::types::*;
pub use crate::utils::*;
pub use crate::{ensure, impl_event, unwrap};

pub use std::time::{Duration, SystemTime};

pub use genawaiter::{
    rc::{Co, Gen},
    Coroutine, GeneratorState,
};

pub(crate) trait BoolExt {
    fn true_or<E>(self, e: E) -> Result<(), E>;
    fn false_or<E>(self, e: E) -> Result<(), E>;
}

impl BoolExt for bool {
    fn true_or<E>(self, e: E) -> Result<(), E> {
        if self {
            Ok(())
        } else {
            Err(e)
        }
    }

    fn false_or<E>(self, e: E) -> Result<(), E> {
        if !self {
            Ok(())
        } else {
            Err(e)
        }
    }
}
