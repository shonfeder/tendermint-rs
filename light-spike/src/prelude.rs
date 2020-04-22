// Some simplified types which only have the fields needed for core verification

pub use crate::{Handler, Input, Output};

pub use tendermint::hash::Hash;
pub type Height = u64;

use std::time::SystemTime;

use derive_more::Display;

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
    NotWithinTrustPeriod,
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
pub struct Header {
    pub height: Height,
    pub bft_time: SystemTime,
    pub validator_set_hash: Hash,
    pub next_validator_set_hash: Hash,
    pub hash: Hash, // TODO: What if we don't have this
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
pub struct ValidatorSet {
    pub hash: Hash,
}

impl From<std::vec::Vec<tendermint::validator::Info>> for ValidatorSet {
    fn from(vis: std::vec::Vec<tendermint::validator::Info>) -> Self {
        todo!()
    }
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
pub struct Commit {
    pub header_hash: Hash,
}

#[derive(Copy, Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
pub struct TrustThreshold {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
pub struct SignedHeader {
    pub header: Header,
    pub commit: Commit,
    pub validators: ValidatorSet,
    pub validators_hash: Hash,
}

impl From<tendermint::block::signed_header::SignedHeader> for SignedHeader {
    fn from(sh: tendermint::block::signed_header::SignedHeader) -> Self {
        todo!()
    }
}

#[derive(Clone, Debug, Display)]
#[display(fmt = "{:?}", self)]
pub struct TrustedState {
    pub header: Header,
    pub validators: ValidatorSet,
}

// Crypto function traits allowing mocking out during testing
pub trait VotingPowerCalculator: Send + Sync {
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

impl VotingPowerCalculator for Box<dyn VotingPowerCalculator> {
    fn voting_power_in(&self, commit: &Commit, validators: &ValidatorSet) -> Result<u64, Error> {
        self.as_ref().voting_power_in(commit, validators)
    }

    fn total_power_of(&self, validators: &ValidatorSet) -> Result<u64, Error> {
        self.as_ref().total_power_of(validators)
    }
}

pub trait CommitValidator: Send + Sync {
    fn validate(&self, commit: &Commit, validators: &ValidatorSet) -> Result<(), Error>;
}

impl<T: CommitValidator> CommitValidator for &T {
    fn validate(&self, commit: &Commit, validators: &ValidatorSet) -> Result<(), Error> {
        (*self).validate(commit, validators)
    }
}

impl CommitValidator for Box<dyn CommitValidator> {
    fn validate(&self, commit: &Commit, validators: &ValidatorSet) -> Result<(), Error> {
        self.as_ref().validate(commit, validators)
    }
}

pub trait HeaderHasher: Send + Sync {
    fn hash(&self, header: &Header) -> Hash; // Or Error?
}

impl<T: HeaderHasher> HeaderHasher for &T {
    fn hash(&self, header: &Header) -> Hash {
        (*self).hash(header)
    }
}

impl HeaderHasher for Box<dyn HeaderHasher> {
    fn hash(&self, header: &Header) -> Hash {
        self.as_ref().hash(header)
    }
}
