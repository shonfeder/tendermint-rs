use crate::prelude::*;

pub enum RequesterEvent {
    // Inputs
    FetchSignedHeader(Height),
    FetchValidatorSet(Height),
    FetchState(Height),
    // Outputs
    SignedHeader(Height, SignedHeader),
    ValidatorSet(Height, ValidatorSet),
    FetchedState {
        height: Height,
        signed_header: SignedHeader,
        validator_set: ValidatorSet,
        next_validator_set: ValidatorSet,
    },
}

pub struct Requester {}

impl Requester {
    pub fn new() -> Self {
        Self {}
    }
}

impl Handler<RequesterEvent> for Requester {
    fn handle(&mut self, event: RequesterEvent) -> Event {
        use RequesterEvent::*;

        match event {
            FetchSignedHeader(_height) => todo!(),
            FetchValidatorSet(_height) => todo!(),
            FetchState(_height) => todo!(),
            _ => unreachable!(),
        }
    }
}

