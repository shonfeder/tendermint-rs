#![allow(unreachable_code, unused_variables)]

use light_spike::{
    light_client::LightClient, prelude::*, requester::Requester, scheduler::Scheduler,
    trusted_store::TrustedStore, verifier::Verifier,
};

fn main() {
    let trusted_store = TrustedStore::new();
    let (store_reader, store_writer) = trusted_store.split();

    let voting_power_calculator: Box<dyn VotingPowerCalculator> = todo!();
    let commit_validator: Box<dyn CommitValidator> = todo!();
    let header_hasher: Box<dyn HeaderHasher> = todo!();
    let rpc_client: tendermint::rpc::Client = todo!();

    let verifier = Verifier::new(voting_power_calculator, commit_validator, header_hasher);
    let requester = Requester::new(rpc_client);
    let light_client = LightClient::new(store_writer);

    let mut scheduler = Scheduler::new(light_client, verifier, requester);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let internal_sender = sender.clone();

    std::thread::spawn(|| scheduler.run(internal_sender, receiver));

    sender.send(Event::Tick).unwrap();
    sender.send(Event::Terminate).unwrap();
}
