// Here's a simple LRU Cache implementation in Go.
// Let's use a doubly linked list combined with a hash map.

package lrucache

import "container/list"

// entry represents a single key-value pair stored in the cache.
// Here's the struct we'll place inside each linked list node.
type entry struct {
	key   string
	value int
}

// LruCache is the main cache structure.
// Let's keep the capacity, a map for lookups, and a list for ordering.
type LruCache struct {
	capacity int
	items    map[string]*list.Element
	order    *list.List
}

// Step 1: Initialize a new LRU Cache with the specified capacity.
// Here's the constructor function that sets everything up.
func NewLruCache(capacity int) *LruCache {
	// Make sure the capacity is at least 1.
	if capacity < 1 {
		capacity = 1
	}
	// Return a pointer to the newly created cache.
	return &LruCache{
		capacity: capacity,
		items:    make(map[string]*list.Element),
		order:    list.New(),
	}
}

// Step 2: Retrieve the value for a given key.
// Here's how we look up and reorder on access.
func (c *LruCache) Get(key string) (int, bool) {
	// Check if the key exists in the map.
	elem, found := c.items[key]
	if !found {
		// Return zero value and false to indicate a miss.
		return 0, false
	}
	// Step 2a: Move the accessed element to the front of the list.
	// Let's mark it as the most recently used item.
	c.order.MoveToFront(elem)
	// Step 2b: Return the value stored in this element.
	return elem.Value.(*entry).value, true
}

// Step 3: Insert or update a key-value pair in the cache.
// Here's how we handle insertion and eviction logic.
func (c *LruCache) Put(key string, value int) {
	// Step 3a: If the key already exists, update and move to front.
	if elem, found := c.items[key]; found {
		// Move the element to the front of the list.
		c.order.MoveToFront(elem)
		// Update the stored value.
		elem.Value.(*entry).value = value
		return
	}
	// Step 3b: Evict the least recently used item if at capacity.
	if c.order.Len() >= c.capacity {
		// Here's where we remove the back element (the LRU item).
		back := c.order.Back()
		if back != nil {
			// Remove from the linked list.
			c.order.Remove(back)
			// Remove from the hash map as well.
			evicted := back.Value.(*entry)
			delete(c.items, evicted.key)
		}
	}
	// Step 3c: Insert the new entry at the front of the list.
	// Let's create a new entry and push it to the front.
	newEntry := &entry{key: key, value: value}
	elem := c.order.PushFront(newEntry)
	// Store the list element reference in the map.
	c.items[key] = elem
}

// Step 4: Return the current number of items in the cache.
// Here's a simple getter for the cache size.
func (c *LruCache) Len() int {
	// Return the length of the internal map.
	return len(c.items)
}

// Step 5: Remove all entries from the cache.
// Let's reset both the map and the linked list.
func (c *LruCache) Clear() {
	// Re-initialize the map to an empty state.
	c.items = make(map[string]*list.Element)
	// Re-initialize the linked list.
	c.order.Init()
}
