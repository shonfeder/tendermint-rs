//! Sketch out the pseudo code for a light client
//! That integrates the learning from the last iteration.
//! What we want:
//! + Simple light client specific types, no crypto
//! + Crypto can abstracted into traits which implement crypto specific functions
//! + Express the core verification logic as a composition of predicates to allow mocking

#![allow(dead_code, unreachable_code)]

use derive_more::Display;
use std::time::{Duration, SystemTime};

use pred::inspect::Inspect;
use pred::*;

// Some simplified types which only have the fields needed for core verification

type Hash = u64;
type Height = u64;

#[derive(Debug, Copy, Clone)]
pub enum Error {
    ImplementationSpecific,
    InsufficientValidatorsOverlap,
    InsufficientVotingPower,
    InvalidCommit,
    InvalidCommitValue,
    InvalidNextValidatorSet,
    InvalidValidatorSet,
    NonIncreasingHeight,
    NonMonotonicBftTime,
    NotWithinTrustedPeriod,
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

impl<T: VotingPowerCalculator> VotingPowerCalculator for &T {
    fn voting_power_in(&self, commit: &Commit, validators: &ValidatorSet) -> Result<u64, Error> {
        (*self).voting_power_in(commit, validators)
    }

    fn total_power_of(&self, validators: &ValidatorSet) -> Result<u64, Error> {
        (*self).total_power_of(validators)
    }
}

trait CommitValidator: Sized {
    fn validate(&self, commit: &Commit, validators: &ValidatorSet) -> Result<(), Error>;
}

impl<T: CommitValidator> CommitValidator for &T {
    fn validate(&self, commit: &Commit, validators: &ValidatorSet) -> Result<(), Error> {
        (*self).validate(commit, validators)
    }
}

trait HeaderHasher: Sized {
    fn hash(&self, header: &Header) -> Hash; // Or Error?
}

impl<T: HeaderHasher> HeaderHasher for &T {
    fn hash(&self, header: &Header) -> Hash {
        (*self).hash(header)
    }
}

/// Predicates

fn _validator_sets_match(signed_header: &SignedHeader, validators: &ValidatorSet) -> bool {
    signed_header.validator_hash == validators.hash
}

fn validator_sets_match<'a>(
    signed_header: &'a SignedHeader,
    validators: &'a ValidatorSet,
) -> impl Pred<Error> + 'a {
    pred::from_fn(move || _validator_sets_match(signed_header, validators))
        .named("validator_sets_match")
        .to_assert(|_| Error::InvalidValidatorSet)
}

fn _next_validators_match(signed_header: &SignedHeader, validators: &ValidatorSet) -> bool {
    signed_header.validator_hash == validators.hash
}

fn next_validators_match<'a>(
    signed_header: &'a SignedHeader,
    validators: &'a ValidatorSet,
) -> impl Pred<Error> + 'a {
    pred::from_fn(move || _next_validators_match(&signed_header, &validators))
        .named("next_validators_match")
        .to_assert(|_| Error::InvalidNextValidatorSet)
}

fn _header_matches_commit(
    header: &Header,
    commit: &Commit,
    header_hasher: impl HeaderHasher,
) -> bool {
    header_hasher.hash(header) == commit.header_hash
}

fn header_matches_commit<'a>(
    header: &'a Header,
    commit: &'a Commit,
    header_hasher: &'a impl HeaderHasher,
) -> impl Pred<Error> + 'a {
    pred::from_fn(move || _header_matches_commit(&header, &commit, &header_hasher))
        .named("header_matches_commit")
        .to_assert(|_| Error::InvalidCommitValue)
}

fn _valid_commit(
    commit: &Commit,
    validators: &ValidatorSet,
    validator: impl CommitValidator,
) -> bool {
    validator.validate(commit, validators).is_ok()
}

fn valid_commit<'a>(
    commit: &'a Commit,
    validators: &'a ValidatorSet,
    validator: &'a impl CommitValidator,
) -> impl Pred<Error> + 'a {
    pred::from_fn(move || _valid_commit(&commit, &validators, &validator))
        .named("valid_commit")
        .to_assert(|_| Error::ImplementationSpecific)
}

fn _is_within_trusted_period(header: &Header, trusting_period: Duration, now: SystemTime) -> bool {
    let header_time: SystemTime = header.bft_time.into();
    let expires_at = header_time + trusting_period;

    header_time < now && expires_at > now
}

fn is_within_trusted_period<'a>(
    header: &'a Header,
    trusting_period: Duration,
    now: SystemTime,
) -> impl Pred<Error> + 'a {
    pred::from_fn(move || _is_within_trusted_period(&header, trusting_period, now))
        .named("is_within_trusted_period")
        .to_assert(|_| Error::NotWithinTrustedPeriod)
}

fn _is_monotonic_bft_time(header_a: &Header, header_b: &Header) -> bool {
    header_b.bft_time >= header_a.bft_time
}

fn is_monotonic_bft_time<'a>(header_a: &'a Header, header_b: &'a Header) -> impl Pred<Error> + 'a {
    pred::from_fn(move || _is_monotonic_bft_time(&header_a, &header_b))
        .named("is_monotonic_bft_time")
        .to_assert(|_| Error::NonMonotonicBftTime)
}

fn _is_monotonic_height(header_a: &Header, header_b: &Header) -> bool {
    header_a.height > header_b.height
}

fn is_monotonic_height<'a>(header_a: &'a Header, header_b: &'a Header) -> impl Pred<Error> + 'a {
    pred::from_fn(move || _is_monotonic_height(&header_a, &header_b))
        .named("is_monotonic_height")
        .to_assert(|_| Error::NonIncreasingHeight)
}

fn _has_sufficient_voting_power(
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

fn has_sufficient_voting_power<'a>(
    commit: &'a Commit,
    validators: &'a ValidatorSet,
    trust_level: &'a TrustLevel,
    calculator: &'a impl VotingPowerCalculator,
) -> impl Pred<Error> + 'a {
    pred::from_fn(move || {
        _has_sufficient_voting_power(&commit, &validators, &trust_level, &calculator)
    })
    .named("has_sufficient_voting_power")
    .to_assert(|_| Error::InsufficientVotingPower)
}

fn _has_sufficient_validators_overlap(
    untrusted_commit: &Commit,
    trusted_validators: &ValidatorSet,
    trust_level: &TrustLevel,
    calculator: &impl VotingPowerCalculator,
) -> bool {
    _has_sufficient_voting_power(
        untrusted_commit,
        trusted_validators,
        trust_level,
        calculator,
    )
}

fn has_sufficient_validators_overlap<'a>(
    untrusted_commit: &'a Commit,
    trusted_validators: &'a ValidatorSet,
    trust_level: &'a TrustLevel,
    calculator: &'a impl VotingPowerCalculator,
) -> impl Pred<Error> + 'a {
    pred::from_fn(move || {
        _has_sufficient_validators_overlap(
            &untrusted_commit,
            &trusted_validators,
            &trust_level,
            &calculator,
        )
    })
    .named("has_sufficient_validators_overlap")
    .to_assert(|_| Error::InsufficientValidatorsOverlap)
}

fn _has_sufficient_signers_overlap(
    untrusted_commit: &Commit,
    untrusted_validators: &ValidatorSet,
    trust_level: &TrustLevel,
    calculator: &impl VotingPowerCalculator,
) -> bool {
    _has_sufficient_voting_power(
        untrusted_commit,
        untrusted_validators,
        trust_level,
        calculator,
    )
}

fn has_sufficient_signers_overlap<'a>(
    untrusted_commit: &'a Commit,
    untrusted_validators: &'a ValidatorSet,
    trust_level: &'a TrustLevel,
    calculator: &'a impl VotingPowerCalculator,
) -> impl Pred<Error> + 'a {
    pred::from_fn(move || {
        _has_sufficient_signers_overlap(
            &untrusted_commit,
            &untrusted_validators,
            &trust_level,
            &calculator,
        )
    })
    .named("has_sufficient_signers_overlap")
    .to_assert(|_| Error::InvalidCommit)
}

fn _invalid_next_validator_set(
    trusted_state: &TrustedState,
    untrusted_sh: &SignedHeader,
    untrusted_next_vals: &ValidatorSet,
) -> bool {
    untrusted_sh.header.height == trusted_state.header.height
        && trusted_state.validators.hash != untrusted_next_vals.hash
}

fn valid_next_validator_set<'a>(
    trusted_state: &'a TrustedState,
    untrusted_sh: &'a SignedHeader,
    untrusted_next_vals: &'a ValidatorSet,
) -> impl Pred<Error> + 'a {
    not(pred::from_fn(move || {
        _invalid_next_validator_set(&trusted_state, &untrusted_sh, &untrusted_next_vals)
    }))
    .named("valid_next_validator_set")
    .to_assert(|_| Error::InvalidNextValidatorSet)
}

fn verify_pred(
    validator_sets_match: impl Pred<Error>,
    next_validators_match: impl Pred<Error>,
    header_matches_commit: impl Pred<Error>,
    valid_commit: impl Pred<Error>,
    is_monotonic_bft_time: impl Pred<Error>,
    is_monotonic_height: impl Pred<Error>,
    valid_next_validator_set: impl Pred<Error>,
    has_sufficient_validators_overlap: impl Pred<Error>,
    has_sufficient_signers_overlap: impl Pred<Error>,
) -> impl Pred<Error> {
    validator_sets_match
        .and(next_validators_match)
        .and(header_matches_commit)
        .and(valid_commit)
        .and(is_monotonic_bft_time)
        .and(is_monotonic_height)
        .and(valid_next_validator_set)
        .and(has_sufficient_validators_overlap)
        .and(has_sufficient_signers_overlap)
}

fn main() {
    let now = SystemTime::now();

    let trusted_state = TrustedState {
        header: Header {
            height: 9,
            bft_time: now + Duration::from_secs(1),
            validator_set_hash: 34,
            next_validator_set_hash: 35,
            hash: 9,
        },
        validators: ValidatorSet { hash: 34 },
    };

    let untrusted_sh = SignedHeader {
        header: Header {
            height: 10,
            bft_time: now + Duration::from_secs(10),
            validator_set_hash: 99,
            next_validator_set_hash: 100,
            hash: 10,
        },
        commit: Commit { header_hash: 10 },
        validators: ValidatorSet { hash: 100 },
        validator_hash: 100,
    };

    let untrusted_vals = ValidatorSet { hash: 100 };
    let untrusted_next_vals = ValidatorSet { hash: 101 };

    let trust_level = TrustLevel {
        numerator: 1,
        denominator: 3,
    };

    #[derive(Copy, Clone)]
    struct MockCommitValidator;
    impl CommitValidator for MockCommitValidator {
        fn validate(&self, _: &Commit, _: &ValidatorSet) -> Result<(), Error> {
            Ok(())
        }
    }

    #[derive(Copy, Clone)]
    struct MockVotingPowerCalculator;
    impl VotingPowerCalculator for MockVotingPowerCalculator {
        fn voting_power_in(&self, _: &Commit, _: &ValidatorSet) -> Result<u64, Error> {
            Ok(31)
        }

        fn total_power_of(&self, _: &ValidatorSet) -> Result<u64, Error> {
            Ok(42)
        }
    }

    #[derive(Copy, Clone)]
    struct MockHeaderHasher;

    impl HeaderHasher for MockHeaderHasher {
        fn hash(&self, header: &Header) -> Hash {
            header.hash
        }
    }

    let p_validator_sets_match = validator_sets_match(&untrusted_sh, &untrusted_vals);
    let p_next_validators_match = next_validators_match(&untrusted_sh, &untrusted_next_vals);
    let p_header_matches_commit = header_matches_commit(
        &untrusted_sh.header,
        &untrusted_sh.commit,
        &MockHeaderHasher,
    );
    let p_valid_commit = valid_commit(
        &untrusted_sh.commit,
        &untrusted_sh.validators,
        &MockCommitValidator,
    );
    let p_is_monotonic_bft_time =
        is_monotonic_bft_time(&untrusted_sh.header, &trusted_state.header);
    let p_is_monotonic_height = is_monotonic_height(&trusted_state.header, &untrusted_sh.header);
    let p_valid_next_validator_set =
        valid_next_validator_set(&trusted_state, &untrusted_sh, &untrusted_next_vals);
    let p_has_sufficient_validators_overlap = has_sufficient_validators_overlap(
        &untrusted_sh.commit,
        &trusted_state.validators,
        &trust_level,
        &MockVotingPowerCalculator,
    );
    let p_has_sufficient_signers_overlap = has_sufficient_signers_overlap(
        &untrusted_sh.commit,
        &untrusted_vals,
        &trust_level,
        &MockVotingPowerCalculator,
    );

    let verify_pred = verify_pred(
        &p_validator_sets_match,
        &p_next_validators_match,
        &p_header_matches_commit,
        &p_valid_commit,
        &p_is_monotonic_bft_time,
        &p_is_monotonic_height,
        &p_valid_next_validator_set,
        &p_has_sufficient_validators_overlap,
        &p_has_sufficient_signers_overlap,
    );

    #[cfg(feature = "inspect-dot")]
    println!("{}", verify_pred.inspect().to_graph());

    #[cfg(feature = "inspect-text")]
    println!("{}", verify_pred.inspect());

    println!("Result: {}", verify_pred.eval());

    let result = verify_pred.assert();

    println!("Result: {}", result.is_ok());

    if verify_pred.eval() {
        assert!(result.is_ok());
    } else {
        assert!(result.is_err());
    }
}
