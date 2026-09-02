use anyhow::Result;

use crate::core::Frame;

pub trait ProtocolCodec: Send + Sync {
    fn encode_frame(&self, frame: &Frame) -> Result<Vec<Vec<u8>>>;
}
