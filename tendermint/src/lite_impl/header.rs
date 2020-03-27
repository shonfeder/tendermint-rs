//! [`lite::Header`] implementation for [`block::Header`].

use crate::lite::Height;
use crate::Hash;
use crate::{block, lite, Time};

impl lite::Header for block::Header {
    type Time = Time;

    fn height(&self) -> Height {
        self.height.value()
    }

    fn bft_time(&self) -> Time {
        self.time
    }

    fn validators_hash(&self) -> Hash {
        self.validators_hash
    }

    fn next_validators_hash(&self) -> Hash {
        self.next_validators_hash
    }
}
