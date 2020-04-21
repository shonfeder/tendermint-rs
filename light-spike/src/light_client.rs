use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, SystemTime},
};

use crate::{prelude::*, trusted_store::TSReadWriter};

pub enum LightClientEvent {
    // Inputs
    VerifyAtHeight {
        trusted_state: TrustedState,
        untrusted_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    },
    NewTrustedState(TrustedState),

    // Outputs
    PerformVerification {
        trusted_state: TrustedState,
        untrusted_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    },
    NewTrustedStates {
        trusted_height: Height,
        trusted_states: Vec<TrustedState>,
    },
}

pub struct PendingState {
    trusted_state: TrustedState,
    untrusted_height: Height,
    trust_threshold: TrustThreshold,
    trusting_period: Duration,
    now: SystemTime,
}

pub struct LightClient {
    trusted_store: TSReadWriter,
    pending_heights: VecDeque<Height>,
    pending_states: HashMap<Height, PendingState>,
    verified_states: Vec<TrustedState>,
}

impl LightClient {
    pub fn new(trusted_store: TSReadWriter) -> Self {
        Self {
            trusted_store,
            pending_heights: VecDeque::new(),
            pending_states: HashMap::new(),
            verified_states: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.pending_heights.clear();
        self.pending_states.clear();
    }
}

impl Handler<LightClientEvent> for LightClient {
    fn handle(&mut self, event: LightClientEvent) -> Event {
        match event {
            LightClientEvent::VerifyAtHeight {
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            } => {
                let pending_state = PendingState {
                    trusted_state: trusted_state.clone(),
                    untrusted_height,
                    trust_threshold,
                    trusting_period,
                    now,
                };

                self.pending_heights.push_front(untrusted_height);
                self.pending_states.insert(untrusted_height, pending_state);

                LightClientEvent::PerformVerification {
                    trusted_state,
                    untrusted_height,
                    trust_threshold,
                    trusting_period,
                    now,
                }
                .into()
            }
            LightClientEvent::NewTrustedState(new_trusted_state) => {
                let new_height = new_trusted_state.header.height;
                let latest_height_to_verify = self.pending_heights.pop_front();

                match latest_height_to_verify {
                    // The height of the new trusted state matches the next height we needed to verify.
                    Some(latest_height_to_verify) if latest_height_to_verify == new_height => {
                        let pending_state = self.pending_states.remove(&latest_height_to_verify);

                        let pending_state = match pending_state {
                            None => {
                                // No matching pending state found.
                                // TODO: Raise error.
                                return Event::NoOp;
                            }
                            Some(pending_state) => pending_state,
                        };

                        self.trusted_store
                            .set(new_height, new_trusted_state.clone());

                        self.verified_states.push(new_trusted_state.clone());

                        if let Some(next_height_to_verify) = self.pending_heights.front() {
                            // We have more states to verify
                            LightClientEvent::PerformVerification {
                                trusted_state: new_trusted_state,
                                untrusted_height: *next_height_to_verify,
                                trust_threshold: pending_state.trust_threshold,
                                trusting_period: pending_state.trusting_period,
                                now: pending_state.now,
                            }
                            .into()
                        } else {
                            // No more heights to verify, we are done, return all verified states
                            let verified_states =
                                std::mem::replace(&mut self.verified_states, Vec::new());

                            self.reset();

                            LightClientEvent::NewTrustedStates {
                                trusted_height: latest_height_to_verify,
                                trusted_states: verified_states.into(),
                            }
                            .into()
                        }
                    }
                    // The height of the new trusted state does not match the latest height we needed to verify.
                    Some(latest_height_to_verify) => {
                        Event::NoOp // TODO: Yield an error
                    }
                    // There were no more heights to verify, ignore the event.
                    None => Event::NoOp,
                }
            }
            _ => unreachable!(),
        }
    }
}

