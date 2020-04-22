use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use pred::{Assertion, Pred, Predicate};

use crate::{predicates::*, prelude::*};

pub enum VerifierError {
    VerificationFailed(crate::prelude::Error),
    NoMatchingPendingState(Height),
    NotWithinTrustingPeriod {
        header: Header,
        trusting_period: Duration,
        now: SystemTime,
    },
}

pub enum VerifierInput {
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
}

pub enum VerifierOutput {
    StateVerified(TrustedState),
    StateNeeded(Height),
    VerificationNeeded {
        trusted_state: TrustedState,
        pivot_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    },
}

pub struct PendingState {
    trusted_state: TrustedState,
    untrusted_height: Height,
    trust_threshold: TrustThreshold,
    trusting_period: Duration,
    now: SystemTime,
}

pub struct Verifier {
    voting_power_calculator: Box<dyn VotingPowerCalculator>,
    commit_validator: Box<dyn CommitValidator>,
    header_hasher: Box<dyn HeaderHasher>,
    pending_states: HashMap<Height, PendingState>,
}

impl Handler<VerifierInput> for Verifier {
    type Output = VerifierOutput;
    type Error = VerifierError;

    fn handle(&mut self, event: VerifierInput) -> Result<VerifierOutput, VerifierError> {
        use VerifierInput::*;

        match event {
            VerifyAtHeight {
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            } => self.on_verify_at_height(
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            ),
            FetchedState {
                height,
                untrusted_sh,
                untrusted_vals,
                untrusted_next_vals,
            } => self.on_fetched_state(height, untrusted_sh, untrusted_vals, untrusted_next_vals),
        }
    }
}

impl Verifier {
    pub fn new(
        voting_power_calculator: impl VotingPowerCalculator + 'static,
        commit_validator: impl CommitValidator + 'static,
        header_hasher: impl HeaderHasher + 'static,
    ) -> Self {
        Self {
            voting_power_calculator: Box::new(voting_power_calculator),
            commit_validator: Box::new(commit_validator),
            header_hasher: Box::new(header_hasher),
            pending_states: HashMap::new(),
        }
    }

    fn on_verify_at_height(
        &mut self,
        trusted_state: TrustedState,
        untrusted_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    ) -> Result<VerifierOutput, VerifierError> {
        let within_trust_period =
            is_within_trust_period(&trusted_state.header, trusting_period, now).eval();

        if !within_trust_period {
            return Err(VerifierError::NotWithinTrustingPeriod {
                header: trusted_state.header,
                trusting_period,
                now,
            });
        }

        self.start_verification(
            trusted_state,
            untrusted_height,
            trust_threshold,
            trusting_period,
            now,
        )
    }

    fn start_verification(
        &mut self,
        trusted_state: TrustedState,
        untrusted_height: Height,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    ) -> Result<VerifierOutput, VerifierError> {
        self.pending_states.insert(
            untrusted_height,
            PendingState {
                trusted_state,
                untrusted_height,
                trust_threshold,
                trusting_period,
                now,
            },
        );

        Ok(VerifierOutput::StateNeeded(untrusted_height))
    }

    fn on_fetched_state(
        &mut self,
        height: Height,
        untrusted_sh: SignedHeader,
        untrusted_vals: ValidatorSet,
        untrusted_next_vals: ValidatorSet,
    ) -> Result<VerifierOutput, VerifierError> {
        let pending_state = self
            .pending_states
            .remove(&height)
            .ok_or_else(|| VerifierError::NoMatchingPendingState(height))?;

        self.perform_verification(
            pending_state.trusted_state,
            untrusted_sh,
            untrusted_vals,
            untrusted_next_vals,
            pending_state.trust_threshold,
            pending_state.trusting_period,
            pending_state.now,
        )
    }

    fn perform_verification(
        &mut self,
        trusted_state: TrustedState,
        untrusted_sh: SignedHeader,
        untrusted_vals: ValidatorSet,
        untrusted_next_vals: ValidatorSet,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    ) -> Result<VerifierOutput, VerifierError> {
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

                Ok(VerifierOutput::StateVerified(new_trusted_state))
            }
            Err(Error::InsufficientVotingPower) => {
                // Insufficient voting power to update.  Need bisection.
                let pivot_height = self.compute_pivot_height(&trusted_state, &untrusted_sh);

                Ok(VerifierOutput::VerificationNeeded {
                    trusted_state,
                    pivot_height,
                    trust_threshold,
                    trusting_period,
                    now,
                })
            }
            Err(err) => Err(VerifierError::VerificationFailed(err)),
        }
    }

    pub fn compute_pivot_height(
        &self,
        trusted_state: &TrustedState,
        untrusted_sh: &SignedHeader,
    ) -> Height {
        let trusted_h = trusted_state.header.height;
        let untrusted_h = untrusted_sh.header.height;
        let pivot_height = trusted_h.checked_add(untrusted_h).expect("height overflow") / 2;
        pivot_height
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
