use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, SystemTime},
};

use crate::{prelude::*, trusted_store::TSReadWriter};

#[derive(Clone, Debug)]
pub enum LightClientError {
    NoMatchingPendingState(Height),
}

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

    fn reset(&mut self) -> Vec<TrustedState> {
        self.pending_heights.clear();
        self.pending_states.clear();

        std::mem::replace(&mut self.verified_states, Vec::new())
    }

    fn save_trusted_state(&mut self, trusted_state: TrustedState) -> Result<(), LightClientError> {
        let height = trusted_state.header.height;

        self.trusted_store.set(height, trusted_state.clone());
        self.verified_states.push(trusted_state);

        Ok(())
    }
}

impl Handler<LightClientEvent> for LightClient {
    type Error = LightClientError;

    fn handle(&mut self, event: LightClientEvent) -> Result<LightClientEvent, LightClientError> {
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

                Ok(LightClientEvent::PerformVerification {
                    trusted_state,
                    untrusted_height,
                    trust_threshold,
                    trusting_period,
                    now,
                })
            }
            LightClientEvent::NewTrustedState(new_trusted_state) => {
                self.save_trusted_state(new_trusted_state.clone())?;

                if let Some(pending_height) = self.pending_heights.pop_front() {
                    // We have more states to verify
                    let pending_state = self
                        .pending_states
                        .remove(&pending_height)
                        .ok_or_else(|| LightClientError::NoMatchingPendingState(pending_height))?;

                    Ok(LightClientEvent::PerformVerification {
                        trusted_state: new_trusted_state,
                        untrusted_height: pending_height,
                        trust_threshold: pending_state.trust_threshold,
                        trusting_period: pending_state.trusting_period,
                        now: pending_state.now,
                    })
                } else {
                    // No more pending heights to verify, we are done, return all verified states
                    let verified_states = self.reset();

                    Ok(LightClientEvent::NewTrustedStates {
                        trusted_height: new_trusted_state.header.height,
                        trusted_states: verified_states.into(),
                    })
                }
            }
            _ => unreachable!(),
        }
    }
}

