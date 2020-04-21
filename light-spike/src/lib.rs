#![allow(unreachable_code, dead_code, unused_variables)]

pub mod light_client;
pub mod predicates;
pub mod prelude;
pub mod requester;
pub mod scheduler;
pub mod verifier;

use crate::{light_client::LightClientEvent, requester::RequesterEvent, verifier::VerifierEvent};

pub trait Handler<Input> {
    fn handle(&mut self, event: Input) -> Event;
}

pub enum Event {
    Verifier(VerifierEvent),
    LightClient(LightClientEvent),
    Requester(RequesterEvent),
    NoOp,
}

impl From<VerifierEvent> for Event {
    fn from(e: VerifierEvent) -> Self {
        Self::Verifier(e)
    }
}

impl From<LightClientEvent> for Event {
    fn from(e: LightClientEvent) -> Self {
        Self::LightClient(e)
    }
}

impl From<RequesterEvent> for Event {
    fn from(e: RequesterEvent) -> Self {
        Self::Requester(e)
    }
}

