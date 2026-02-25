use std::collections::{HashMap, VecDeque};

/// A least-recently-used cache with a fixed maximum capacity.
///
/// This implementation uses a `HashMap` for constant-time key lookups
/// and a `VecDeque` to track access recency. When the cache exceeds
/// its maximum capacity, the least recently accessed entry is evicted.
pub struct LeastRecentlyUsedCache<KeyType, ValueType>
where
    KeyType: Eq + std::hash::Hash + Clone,
{
    stored_entries: HashMap<KeyType, ValueType>,
    access_order_tracker: VecDeque<KeyType>,
    maximum_capacity: usize,
}

impl<KeyType, ValueType> LeastRecentlyUsedCache<KeyType, ValueType>
where
    KeyType: Eq + std::hash::Hash + Clone,
{
    /// Creates a new cache that holds at most `maximum_capacity` entries.
    ///
    /// Note that a capacity of zero is allowed but means every insertion
    /// will immediately evict the entry.
    pub fn with_capacity(maximum_capacity: usize) -> Self {
        Self {
            stored_entries: HashMap::with_capacity(maximum_capacity),
            access_order_tracker: VecDeque::with_capacity(maximum_capacity),
            maximum_capacity,
        }
    }

    /// Inserts a key-value pair into the cache.
    ///
    /// If the key already exists, its value is updated and it is promoted
    /// to the most-recently-used position. If inserting a new key would
    /// exceed the maximum capacity, the least recently used entry is
    /// evicted first. This ensures the cache never grows beyond its bound.
    pub fn insert_entry(&mut self, cache_key: KeyType, cache_value: ValueType) {
        if self.stored_entries.contains_key(&cache_key) {
            self.promote_key_to_most_recent(&cache_key);
            self.stored_entries.insert(cache_key, cache_value);
            return;
        }

        if self.stored_entries.len() >= self.maximum_capacity {
            self.evict_least_recently_used_entry();
        }

        self.access_order_tracker.push_back(cache_key.clone());
        self.stored_entries.insert(cache_key, cache_value);
    }

    /// Retrieves a reference to the value associated with the given key.
    ///
    /// Accessing an entry promotes it to the most-recently-used position,
    /// which protects it from near-term eviction.
    pub fn retrieve_entry(&mut self, cache_key: &KeyType) -> Option<&ValueType> {
        if self.stored_entries.contains_key(cache_key) {
            self.promote_key_to_most_recent(cache_key);
            return self.stored_entries.get(cache_key);
        }
        None
    }

    /// Returns the number of entries currently stored in the cache.
    pub fn current_entry_count(&self) -> usize {
        self.stored_entries.len()
    }

    /// Moves the given key to the back of the access-order tracker,
    /// marking it as the most recently used entry. This ensures that
    /// frequently accessed entries survive eviction longer.
    fn promote_key_to_most_recent(&mut self, cache_key: &KeyType) {
        self.access_order_tracker
            .retain(|existing_key| existing_key != cache_key);
        self.access_order_tracker.push_back(cache_key.clone());
    }

    /// Removes the least recently used entry from both the access-order
    /// tracker and the underlying storage map.
    fn evict_least_recently_used_entry(&mut self) {
        if let Some(evicted_key) = self.access_order_tracker.pop_front() {
            self.stored_entries.remove(&evicted_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used_when_capacity_exceeded() {
        let mut cache = LeastRecentlyUsedCache::with_capacity(2);
        cache.insert_entry("first_key", 100);
        cache.insert_entry("second_key", 200);
        cache.insert_entry("third_key", 300);

        // Note that "first_key" was evicted because it was the oldest entry
        assert!(cache.retrieve_entry(&"first_key").is_none());
        assert_eq!(cache.retrieve_entry(&"second_key"), Some(&200));
        assert_eq!(cache.retrieve_entry(&"third_key"), Some(&300));
    }
}
