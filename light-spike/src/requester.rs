use tendermint::{block, rpc};

use crate::prelude::*;
use std::future::Future;

pub enum RequesterError {
    RpcError(rpc::Error),
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

pub struct Requester {
    rpc_client: rpc::Client,
}

impl Requester {
    pub fn new(rpc_client: rpc::Client) -> Self {
        Self { rpc_client }
    }

    pub fn fetch_signed_header(&self, h: Height) -> Result<SignedHeader, RequesterError> {
        let height: block::Height = h.into();

        let res = block_on(async {
            match height.value() {
                0 => self.rpc_client.latest_commit().await,
                _ => self.rpc_client.commit(height).await,
            }
        });

        match res {
            Ok(response) => Ok(response.signed_header.into()),
            Err(err) => Err(RequesterError::RpcError(err)),
        }
    }

    pub fn fetch_validator_set(&self, h: Height) -> Result<ValidatorSet, RequesterError> {
        let height: block::Height = h.into();

        let res = block_on(self.rpc_client.validators(h));

        match res {
            Ok(response) => Ok(response.validators.into()),
            Err(err) => Err(RequesterError::RpcError(err)),
        }
    }
}

impl Handler<RequesterEvent> for Requester {
    type Error = RequesterError;

    fn handle(&mut self, event: RequesterEvent) -> Result<RequesterEvent, RequesterError> {
        use RequesterEvent::*;

        match event {
            FetchSignedHeader(height) => {
                let signed_header = self.fetch_signed_header(height)?;
                Ok(RequesterEvent::SignedHeader(height, signed_header))
            }
            FetchValidatorSet(height) => {
                let validator_set = self.fetch_validator_set(height)?;
                Ok(RequesterEvent::ValidatorSet(height, validator_set))
            }
            FetchState(height) => {
                let signed_header = self.fetch_signed_header(height)?;
                let validator_set = self.fetch_validator_set(height)?;
                let next_validator_set = self.fetch_validator_set(height + 1)?;

                Ok(RequesterEvent::FetchedState {
                    height,
                    signed_header,
                    validator_set,
                    next_validator_set,
                })
            }
            _ => unreachable!(),
        }
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
