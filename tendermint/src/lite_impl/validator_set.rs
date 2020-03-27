//! [`lite::ValidatorSet`] implementation for [`validator::Set`].

use crate::validator;
use crate::{lite, merkle, Hash};

impl From<validator::Set> for lite::LightValidatorSet {
    fn from(vals: validator::Set) -> Self {
        Self {
            hash: hash(&vals),
            total_power: total_power(&vals),
        }
    }
}

/// Compute the Merkle root of the validator set
fn hash(vals: &validator::Set) -> Hash {
    let validator_bytes: Vec<Vec<u8>> = vals
        .validators()
        .iter()
        .map(|validator| validator.hash_bytes())
        .collect();
    Hash::Sha256(merkle::simple_hash_from_byte_vectors(validator_bytes))
}

fn total_power(vals: &validator::Set) -> u64 {
    vals.validators().iter().fold(0u64, |total, val_info| {
        total + val_info.voting_power.value()
    })
}
