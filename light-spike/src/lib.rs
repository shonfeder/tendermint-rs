#![allow(unreachable_code, dead_code, unused_variables)]

pub mod light_client;
pub mod predicates;
pub mod prelude;
pub mod requester;
pub mod scheduler;
pub mod trusted_store;
pub mod verifier;

use crate::{
    light_client::{LightClientInput, LightClientOutput},
    requester::{RequesterInput, RequesterOutput},
    verifier::{VerifierInput, VerifierOutput},
};

pub trait Handler<Input> {
    type Output;
    type Error;

    fn handle(&mut self, event: Input) -> Result<Self::Output, Self::Error>;
}

pub enum Input {
    NoOp,
    Tick,
    Terminate,
    Verifier(VerifierInput),
    LightClient(LightClientInput),
    Requester(RequesterInput),
}

impl From<VerifierInput> for Input {
    fn from(e: VerifierInput) -> Self {
        Self::Verifier(e)
    }
}

impl From<LightClientInput> for Input {
    fn from(e: LightClientInput) -> Self {
        Self::LightClient(e)
    }
}

impl From<RequesterInput> for Input {
    fn from(e: RequesterInput) -> Self {
        Self::Requester(e)
    }
}

pub enum Output {
    NoOp,
    Verifier(VerifierOutput),
    LightClient(LightClientOutput),
    Requester(RequesterOutput),
}

impl From<VerifierOutput> for Output {
    fn from(e: VerifierOutput) -> Self {
        Self::Verifier(e)
    }
}

impl From<LightClientOutput> for Output {
    fn from(e: LightClientOutput) -> Self {
        Self::LightClient(e)
    }
}

impl From<RequesterOutput> for Output {
    fn from(e: RequesterOutput) -> Self {
        Self::Requester(e)
    }
}
