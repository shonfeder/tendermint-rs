use crate::prelude::*;

use anomaly::BoxError;

use tendermint::block::CommitSig;
use tendermint::lite::types::ValidatorSet as _;
use tendermint::vote::{SignedVote, Vote};

pub trait VotingPowerCalculator {
    fn total_power_of(&self, validators: &ValidatorSet) -> u64;
    fn voting_power_in(
        &self,
        signed_header: &SignedHeader,
        validators: &ValidatorSet,
    ) -> Result<u64, BoxError>;
}

impl<T: VotingPowerCalculator> VotingPowerCalculator for &T {
    fn total_power_of(&self, validators: &ValidatorSet) -> u64 {
        (*self).total_power_of(validators)
    }

    fn voting_power_in(
        &self,
        signed_header: &SignedHeader,
        validators: &ValidatorSet,
    ) -> Result<u64, BoxError> {
        (*self).voting_power_in(signed_header, validators)
    }
}

impl VotingPowerCalculator for Box<dyn VotingPowerCalculator> {
    fn total_power_of(&self, validators: &ValidatorSet) -> u64 {
        self.as_ref().total_power_of(validators)
    }

    fn voting_power_in(
        &self,
        signed_header: &SignedHeader,
        validators: &ValidatorSet,
    ) -> Result<u64, BoxError> {
        self.as_ref().voting_power_in(signed_header, validators)
    }
}

pub struct ProdVotingPowerCalculator;

impl VotingPowerCalculator for ProdVotingPowerCalculator {
    fn total_power_of(&self, validators: &ValidatorSet) -> u64 {
        validators.total_power()
    }

    fn voting_power_in(
        &self,
        signed_header: &SignedHeader,
        validator_set: &ValidatorSet,
    ) -> Result<u64, BoxError> {
        let signatures = &signed_header.commit.signatures;
        let validators = validator_set.validators();

        // ensure!(
        //     validators.len() == signatures.len(),
        //     // TODO: Raise error
        // );

        // NOTE: We don't know the validators that committed this block,
        //       so we have to check for each vote if its validator is already known.
        let voting_power_needed = self.total_power_of(validator_set) * 2 / 3;
        let mut tallied_voting_power = 0_u64;

        for (idx, signature) in signatures.into_iter().enumerate() {
            if signature.is_absent() {
                continue; // OK, some signatures can be absent.
            }

            // The vals and commit have a 1-to-1 correspondance (see check above).
            // This means we don't need the validator address or to do any lookup.
            let val = validators[idx];

            let vote = vote_from_non_absent_signature(signature, idx as u64, &signed_header.commit)
                .unwrap(); // SAFETY: Safe because of `is_absent()` check above.

            let signed_vote = SignedVote::new(
                (&vote).into(),
                signed_header.header.chain_id.as_str(),
                vote.validator_address,
                vote.signature,
            );

            // Check vote is valid from validator
            let sign_bytes = signed_vote.sign_bytes();
            if !val.verify_signature(&sign_bytes, signed_vote.signature()) {
                bail!(VerificationError::ImplementationSpecific(format!(
                    "Couldn't verify signature {:?} with validator {:?} on sign_bytes {:?}",
                    signed_vote.signature(),
                    val,
                    sign_bytes,
                )));
            }

            if signature.is_commit() {
                tallied_voting_power += val.power();
            } else {
                // It's OK. We include stray signatures (~votes for nil) to measure
                // validator availability.
            }

            if tallied_voting_power >= voting_power_needed {
                break;
            }
        }

        Ok(tallied_voting_power)
    }
}

fn vote_from_non_absent_signature(
    commit_sig: &CommitSig,
    validator_index: u64,
    commit: &Commit,
) -> Option<Vote> {
    let (validator_address, timestamp, signature, block_id) = match commit_sig {
        CommitSig::BlockIDFlagAbsent { .. } => return None,
        CommitSig::BlockIDFlagCommit {
            validator_address,
            timestamp,
            signature,
        } => (
            validator_address.clone(),
            timestamp.clone(),
            signature.clone(),
            Some(commit.block_id.clone()),
        ),
        CommitSig::BlockIDFlagNil {
            validator_address,
            timestamp,
            signature,
        } => (
            validator_address.clone(),
            timestamp.clone(),
            signature.clone(),
            None,
        ),
    };

    Some(Vote {
        vote_type: tendermint::vote::Type::Precommit,
        height: commit.height,
        round: commit.round,
        block_id,
        timestamp,
        validator_address,
        validator_index,
        signature,
    })
}
