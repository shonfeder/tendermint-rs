//! Sketch out the pseudo code for a light client
//! That integrates the learning from the last iteration.
//! What we want:
//! + Simple light client specific types, no crypto
//! + Crypto can abstracted into traits which implement crypto specific functions
//! + Express the core verification logic as a composition of predicates to allow mocking

#![allow(dead_code, unreachable_code)]

use derive_more::Display;
use std::time::{Duration, SystemTime};

// Some simplified types which only have the fields needed for core verification

type Hash = u64;
type Height = u64;

#[derive(Debug, Copy, Clone)]
enum Error {
    InvalidCommit,
    InvalidValidatorSet,
    InvalidNextValidatorSet,
    InvalidCommitValue,
    ImplementationSpecific,
    NonIncreasingHeight,
    NonMonotonicBftTime,
    InsufficientVotingPower,
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
struct Header {
    height: Height,
    bft_time: SystemTime,
    validator_set_hash: Hash,
    next_validator_set_hash: Hash,
    hash: Hash, // What if we don't have this
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
struct ValidatorSet {
    hash: Hash,
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
struct Commit {
    header_hash: Hash,
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
struct TrustLevel {
    numerator: u64,
    denominator: u64,
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
struct SignedHeader {
    header: Header,
    commit: Commit,
    validators: ValidatorSet,
    validator_hash: Hash,
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
struct TrustedState {
    header: Header,
    validators: ValidatorSet,
}

// Crypto function traits allowing mocking out during testing
trait VotingPowerCalculator: Sized {
    // What kind of errors should we be reporting here?
    fn voting_power_in(&self, commit: &Commit, validators: &ValidatorSet) -> Result<u64, Error>;
    fn total_power_of(&self, validators: &ValidatorSet) -> Result<u64, Error>;
}

trait CommitValidator: Sized {
    fn validate(&self, commit: &Commit, validators: &ValidatorSet) -> Result<(), Error>;
}

trait HeaderHasher: Sized {
    fn hash(&self, header: &Header) -> Hash; // Or Error?
}

/// Predicates

fn validator_sets_match(signed_header: &SignedHeader, validators: &ValidatorSet) -> bool {
    signed_header.validator_hash == validators.hash
}

fn next_validators_match(signed_header: &SignedHeader, validators: &ValidatorSet) -> bool {
    signed_header.validator_hash == validators.hash
}

fn header_matches_commit(
    header: &Header,
    commit: &Commit,
    header_hasher: &impl HeaderHasher,
) -> bool {
    header_hasher.hash(header) == commit.header_hash
}

fn valid_commit(
    commit: &Commit,
    validators: &ValidatorSet,
    validator: &impl CommitValidator,
) -> bool {
    validator.validate(commit, validators).is_ok()
}

fn is_within_trusted_period(header: &Header, trusting_period: Duration, now: SystemTime) -> bool {
    let header_time: SystemTime = header.bft_time.into();
    let expires_at = header_time + trusting_period;

    header_time < now && expires_at > now
}

fn is_monotonic_bft_time(header_a: &Header, header_b: &Header) -> bool {
    header_b.bft_time >= header_a.bft_time
}

fn is_monotonic_height(header_a: &Header, header_b: &Header) -> bool {
    header_a.height > header_b.height
}

fn has_sufficient_voting_power(
    commit: &Commit,
    validators: &ValidatorSet,
    trust_level: &TrustLevel,
    calculator: &impl VotingPowerCalculator,
) -> bool {
    let total_power = calculator.total_power_of(validators);
    let voting_power = calculator.voting_power_in(commit, validators);

    if let (Ok(total_power), Ok(voting_power)) = (total_power, voting_power) {
        // XXX: Maybe trust_level doesn't need a very sophisticated type
        voting_power * trust_level.denominator > total_power * trust_level.numerator
    } else {
        false
    }
}

fn has_sufficient_validators_overlap(
    untrusted_commit: &Commit,
    trusted_validators: &ValidatorSet,
    trust_level: &TrustLevel,
    calculator: &impl VotingPowerCalculator,
) -> bool {
    has_sufficient_voting_power(
        untrusted_commit,
        trusted_validators,
        trust_level,
        calculator,
    )
}

fn has_sufficient_signers_overlap(
    untrusted_commit: &Commit,
    untrusted_validators: &ValidatorSet,
    trust_level: &TrustLevel,
    calculator: &impl VotingPowerCalculator,
) -> bool {
    has_sufficient_voting_power(
        untrusted_commit,
        untrusted_validators,
        trust_level,
        calculator,
    )
}
fn invalid_next_validator_set(
    trusted_state: &TrustedState,
    untrusted_sh: &SignedHeader,
    untrusted_next_vals: &ValidatorSet,
) -> bool {
    untrusted_sh.header.height == trusted_state.header.height
        && trusted_state.validators.hash != untrusted_next_vals.hash
}

fn verify(
    trusted_state: TrustedState,
    untrusted_sh: SignedHeader,
    untrusted_vals: ValidatorSet,
    untrusted_next_vals: ValidatorSet,
    trust_level: TrustLevel,

    // Operations
    validator: impl CommitValidator + Clone,
    calculator: impl VotingPowerCalculator + Clone,
    header_hasher: impl HeaderHasher + Clone,
) -> Result<(), Error> {
    // shouldn't this return a new TrustedState?

    if !validator_sets_match(&untrusted_sh, &untrusted_vals) {
        return Err(Error::InvalidValidatorSet);
    }

    if !next_validators_match(&untrusted_sh, &untrusted_next_vals) {
        return Err(Error::InvalidNextValidatorSet);
    }

    if !header_matches_commit(&untrusted_sh.header, &untrusted_sh.commit, &header_hasher) {
        return Err(Error::InvalidCommitValue);
    }

    if !valid_commit(&untrusted_sh.commit, &untrusted_sh.validators, &validator) {
        return Err(Error::ImplementationSpecific);
    }

    if !is_monotonic_bft_time(&untrusted_sh.header, &trusted_state.header) {
        return Err(Error::NonMonotonicBftTime);
    }

    if !is_monotonic_height(&trusted_state.header, &untrusted_sh.header) {
        return Err(Error::NonIncreasingHeight);
    }

    // XXX: why not integrate this into next_validators_match check?
    if !invalid_next_validator_set(&trusted_state, &untrusted_sh, &untrusted_next_vals) {
        return Err(Error::InvalidNextValidatorSet);
    }

    if !has_sufficient_validators_overlap(
        &untrusted_sh.commit,
        &trusted_state.validators,
        &trust_level,
        &calculator,
    ) {
        return Err(Error::InsufficientVotingPower);
    }

    if !has_sufficient_signers_overlap(
        &untrusted_sh.commit,
        &untrusted_vals,
        &trust_level,
        &calculator,
    ) {
        return Err(Error::InvalidCommit);
    }

    Ok(())
}

//  TODO: Now do the bisection logic as a sequence of verify applications
