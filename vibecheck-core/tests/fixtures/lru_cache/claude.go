package lrucache

import "container/list"

// CacheEntry holds a single key-value pair stored within the cache.
// It is kept as the Value of each doubly-linked list element so that
// we can recover the key during eviction without a reverse lookup.
type CacheEntry struct {
	EntryKey   string
	EntryValue int
}

// LeastRecentlyUsedCache provides a fixed-capacity key-value store
// that automatically evicts the least recently accessed entry when
// the capacity is exceeded. It combines a doubly-linked list for
// recency tracking with a map for constant-time key lookups.
type LeastRecentlyUsedCache struct {
	maximumCapacity  int
	accessOrderList  *list.List
	entryLookupTable map[string]*list.Element
}

// NewLeastRecentlyUsedCache creates a cache with the given maximum
// capacity. Note that the capacity must be at least one; otherwise
// the cache would be unable to store any entries.
func NewLeastRecentlyUsedCache(maximumCapacity int) *LeastRecentlyUsedCache {
	return &LeastRecentlyUsedCache{
		maximumCapacity:  maximumCapacity,
		accessOrderList:  list.New(),
		entryLookupTable: make(map[string]*list.Element, maximumCapacity),
	}
}

// InsertEntry adds or updates a key-value pair in the cache. If the
// key already exists, its value is replaced and the entry is promoted
// to the most-recently-used position. If the cache is at capacity,
// the least recently used entry is evicted first. This ensures the
// cache never exceeds its configured bound.
func (cache *LeastRecentlyUsedCache) InsertEntry(cacheKey string, cacheValue int) {
	if existingElement, keyExists := cache.entryLookupTable[cacheKey]; keyExists {
		cache.accessOrderList.MoveToFront(existingElement)
		existingElement.Value.(*CacheEntry).EntryValue = cacheValue
		return
	}

	if len(cache.entryLookupTable) >= cache.maximumCapacity {
		cache.evictLeastRecentlyUsedEntry()
	}

	newEntry := &CacheEntry{EntryKey: cacheKey, EntryValue: cacheValue}
	insertedElement := cache.accessOrderList.PushFront(newEntry)
	cache.entryLookupTable[cacheKey] = insertedElement
}

// RetrieveEntry looks up the value for the given key. The boolean
// return value indicates whether the key was found. Accessing an
// entry promotes it to the most-recently-used position, which
// protects it from near-term eviction.
func (cache *LeastRecentlyUsedCache) RetrieveEntry(cacheKey string) (int, bool) {
	foundElement, keyExists := cache.entryLookupTable[cacheKey]
	if !keyExists {
		return 0, false
	}

	// Note that moving to front marks this entry as most recently used,
	// since we evict from the back of the list.
	cache.accessOrderList.MoveToFront(foundElement)
	return foundElement.Value.(*CacheEntry).EntryValue, true
}

// CurrentEntryCount returns how many entries are stored in the cache.
func (cache *LeastRecentlyUsedCache) CurrentEntryCount() int {
	return len(cache.entryLookupTable)
}

// evictLeastRecentlyUsedEntry removes the entry at the back of the
// access-order list. The back element is the least recently used
// because every access moves its element to the front.
func (cache *LeastRecentlyUsedCache) evictLeastRecentlyUsedEntry() {
	tailElement := cache.accessOrderList.Back()
	if tailElement == nil {
		return
	}

	evictedEntry := tailElement.Value.(*CacheEntry)
	cache.accessOrderList.Remove(tailElement)
	delete(cache.entryLookupTable, evictedEntry.EntryKey)
}
