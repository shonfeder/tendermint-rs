use std::sync::mpsc::{Receiver, SyncSender};

use crate::{
    light_client::{LightClient, LightClientEvent},
    prelude::*,
    requester::{Requester, RequesterEvent},
    verifier::{Verifier, VerifierEvent},
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

    pub fn run(&mut self, sender: SyncSender<Event>, receiver: Receiver<Event>) {
        loop {
            let event = receiver.recv().unwrap();

            match event {
                Event::NoOp => continue,
                Event::Terminate => break,
                Event::Tick => todo!(),
                event => {
                    let next_event = self.handle(event);
                    sender.send(next_event).unwrap();
                }
            }
        }
    }

    pub fn handle(&mut self, event: Event) -> Event {
        match event {
            Event::LightClient(event) => match self.light_client.handle(event) {
                Ok(res) => self.route_event(Event::LightClient(res)),
                Err(err) => todo!(),
            },
            Event::Verifier(e) => match self.verifier.handle(e) {
                Ok(res) => self.route_event(Event::Verifier(res)),
                Err(err) => todo!(),
            },
            Event::Requester(e) => match self.requester.handle(e) {
                Ok(res) => self.route_event(Event::Requester(res)),
                Err(err) => todo!(),
            },
            _ => unreachable!(),
        }
    }

    fn route_event(&self, event: Event) -> Event {
        match event {
            Event::LightClient(LightClientEvent::PerformVerification {
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            }) => Event::Verifier(VerifierEvent::VerifyAtHeight {
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            }),

            Event::Verifier(VerifierEvent::StateNeeded(height)) => {
                RequesterEvent::FetchState(height).into()
            }

            Event::Verifier(VerifierEvent::StateVerified(trusted_state)) => {
                LightClientEvent::NewTrustedState(trusted_state).into()
            }

            Event::Verifier(VerifierEvent::VerificationNeeded {
                trusted_state,
                pivot_height,
                trust_threshold,
                trusting_period,
                now,
            }) => LightClientEvent::VerifyAtHeight {
                trusted_state,
                untrusted_height: pivot_height,
                trust_threshold,
                trusting_period,
                now,
            }
            .into(),

            Event::Requester(RequesterEvent::FetchedState {
                height,
                signed_header,
                validator_set,
                next_validator_set,
            }) => VerifierEvent::FetchedState {
                height,
                untrusted_sh: signed_header,
                untrusted_vals: validator_set,
                untrusted_next_vals: next_validator_set,
            }
            .into(),

            event => event,
        }
    }
}

