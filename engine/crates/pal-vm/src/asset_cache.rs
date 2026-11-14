//! Content-addressed, byte-bounded LRU for decoded assets. Keys include the
//! complete input, so loose-file edits and archive overrides stay live.
use std::collections::HashMap;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub(crate) struct ContentCache<T> {
    entries: HashMap<[u8; 32], (T, usize, u64)>,
    bytes: usize,
    limit: usize,
    clock: u64,
}

impl<T: Clone> ContentCache<T> {
    pub(crate) fn new(desktop_limit: usize) -> Self {
        let limit = if cfg!(any(target_os = "vita", target_os = "horizon")) { 0 } else { desktop_limit };
        Self { entries: HashMap::new(), bytes: 0, limit, clock: 0 }
    }
    pub(crate) fn key(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }
    pub(crate) fn can_store(&self, bytes: usize) -> bool { self.limit != 0 && bytes <= self.limit }
    pub(crate) fn get(&mut self, key: &[u8; 32]) -> Option<T> {
        self.clock = self.clock.wrapping_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.2 = self.clock;
        Some(entry.0.clone())
    }
    pub(crate) fn insert(&mut self, key: [u8; 32], value: T, bytes: usize) {
        if bytes > self.limit || self.limit == 0 { return; }
        if let Some((_, old_bytes, _)) = self.entries.remove(&key) { self.bytes -= old_bytes; }
        while self.bytes + bytes > self.limit || self.entries.len() >= 256 {
            let Some(oldest) = self.entries.iter().min_by_key(|(_, entry)| entry.2).map(|(key, _)| *key) else { break; };
            self.bytes -= self.entries.remove(&oldest).unwrap().1;
        }
        self.clock = self.clock.wrapping_add(1);
        self.entries.insert(key, (value, bytes, self.clock));
        self.bytes += bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_evicts_by_bytes_and_recency_and_rejects_oversize_assets() {
        let mut cache = ContentCache::new(6);
        cache.insert([1; 32], vec![1; 3], 3);
        cache.insert([2; 32], vec![2; 3], 3);
        assert!(cache.get(&[1; 32]).is_some());
        cache.insert([3; 32], vec![3; 3], 3);
        assert!(cache.get(&[2; 32]).is_none());
        assert!(cache.get(&[1; 32]).is_some());
        cache.insert([4; 32], vec![4; 7], 7);
        assert!(cache.get(&[4; 32]).is_none());
        assert_eq!(cache.bytes, 6);
        assert_ne!(ContentCache::<()>::key(b"old"), ContentCache::<()>::key(b"new"));
    }
}
