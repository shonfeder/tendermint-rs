use std::sync::mpsc::{Receiver, SyncSender};

use crate::{
    light_client::{LightClient, LightClientInput, LightClientOutput},
    prelude::*,
    requester::{Requester, RequesterInput, RequesterOutput},
    verifier::{Verifier, VerifierInput, VerifierOutput},
};

pub struct Scheduler {
    light_client: LightClient,
    verifier: Verifier,
    requester: Requester,
}

impl Scheduler {
    pub fn new(light_client: LightClient, verifier: Verifier, requester: Requester) -> Self {
        Self {
            light_client,
            verifier,
            requester,
        }
    }

    pub fn run(&mut self, sender: SyncSender<Input>, receiver: Receiver<Input>) {
        loop {
            let event = receiver.recv().unwrap();

            match event {
                Input::Terminate => break,
                Input::Tick => todo!(),
                event => {
                    let next_event = self.handle(event);
                    sender.send(next_event).unwrap();
                }
            }
        }
    }

    pub fn handle(&mut self, event: Input) -> Input {
        match event {
            Input::LightClient(event) => match self.light_client.handle(event) {
                Ok(res) => self.route_event(Output::LightClient(res)),
                Err(err) => todo!(),
            },
            Input::Verifier(e) => match self.verifier.handle(e) {
                Ok(res) => self.route_event(Output::Verifier(res)),
                Err(err) => todo!(),
            },
            Input::Requester(e) => match self.requester.handle(e) {
                Ok(res) => self.route_event(Output::Requester(res)),
                Err(err) => todo!(),
            },
            _ => unreachable!(),
        }
    }

    fn route_event(&self, event: Output) -> Input {
        match event {
            Output::LightClient(LightClientOutput::NewTrustedStates { .. }) => {
                todo!() // route back to caller
            }

            Output::LightClient(LightClientOutput::PerformVerification {
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            }) => Input::Verifier(VerifierInput::VerifyAtHeight {
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            }),

            Output::Verifier(VerifierOutput::StateNeeded(height)) => {
                RequesterInput::FetchState(height).into()
            }

            Output::Verifier(VerifierOutput::StateVerified(trusted_state)) => {
                LightClientInput::NewTrustedState(trusted_state).into()
            }

            Output::Verifier(VerifierOutput::VerificationNeeded {
                trusted_state,
                pivot_height,
                trust_threshold,
                trusting_period,
                now,
            }) => LightClientInput::VerifyAtHeight {
                trusted_state,
                untrusted_height: pivot_height,
                trust_threshold,
                trusting_period,
                now,
            }
            .into(),

            Output::Requester(RequesterOutput::FetchedState {
                height,
                signed_header,
                validator_set,
                next_validator_set,
            }) => VerifierInput::FetchedState {
                height,
                untrusted_sh: signed_header,
                untrusted_vals: validator_set,
                untrusted_next_vals: next_validator_set,
            }
            .into(),

            Output::NoOp => Input::NoOp,
        }
    }
}

