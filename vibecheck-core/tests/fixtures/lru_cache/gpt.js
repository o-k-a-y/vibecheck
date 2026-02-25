// Here's a simple LRU Cache implementation in JavaScript.
// Let's use a Map since it preserves insertion order.

class LruCache {
  // Step 1: Initialize the cache with a given capacity.
  // Here's the constructor that sets up the internal storage.
  constructor(capacity) {
    // Store the maximum capacity for the cache.
    this.capacity = Math.max(1, capacity);
    // Let's use a Map to hold our key-value pairs.
    this.cache = new Map();
  }

  // Step 2: Retrieve the value associated with a key.
  // Here's how we handle the lookup and reordering.
  get(key) {
    // Check if the key exists in the cache.
    if (!this.cache.has(key)) {
      // Return -1 to signal a cache miss.
      return -1;
    }
    // Step 2a: Get the current value before reordering.
    const value = this.cache.get(key);
    // Step 2b: Delete and re-insert to move it to the end.
    // Let's make sure this key is now the most recently used.
    this.cache.delete(key);
    this.cache.set(key, value);
    // Step 2c: Return the retrieved value.
    return value;
  }

  // Step 3: Insert or update a key-value pair in the cache.
  // Here's how we manage capacity and eviction logic.
  put(key, value) {
    // Step 3a: If the key already exists, remove it first.
    if (this.cache.has(key)) {
      // Delete the old entry so we can re-insert at the end.
      this.cache.delete(key);
    } else if (this.cache.size >= this.capacity) {
      // Step 3b: Evict the least recently used entry.
      // Here's where we grab the first key from the map.
      const firstKey = this.cache.keys().next().value;
      // Remove the LRU entry from the cache.
      this.cache.delete(firstKey);
    }
    // Step 3c: Insert the new key-value pair at the end.
    this.cache.set(key, value);
  }

  // Step 4: Return the current number of items stored.
  // Let's expose the size of the internal map.
  size() {
    // Return the count of entries in the cache.
    return this.cache.size;
  }

  // Step 5: Check if a key is present in the cache.
  // Here's a simple existence check without reordering.
  has(key) {
    // Return true if the map contains the key.
    return this.cache.has(key);
  }

  // Step 6: Remove all entries from the cache.
  // Here's how we reset the cache to an empty state.
  clear() {
    // Clear the underlying map completely.
    this.cache.clear();
  }

  // Step 7: Return all keys in order from least to most recent.
  // Let's convert the map keys into a regular array.
  keys() {
    // Spread the keys iterator into an array.
    return [...this.cache.keys()];
  }

  // Step 8: Provide a readable string of the cache contents.
  // Here's a helper for debugging and logging purposes.
  toString() {
    // Build a string from all cache entries.
    const entries = [];
    for (const [k, v] of this.cache) {
      entries.push(`${k}: ${v}`);
    }
    return `LruCache { ${entries.join(", ")} }`;
  }
}

// Let's export the class so it can be used elsewhere.
module.exports = { LruCache };
