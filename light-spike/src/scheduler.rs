use std::sync::mpsc;

use crate::{
    inner_verifier::{InnerVerifier, InnerVerifierEvent},
    prelude::*,
    requester::{Requester, RequesterEvent},
    verifier::{Verifier, VerifierEvent},
};

pub struct Scheduler {
    verifier: Verifier,
    inner_verifier: InnerVerifier,
    requester: Requester,
    recv_input: mpsc::Receiver<Event>,
    send_output: mpsc::SyncSender<Event>,
}

impl Scheduler {
    pub fn new(
        verifier: Verifier,
        inner_verifier: InnerVerifier,
        requester: Requester,
    ) -> (mpsc::SyncSender<Event>, mpsc::Receiver<Event>, Self) {
        let (send_input, recv_input) = mpsc::sync_channel(16);
        let (send_output, recv_output) = mpsc::sync_channel(16);

        let scheduler = Self {
            verifier,
            inner_verifier,
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
                Event::VerifierEvent(VerifierEvent::VerifiedTrustedStates { .. }) => {
                    self.send_output.send(event).unwrap()
                }
                Event::VerifierEvent(e) => {
                    let res = self.verifier.handle(e);
                    next_event = Some(self.route_event(res));
                }
                Event::RequesterEvent(e) => {
                    let res = self.requester.handle(e);
                    next_event = Some(self.route_event(res));
                }
                Event::InnerVerifierEvent(e) => unreachable!(),
                Event::NoOp => continue,
            }
        }
    }

    fn route_event(&self, event: Event) -> Event {
        match event {
            Event::RequesterEvent(RequesterEvent::FetchedState {
                height,
                signed_header,
                validator_set,
                next_validator_set,
            }) => InnerVerifierEvent::FetchedState {
                height,
                untrusted_sh: signed_header,
                untrusted_vals: validator_set,
                untrusted_next_vals: next_validator_set,
            }
            .into(),
            Event::InnerVerifierEvent(InnerVerifierEvent::VerifiedTrustedState(trusted_state)) => {
                VerifierEvent::NewTrustedState(trusted_state).into()
            }
            Event::InnerVerifierEvent(InnerVerifierEvent::BisectionNeeded {
                trusted_state,
                pivot_height,
                trust_threshold,
                trusting_period,
                now,
            }) => VerifierEvent::VerifyAtHeight {
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

