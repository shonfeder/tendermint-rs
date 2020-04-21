#![allow(unreachable_code, dead_code, unused_variables)]

pub mod inner_verifier;
pub mod predicates;
pub mod prelude;
pub mod requester;
pub mod scheduler;
pub mod verifier;

use crate::{
    inner_verifier::InnerVerifierEvent, requester::RequesterEvent, verifier::VerifierEvent,
};

pub trait Handler<Input> {
    fn handle(&mut self, event: Input) -> Event;
}

pub enum Event {
    VerifierEvent(VerifierEvent),
    InnerVerifierEvent(InnerVerifierEvent),
    RequesterEvent(RequesterEvent),
    NoOp,
}

impl From<VerifierEvent> for Event {
    fn from(e: VerifierEvent) -> Self {
        Self::VerifierEvent(e)
    }
}

impl From<InnerVerifierEvent> for Event {
    fn from(e: InnerVerifierEvent) -> Self {
        Self::InnerVerifierEvent(e)
    }
}

impl From<RequesterEvent> for Event {
    fn from(e: RequesterEvent) -> Self {
        Self::RequesterEvent(e)
    }
}

