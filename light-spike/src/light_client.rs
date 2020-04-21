use std::{
    collections::VecDeque,
    time::{Duration, SystemTime},
};

use crate::{prelude::*, verifier::VerifierEvent};

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
    VerifiedTrustedStates {
        trusted_height: Height,
        trusted_states: Vec<TrustedState>,
    },
}

pub struct LightClient {
    heights_to_verify: VecDeque<Height>,
    verified_states: VecDeque<TrustedState>,
}

impl LightClient {
    pub fn new() -> Self {
        Self {
            heights_to_verify: VecDeque::new(),
            verified_states: VecDeque::new(),
        }
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
                self.heights_to_verify.push_front(untrusted_height);

                VerifierEvent::VerifyAtHeight {
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
                let latest_height_to_verify = self.heights_to_verify.pop_front();

                match latest_height_to_verify {
                    // The height of the new trusted state matches the next height we needed to verify.
                    Some(latest_height_to_verify) if latest_height_to_verify == new_height => {
                        self.verified_states.push_front(new_trusted_state.clone());

                        if let Some(next_height_to_verify) = self.heights_to_verify.front() {
                            // We have more states to verify
                            VerifierEvent::VerifyAtHeight {
                                trusted_state: new_trusted_state,
                                untrusted_height: *next_height_to_verify,
                                trust_threshold: todo!(),
                                trusting_period: todo!(),
                                now: todo!(),
                            }
                            .into()
                        } else {
                            // No more heights to verify, we are done, return all verified states
                            let verified_states =
                                std::mem::replace(&mut self.verified_states, VecDeque::new());

                            LightClientEvent::VerifiedTrustedStates {
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

