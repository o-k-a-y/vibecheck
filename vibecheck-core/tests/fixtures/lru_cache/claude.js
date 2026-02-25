/**
 * A least-recently-used cache backed by a JavaScript Map.
 *
 * The Map preserves insertion order, which allows us to treat the
 * first entry as the least recently used. On every access or update,
 * the touched entry is deleted and re-inserted at the end of the
 * iteration order, effectively promoting it to most-recently-used.
 */
class LeastRecentlyUsedCache {
  /**
   * Creates a new cache with the given maximum capacity.
   * @param {number} maximumCapacity - The upper bound on stored entries.
   */
  constructor(maximumCapacity) {
    if (maximumCapacity <= 0) {
      throw new RangeError("Cache capacity must be a positive integer");
    }
    this.maximumCapacity = maximumCapacity;
    this.orderedStorage = new Map();
  }

  /**
   * Inserts or updates a key-value pair in the cache.
   *
   * If the key already exists, its value is replaced and the entry is
   * promoted to the most-recently-used position. If the cache is full,
   * the least recently used entry is evicted first. This ensures the
   * cache never grows beyond its configured capacity.
   * @param {*} cacheKey - The key to store.
   * @param {*} cacheValue - The value associated with the key.
   */
  insertEntry(cacheKey, cacheValue) {
    if (this.orderedStorage.has(cacheKey)) {
      this.orderedStorage.delete(cacheKey);
      this.orderedStorage.set(cacheKey, cacheValue);
      return;
    }

    if (this.orderedStorage.size >= this.maximumCapacity) {
      this.evictLeastRecentlyUsedEntry();
    }

    this.orderedStorage.set(cacheKey, cacheValue);
  }

  /**
   * Retrieves the value for the given key, or undefined if absent.
   *
   * Accessing an entry promotes it to the most-recently-used position,
   * which protects it from near-term eviction.
   * @param {*} cacheKey - The key to look up.
   * @returns {*} The cached value, or undefined if not found.
   */
  retrieveEntry(cacheKey) {
    if (!this.orderedStorage.has(cacheKey)) {
      return undefined;
    }

    const cachedValue = this.orderedStorage.get(cacheKey);
    // Note that we must delete and re-insert to move the entry to the
    // end of the Map's iteration order, which represents most-recent.
    this.orderedStorage.delete(cacheKey);
    this.orderedStorage.set(cacheKey, cachedValue);
    return cachedValue;
  }

  /**
   * Returns the number of entries currently stored in the cache.
   * @returns {number} The current entry count.
   */
  currentEntryCount() {
    return this.orderedStorage.size;
  }

  /**
   * Removes the least recently used entry from the cache.
   *
   * The first key in the Map's iteration order is the oldest entry,
   * since every access moves its key to the end via delete-and-reinsert.
   */
  evictLeastRecentlyUsedEntry() {
    const leastRecentKey = this.orderedStorage.keys().next().value;
    this.orderedStorage.delete(leastRecentKey);
  }
}

// Verification that the cache behaves correctly
const demonstrationCache = new LeastRecentlyUsedCache(2);
demonstrationCache.insertEntry("first_key", 100);
demonstrationCache.insertEntry("second_key", 200);

// This access promotes "first_key" to most-recently-used
demonstrationCache.retrieveEntry("first_key");

demonstrationCache.insertEntry("third_key", 300);

// Note that "second_key" was evicted because it was the least recently used
console.assert(demonstrationCache.retrieveEntry("second_key") === undefined);
console.assert(demonstrationCache.retrieveEntry("first_key") === 100);
console.assert(demonstrationCache.currentEntryCount() === 2);

module.exports = { LeastRecentlyUsedCache };
