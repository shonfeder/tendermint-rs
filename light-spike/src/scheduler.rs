use std::sync::mpsc;

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
    recv_input: mpsc::Receiver<Event>,
    send_output: mpsc::SyncSender<Event>,
}

impl Scheduler {
    pub fn new(
        light_client: LightClient,
        verifier: Verifier,
        requester: Requester,
    ) -> (mpsc::SyncSender<Event>, mpsc::Receiver<Event>, Self) {
        let (send_input, recv_input) = mpsc::sync_channel(16);
        let (send_output, recv_output) = mpsc::sync_channel(16);

        let scheduler = Self {
            light_client,
            verifier,
            requester,
            recv_input,
            send_output,
        };

        (send_input, recv_output, scheduler)
    }

    pub fn run(&mut self) {
        let mut next_event = None;

        loop {
            let event = next_event
                .take()
                .unwrap_or_else(|| self.recv_input.recv().unwrap());

            match event {
                Event::LightClient(LightClientEvent::VerifiedTrustedStates { .. }) => {
                    self.send_output.send(event).unwrap()
                }
                Event::LightClient(e) => {
                    let res = self.light_client.handle(e);
                    next_event = Some(self.route_event(res));
                }
                Event::Requester(e) => {
                    let res = self.requester.handle(e);
                    next_event = Some(self.route_event(res));
                }
                Event::Verifier(e) => unreachable!(),
                Event::NoOp => continue,
            }
        }
    }

    fn route_event(&self, event: Event) -> Event {
        match event {
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

