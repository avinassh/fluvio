use std::fmt;

use fluvio_controlplane_metadata::partition::Replica;
use dataplane::Offset;
use fluvio_types::SpuId;

#[derive(Debug)]
pub enum LeaderReplicaControllerCommand {
    UpdateReplicaFromSc(Replica),
    RemoveReplicaFromSc,
}

#[derive(Debug)]
pub struct FollowerOffsetUpdate {
    pub follower_id: SpuId,
    pub leo: Offset, // log end offset
    pub hw: Offset,  // high water mark
}

impl FollowerOffsetUpdate {
    #[allow(dead_code)]
    pub fn new(follower_id: SpuId, leo: Offset, hw: Offset) -> Self {
        assert!(hw <= leo, "high watermark is always less than end offset");
        Self {
            follower_id,
            leo,
            hw,
        }
    }
}

impl From<(SpuId, Offset, Offset)> for FollowerOffsetUpdate {
    fn from(value: (SpuId, Offset, Offset)) -> Self {
        FollowerOffsetUpdate {
            follower_id: value.0,
            leo: value.1,
            hw: value.2,
        }
    }
}

impl fmt::Display for FollowerOffsetUpdate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "follower: {}, leo: {}, hw: {}",
            self.follower_id, self.leo, self.hw
        )
    }
}
