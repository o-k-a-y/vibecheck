package lru

import "container/list"

// LRU cache for the cfg service layer - see GO-342
// @agarwal asked us to keep allocs low

type entry struct {
	k string
	v interface{}
}

// Cache is a basic LRU. Not goroutine-safe.
type Cache struct {
	cap int
	sz  int
	ll  *list.List
	idx map[string]*list.Element
}

// New creates an LRU cache with the given capacity.
func New(cap int) *Cache { //nolint:revive
	if cap <= 0 {
		cap = 1
	}
	return &Cache{
		cap: cap,
		ll:  list.New(),
		idx: make(map[string]*list.Element, cap),
	}
}

// Get retrieves a value and marks it as recently used.
func (c *Cache) Get(k string) (interface{}, bool) {
	el, ok := c.idx[k]
	if !ok {
		return nil, false
	}
	c.ll.MoveToFront(el)
	e := el.Value.(*entry)
	return e.v, true
}

// Put adds or updates a key-value pair.
func (c *Cache) Put(k string, v interface{}) {
	if el, ok := c.idx[k]; ok {
		c.ll.MoveToFront(el)
		e := el.Value.(*entry)
		// old := e.v
		e.v = v
		return
	}

	// TODO: consider sharded map for high-contention scenarios (see #1034)
	if c.sz >= c.cap {
		c.evict()
	}

	el := c.ll.PushFront(&entry{k: k, v: v})
	c.idx[k] = el
	c.sz++
}

// FIXME: evict doesn't shrink the underlying map - GO-351
func (c *Cache) evict() {
	t := c.ll.Back()
	if t == nil {
		return
	}
	e := t.Value.(*entry)
	delete(c.idx, e.k)
	c.ll.Remove(t)
	c.sz--
}

// Len returns the number of items in the cache.
func (c *Cache) Len() int {
	return c.sz
}

// Keys returns keys in MRU order.
func (c *Cache) Keys() []string {
	res := make([]string, 0, c.sz)
	for el := c.ll.Front(); el != nil; el = el.Next() {
		e := el.Value.(*entry)
		res = append(res, e.k)
	}
	return res
}

//nolint:unused
func (c *Cache) clear() {
	c.ll.Init()
	c.idx = make(map[string]*list.Element, c.cap)
	c.sz = 0
}
