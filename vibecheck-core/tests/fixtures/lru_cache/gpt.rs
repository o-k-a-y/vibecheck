// Here's a simple LRU Cache implementation in Rust.
// Let's use a HashMap combined with a VecDeque to track access order.

use std::collections::HashMap;
use std::collections::VecDeque;

// Here's the main struct that represents our LRU Cache.
pub struct LruCache<K, V> {
    capacity: usize,
    map: HashMap<K, V>,
    order: VecDeque<K>,
}

impl<K: Eq + std::hash::Hash + Clone, V> LruCache<K, V> {
    // Step 1: Initialize the cache with a given capacity.
    // Let's make sure the capacity is at least 1.
    pub fn new(capacity: usize) -> Self {
        // Here's where we construct the empty cache.
        let capacity = if capacity == 0 { 1 } else { capacity };
        LruCache {
            capacity,
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    // Step 2: Check if a key exists and return a reference to the value.
    // Let's also move the accessed key to the back of the queue.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        // Check if the key is present in the map.
        if self.map.contains_key(key) {
            // Step 2a: Remove the key from its current position in the order.
            self.order.retain(|k| k != key);
            // Step 2b: Push it to the back to mark it as recently used.
            self.order.push_back(key.clone());
            // Return the value from the map.
            return self.map.get(key);
        }
        // If the key is not found, return None.
        None
    }

    // Step 3: Insert a key-value pair into the cache.
    // Here's how we handle eviction when the cache is full.
    pub fn put(&mut self, key: K, value: V) {
        // Step 3a: If the key already exists, update it.
        if self.map.contains_key(&key) {
            // Remove old position from the order tracking.
            self.order.retain(|k| k != &key);
        } else if self.map.len() >= self.capacity {
            // Step 3b: Evict the least recently used item.
            // Let's pop the front of the deque, that's the LRU key.
            if let Some(evicted) = self.order.pop_front() {
                // Remove the evicted entry from the map.
                self.map.remove(&evicted);
            }
        }
        // Step 3c: Insert the new key-value pair.
        self.map.insert(key.clone(), value);
        // Mark it as the most recently used.
        self.order.push_back(key);
    }

    // Step 4: Return the current number of items in the cache.
    // Here's a simple helper method for checking the size.
    pub fn len(&self) -> usize {
        // Return the length of the internal map.
        self.map.len()
    }

    // Step 5: Check if the cache is empty.
    // Let's just compare the length to zero.
    pub fn is_empty(&self) -> bool {
        // Return true if there are no items stored.
        self.map.is_empty()
    }

    // Step 6: Clear all entries from the cache.
    // Here's how we reset everything back to the initial state.
    pub fn clear(&mut self) {
        // Remove all entries from the map.
        self.map.clear();
        // Remove all tracked keys from the order deque.
        self.order.clear();
    }
}
