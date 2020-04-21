use std::time::{Duration, SystemTime};

use pred::{Assertion, Pred};

use crate::{predicates::*, prelude::*, requester::RequesterEvent};

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

