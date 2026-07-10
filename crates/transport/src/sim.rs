//! In-memory multi-peer transport for tests (no radios).

use crate::frame::{decode_frame, Frame, FrameError};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct SimTransport {
    inbox: Arc<Mutex<VecDeque<Vec<u8>>>>,
    /// Shared bus: all peers' inboxes
    bus: Arc<Mutex<Vec<Arc<Mutex<VecDeque<Vec<u8>>>>>>>,
}

impl SimTransport {
    pub fn new_network(n: usize) -> Vec<Self> {
        let bus: Arc<Mutex<Vec<Arc<Mutex<VecDeque<Vec<u8>>>>>>> =
            Arc::new(Mutex::new(Vec::new()));
        let mut peers = Vec::new();
        for _ in 0..n {
            let inbox = Arc::new(Mutex::new(VecDeque::new()));
            bus.lock().unwrap().push(inbox.clone());
            peers.push(Self {
                inbox,
                bus: bus.clone(),
            });
        }
        peers
    }

    pub fn broadcast(&self, frame_bytes: &[u8]) {
        let peers = self.bus.lock().unwrap();
        for p in peers.iter() {
            // skip exact same Arc? deliver to all including self for simplicity of tests
            p.lock().unwrap().push_back(frame_bytes.to_vec());
        }
    }

    pub fn try_recv(&self) -> Option<Vec<u8>> {
        self.inbox.lock().unwrap().pop_front()
    }

    pub fn try_recv_frame(&self) -> Result<Option<Frame>, FrameError> {
        match self.try_recv() {
            None => Ok(None),
            Some(b) => Ok(Some(decode_frame(&b)?)),
        }
    }
}
