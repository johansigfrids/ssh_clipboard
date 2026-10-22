const STDERR_PART_BYTES: usize = 8192;

#[derive(Default)]
pub(super) struct StderrCapture {
    head: Vec<u8>,
    tail: Vec<u8>,
}

impl StderrCapture {
    pub(super) fn observe(&mut self, bytes: &[u8]) {
        let head_count = (STDERR_PART_BYTES - self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..head_count]);
        if bytes.len() >= STDERR_PART_BYTES {
            self.tail.clear();
            self.tail
                .extend_from_slice(&bytes[bytes.len() - STDERR_PART_BYTES..]);
        } else {
            let excess = (self.tail.len() + bytes.len()).saturating_sub(STDERR_PART_BYTES);
            self.tail.drain(..excess);
            self.tail.extend_from_slice(bytes);
        }
    }
    pub(super) fn classification_bytes(&self) -> Vec<u8> {
        let mut bytes = self.head.clone();
        bytes.push(b'\n');
        bytes.extend_from_slice(&self.tail);
        bytes
    }
}
