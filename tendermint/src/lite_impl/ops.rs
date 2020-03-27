//! Concrete implementation of light client operations

use anomaly::fail;

use crate::amino_types::{message::AminoMessage, BlockId, ConsensusVersion, TimeMsg};
use crate::lite::error::{Error, Kind};
use crate::lite::{Commit, LightOperations, ValidatorSet};
use crate::merkle::simple_hash_from_byte_vectors;
use crate::{block, validator, Hash};

/// Implementation of light client operations
pub struct LightImplOps;

impl LightOperations<block::signed_header::SignedHeader, block::Header> for LightImplOps {
    // TODO: Move into CommitOps trait
    fn voting_power_in(
        &self,
        signed_header: &block::signed_header::SignedHeader,
        validators: &validator::Set,
    ) -> Result<u64, Error> {
        // NOTE we don't know the validators that committed this block,
        // so we have to check for each vote if its validator is already known.
        let mut signed_power = 0u64;
        for vote_opt in &signed_header.iter() {
            // skip absent and nil votes
            // NOTE: do we want to check the validity of votes
            // for nil ?
            // TODO: clarify this!
            let vote = match vote_opt {
                Some(v) => v,
                None => continue,
            };

            // check if this vote is from a known validator
            let val_id = vote.validator_id();
            let val = match validators.validator(val_id) {
                Some(v) => v,
                None => continue,
            };

            // check vote is valid from validator
            let sign_bytes = vote.sign_bytes();

            if !val.verify_signature(&sign_bytes, vote.signature()) {
                fail!(
                    Kind::ImplementationSpecific,
                    "Couldn't verify signature {:?} with validator {:?} on sign_bytes {:?}",
                    vote.signature(),
                    val,
                    sign_bytes,
                );
            }
            signed_power += val.power();
        }

        Ok(signed_power)
    }

    // Move into CommitOps trait
    fn validate(
        &self,
        signed_header: &block::signed_header::SignedHeader,
        vals: &validator::Set,
    ) -> Result<(), Error> {
        if signed_header.commit.precommits.len() != vals.validators().len() {
            fail!(
                Kind::ImplementationSpecific,
                "pre-commit length: {} doesn't match validator length: {}",
                signed_header.commit.precommits.len(),
                vals.validators().len()
            );
        }

        for precommit_opt in signed_header.commit.precommits.iter() {
            match precommit_opt {
                Some(precommit) => {
                    // make sure each vote is for the correct header
                    if let Some(header_hash) = precommit.header_hash() {
                        if header_hash != signed_header.header_hash() {
                            fail!(
                                Kind::ImplementationSpecific,
                                "validator({}) voted for header {}, but current header is {}",
                                precommit.validator_address,
                                header_hash,
                                signed_header.header_hash()
                            );
                        }
                    }

                    // returns FaultyFullNode error if it detects a signer isn't present in the validator set
                    if vals.validator(precommit.validator_address) == None {
                        let reason = format!(
                            "Found a faulty signer ({}) not present in the validator set ({})",
                            precommit.validator_address,
                            vals.hash()
                        );
                        fail!(Kind::FaultyFullNode, reason);
                    }
                }
                None => (),
            }
        }

        Ok(())
    }

    // TODO: Move into HeaderOps trait
    fn hash(&self, header: &block::Header) -> Hash {
        // Note that if there is an encoding problem this will
        // panic (as the golang code would):
        // https://github.com/tendermint/tendermint/blob/134fe2896275bb926b49743c1e25493f6b24cc31/types/block.go#L393
        // https://github.com/tendermint/tendermint/blob/134fe2896275bb926b49743c1e25493f6b24cc31/types/encoding_helper.go#L9:6

        let mut fields_bytes: Vec<Vec<u8>> = Vec::with_capacity(16);
        fields_bytes.push(AminoMessage::bytes_vec(&ConsensusVersion::from(
            &header.version,
        )));
        fields_bytes.push(bytes_enc(header.chain_id.as_bytes()));
        fields_bytes.push(encode_varint(header.height.value()));
        fields_bytes.push(AminoMessage::bytes_vec(&TimeMsg::from(header.time)));
        fields_bytes.push(encode_varint(header.num_txs));
        fields_bytes.push(encode_varint(header.total_txs));
        fields_bytes.push(
            header
                .last_block_id
                .as_ref()
                .map_or(vec![], |id| AminoMessage::bytes_vec(&BlockId::from(id))),
        );
        fields_bytes.push(header.last_commit_hash.as_ref().map_or(vec![], encode_hash));
        fields_bytes.push(header.data_hash.as_ref().map_or(vec![], encode_hash));
        fields_bytes.push(encode_hash(&header.validators_hash));
        fields_bytes.push(encode_hash(&header.next_validators_hash));
        fields_bytes.push(encode_hash(&header.consensus_hash));
        fields_bytes.push(bytes_enc(&header.app_hash));
        fields_bytes.push(
            header
                .last_results_hash
                .as_ref()
                .map_or(vec![], encode_hash),
        );
        fields_bytes.push(header.evidence_hash.as_ref().map_or(vec![], encode_hash));
        fields_bytes.push(bytes_enc(header.proposer_address.as_bytes()));

        Hash::Sha256(simple_hash_from_byte_vectors(fields_bytes))
    }
}

fn bytes_enc(bytes: &[u8]) -> Vec<u8> {
    let mut chain_id_enc = vec![];
    prost_amino::encode_length_delimiter(bytes.len(), &mut chain_id_enc).unwrap();
    chain_id_enc.append(&mut bytes.to_vec());
    chain_id_enc
}

fn encode_hash(hash: &Hash) -> Vec<u8> {
    bytes_enc(hash.as_bytes())
}

fn encode_varint(val: u64) -> Vec<u8> {
    let mut val_enc = vec![];
    prost_amino::encoding::encode_varint(val, &mut val_enc);
    val_enc
}
