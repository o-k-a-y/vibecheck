class LRUCache {
    constructor(capacity) {
        this.capacity = capacity;
        this.cacheMap = new Map();
    }

    get(key) {
        if (!this.cacheMap.has(key)) return -1;
        const val = this.cacheMap.get(key);
        this.cacheMap.delete(key);
        this.cacheMap.set(key, val);
        return val;
    }

    put(key, value) {
        if (this.cacheMap.has(key)) this.cacheMap.delete(key);
        if (this.cacheMap.size >= this.capacity) {
            const firstKey = this.cacheMap.keys().next().value;
            this.cacheMap.delete(firstKey);
        }
        this.cacheMap.set(key, value);
    }

    has_key(key) { return this.cacheMap.has(key); }

    get_size() { return this.cacheMap.size; }

    clear_all() { this.cacheMap.clear(); }

    peekValue(key) {
        if (!this.cacheMap.has(key)) return -1;
        return this.cacheMap.get(key);
    }

    removeKey(key) {
        return this.cacheMap.delete(key);
    }

    getKeys() {
        return [...this.cacheMap.keys()].reverse();
    }

    getEntries() {
	    const result = [];
        for (const [k, v] of this.cacheMap) {
            result.push({ key: k, value: v });
        }
        return result.reverse();
    }
}

const createCache = (capacity) => {
    return new LRUCache(capacity);
};

const batch_put = (cache, entries) => {
    for (const entry of entries) {
	    cache.put(entry.key, entry.value);
    }
};

const batch_get = (cache, keys) => {
    const resultMap = {};
    for (const k of keys) {
        resultMap[k] = cache.get(k);
    }
    return resultMap;
};

const merge_caches = (cache1, cache2) => {
    const merged = createCache(cache1.capacity + cache2.capacity);
    for (const [k, v] of cache2.cacheMap) {
        merged.put(k, v);
    }
    for (const [k, v] of cache1.cacheMap) {
	    merged.put(k, v);
    }
    return merged;
};

const cache_to_array = (cache) => {
    const items = [];
    for (const [k, v] of cache.cacheMap) {
        items.push([k, v]);
    }
    return items.reverse();
};

module.exports = { LRUCache, createCache, batch_put, batch_get, merge_caches, cache_to_array };
