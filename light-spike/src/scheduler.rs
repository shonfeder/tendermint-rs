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
            Event::LightClient(e) => {
                let res = self.light_client.handle(e);
                self.route_event(res)
            }
            Event::Verifier(e) => {
                let res = self.verifier.handle(e);
                self.route_event(res)
            }
            Event::Requester(e) => {
                let res = self.requester.handle(e);
                self.route_event(res)
            }
            _ => unreachable!(),
        }
    }

    fn route_event(&self, event: Event) -> Event {
        match event {
            Event::LightClient(LightClientEvent::VerifiedTrustedStates { .. }) => event,

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

            Event::Verifier(VerifierEvent::VerifiedTrustedState(trusted_state)) => {
                LightClientEvent::NewTrustedState(trusted_state).into()
            }

            Event::Verifier(VerifierEvent::BisectionNeeded {
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

            event => event,
        }
    }
}

