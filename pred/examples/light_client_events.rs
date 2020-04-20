#![allow(unreachable_code, dead_code, unused_variables)]

use std::{
    collections::VecDeque,
    time::{Duration, SystemTime},
};

use pred::{light_client::*, Assertion, Pred};

pub trait Handler<Input> {
    fn handle(&mut self, event: Input) -> Event;
}

pub enum Event {
    VerifierEvent(VerifierEvent),
    InnerVerifierEvent(InnerVerifierEvent),
    RequesterEvent(RequesterEvent),
    NoOp,
}

impl From<VerifierEvent> for Event {
    fn from(e: VerifierEvent) -> Self {
        Self::VerifierEvent(e)
    }
}

impl From<InnerVerifierEvent> for Event {
    fn from(e: InnerVerifierEvent) -> Self {
        Self::InnerVerifierEvent(e)
    }
}

impl From<RequesterEvent> for Event {
    fn from(e: RequesterEvent) -> Self {
        Self::RequesterEvent(e)
    }
}

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

pub enum VerifierEvent {
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

pub struct Verifier {
    heights_to_verify: VecDeque<Height>,
    verified_states: VecDeque<TrustedState>,
}

impl Verifier {
    pub fn new() -> Self {
        Self {
            heights_to_verify: VecDeque::new(),
            verified_states: VecDeque::new(),
        }
    }
}

impl Handler<VerifierEvent> for Verifier {
    fn handle(&mut self, event: VerifierEvent) -> Event {
        match event {
            VerifierEvent::VerifyAtHeight {
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            } => {
                self.heights_to_verify.push_front(untrusted_height);

                InnerVerifierEvent::VerifyAtHeight {
                    trusted_state,
                    untrusted_height,
                    trust_threshold,
                    trusting_period,
                    now,
                }
                .into()
            }
            VerifierEvent::NewTrustedState(new_trusted_state) => {
                let new_height = new_trusted_state.header.height;
                let latest_height_to_verify = self.heights_to_verify.pop_front();

                match latest_height_to_verify {
                    // The height of the new trusted state matches the next height we needed to verify.
                    Some(latest_height_to_verify) if latest_height_to_verify == new_height => {
                        self.verified_states.push_front(new_trusted_state.clone());

                        if let Some(next_height_to_verify) = self.heights_to_verify.front() {
                            // We have more states to verify
                            InnerVerifierEvent::VerifyAtHeight {
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

                            VerifierEvent::VerifiedTrustedStates {
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

pub enum InnerVerifierEvent {
    // Inputs
    VerifyAtHeight {
        trusted_state: TrustedState,
        untrusted_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    },
    FetchedState {
        height: Height,
        untrusted_sh: SignedHeader,
        untrusted_vals: ValidatorSet,
        untrusted_next_vals: ValidatorSet,
    },
    // Outputs
    VerifiedTrustedState(TrustedState),
    BisectionNeeded {
        trusted_state: TrustedState,
        pivot_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    },
}

pub enum InnerVerifierState {
    Ready,
    Unknown,
    WaitingForUntrustedState {
        trusted_state: TrustedState,
        untrusted_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    },
}

pub struct InnerVerifier {
    voting_power_calculator: Box<dyn VotingPowerCalculator>,
    commit_validator: Box<dyn CommitValidator>,
    header_hasher: Box<dyn HeaderHasher>,
    state: InnerVerifierState,
}

impl Handler<InnerVerifierEvent> for InnerVerifier {
    fn handle(&mut self, event: InnerVerifierEvent) -> Event {
        use InnerVerifierEvent::*;
        use InnerVerifierState::*;

        let state = std::mem::replace(&mut self.state, Unknown);

        match (state, event) {
            (
                Ready,
                VerifyAtHeight {
                    trusted_state,
                    untrusted_height,
                    trust_threshold,
                    trusting_period,
                    now,
                },
            ) => self.init_verification(
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            ),
            (
                WaitingForUntrustedState {
                    trusted_state,
                    untrusted_height,
                    trust_threshold,
                    trusting_period,
                    now,
                },
                FetchedState {
                    height,
                    untrusted_sh,
                    untrusted_vals,
                    untrusted_next_vals,
                },
            ) => {
                if untrusted_height != height {
                    // TODO: Raise error
                    self.state = InnerVerifierState::Ready;
                    return Event::NoOp;
                }

                self.perform_verification(
                    trusted_state,
                    untrusted_sh,
                    untrusted_vals,
                    untrusted_next_vals,
                    trust_threshold,
                    trusting_period,
                    now,
                )
            }
            _ => unreachable!(),
        }
    }
}

impl InnerVerifier {
    pub fn new(
        voting_power_calculator: impl VotingPowerCalculator + 'static,
        commit_validator: impl CommitValidator + 'static,
        header_hasher: impl HeaderHasher + 'static,
    ) -> Self {
        Self {
            voting_power_calculator: Box::new(voting_power_calculator),
            commit_validator: Box::new(commit_validator),
            header_hasher: Box::new(header_hasher),
            state: InnerVerifierState::Ready,
        }
    }

    pub fn init_verification(
        &mut self,
        trusted_state: TrustedState,
        untrusted_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    ) -> Event {
        if let Err(err) =
            is_within_trust_period(&trusted_state.header, trusting_period, now).assert()
        {
            // TODO: Report errror
            self.state = InnerVerifierState::Ready;
            return Event::NoOp;
        }

        self.start_verification(
            trusted_state,
            untrusted_height,
            trust_threshold,
            trusting_period,
            now,
        )
    }

    pub fn start_verification(
        &mut self,
        trusted_state: TrustedState,
        untrusted_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    ) -> Event {
        self.state = InnerVerifierState::WaitingForUntrustedState {
            trusted_state,
            untrusted_height,
            trust_threshold,
            trusting_period,
            now,
        };

        RequesterEvent::FetchState(untrusted_height).into()
    }

    pub fn perform_verification(
        &mut self,
        trusted_state: TrustedState,
        untrusted_sh: SignedHeader,
        untrusted_vals: ValidatorSet,
        untrusted_next_vals: ValidatorSet,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    ) -> Event {
        let result = self.verify_untrusted_state(
            &trusted_state,
            &untrusted_sh,
            &untrusted_vals,
            &untrusted_next_vals,
            &trust_threshold,
            &trusting_period,
            &now,
        );

        match result {
            Ok(()) => {
                let new_trusted_state = TrustedState {
                    header: untrusted_sh.header,
                    validators: untrusted_vals,
                };

                InnerVerifierEvent::VerifiedTrustedState(new_trusted_state).into()
            }
            Err(Error::InsufficientVotingPower) => {
                // Insufficient voting power to update.  Need bisection.

                // Get the pivot height for bisection.
                let trusted_h = trusted_state.header.height;
                let untrusted_h = untrusted_sh.header.height;
                let pivot_height = trusted_h.checked_add(untrusted_h).expect("height overflow") / 2;

                InnerVerifierEvent::BisectionNeeded {
                    trusted_state,
                    pivot_height,
                    trust_threshold,
                    trusting_period,
                    now,
                }
                .into()
            }
            Err(err) => {
                // TODO: Report error
                self.state = InnerVerifierState::Ready;
                Event::NoOp
            }
        }
    }

    pub fn verify_untrusted_state(
        &self,
        trusted_state: &TrustedState,
        untrusted_sh: &SignedHeader,
        untrusted_vals: &ValidatorSet,
        untrusted_next_vals: &ValidatorSet,
        trust_threshold: &TrustThreshold,
        trusting_period: &Duration,
        now: &SystemTime,
    ) -> Result<(), Error> {
        let predicate = self.build_verify_predicate(
            &trusted_state,
            &untrusted_sh,
            &untrusted_vals,
            &untrusted_next_vals,
            &trust_threshold,
            &trusting_period,
            &now,
        );

        predicate.assert()
    }

    pub fn build_verify_predicate<'a>(
        &'a self,
        trusted_state: &'a TrustedState,
        untrusted_sh: &'a SignedHeader,
        untrusted_vals: &'a ValidatorSet,
        untrusted_next_vals: &'a ValidatorSet,
        trust_threshold: &'a TrustThreshold,
        trusting_period: &'a Duration,
        now: &'a SystemTime,
    ) -> impl Pred<Error> + 'a {
        let p_validator_sets_match = validator_sets_match(&untrusted_sh, &untrusted_vals);
        let p_next_validators_match = next_validators_match(&untrusted_sh, &untrusted_next_vals);

        let p_header_matches_commit = header_matches_commit(
            &untrusted_sh.header,
            &untrusted_sh.commit,
            &self.header_hasher,
        );

        let p_valid_commit = valid_commit(
            &untrusted_sh.commit,
            &untrusted_sh.validators,
            &self.commit_validator,
        );

        let p_is_monotonic_bft_time =
            is_monotonic_bft_time(&untrusted_sh.header, &trusted_state.header);

        let p_is_monotonic_height =
            is_monotonic_height(&trusted_state.header, &untrusted_sh.header);

        let p_valid_next_validator_set =
            valid_next_validator_set(&trusted_state, &untrusted_sh, &untrusted_next_vals);

        let p_has_sufficient_validators_overlap = has_sufficient_validators_overlap(
            &untrusted_sh.commit,
            &trusted_state.validators,
            &trust_threshold,
            &self.voting_power_calculator,
        );

        let p_has_sufficient_signers_overlap = has_sufficient_signers_overlap(
            &untrusted_sh.commit,
            &untrusted_vals,
            &trust_threshold,
            &self.voting_power_calculator,
        );

        let verify_pred = verify_pred(
            p_validator_sets_match,
            p_next_validators_match,
            p_header_matches_commit,
            p_valid_commit,
            p_is_monotonic_bft_time,
            p_is_monotonic_height,
            p_valid_next_validator_set,
            p_has_sufficient_validators_overlap,
            p_has_sufficient_signers_overlap,
        );

        verify_pred
    }
}

use std::sync::mpsc;

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

fn main() {}
