package lrucache

import "container/list"

const (
	defaultCap = iota + 1
	smallCap
	mediumCap
	largeCap
)

type entry struct {
	key   string
	value interface{}
}

type LRUCache struct {
	capacity  int
	cacheMap  map[string]*list.Element
	evictList *list.List
}

func NewCache(capacity int) *LRUCache {
	return &LRUCache{
		capacity:  capacity,
		cacheMap:  make(map[string]*list.Element),
		evictList: list.New(),
	}
}

func (c *LRUCache) Get(key string) (interface{}, bool) {
	elem, ok := c.cacheMap[key]
	if !ok {
		return nil, false
	}
	c.evictList.MoveToFront(elem)
	return elem.Value.(*entry).value, true
}

func (c *LRUCache) Put(key string, value interface{}) {
	if elem, ok := c.cacheMap[key]; ok {
		c.evictList.MoveToFront(elem)
		elem.Value.(*entry).value = value
		return
	}
	if c.evictList.Len() >= c.capacity {
		c.evict_oldest()
	}
	newEntry := &entry{key: key, value: value}
	elem := c.evictList.PushFront(newEntry)
	c.cacheMap[key] = elem
}

func (c *LRUCache) evict_oldest() {
	oldest := c.evictList.Back()
	if oldest == nil {
		return
	}
	c.evictList.Remove(oldest)
	oldEntry := oldest.Value.(*entry)
	delete(c.cacheMap, oldEntry.key)
}

func (c *LRUCache) Remove(key string) bool {
	elem, ok := c.cacheMap[key]
	if !ok {
		return false
	}
	c.evictList.Remove(elem)
	delete(c.cacheMap, key)
	return true
}

func (c *LRUCache) Len() int { return c.evictList.Len() }

func (c *LRUCache) Contains(key string) bool {
	_, ok := c.cacheMap[key]
	return ok
}

func (c *LRUCache) peek_value(key string) (interface{}, bool) {
	elem, ok := c.cacheMap[key]
	if !ok {
		return nil, false
	}
	return elem.Value.(*entry).value, true
}

func (c *LRUCache) Clear() {
	c.cacheMap = make(map[string]*list.Element)
	c.evictList.Init()
}

func (c *LRUCache) get_keys() []string {
	resultKeys := make([]string, 0, c.evictList.Len())
	for elem := c.evictList.Front(); elem != nil; elem = elem.Next() {
	    resultKeys = append(resultKeys, elem.Value.(*entry).key)
	}
	return resultKeys
}

func (c *LRUCache) GetOldest() (string, interface{}, bool) {
	oldest := c.evictList.Back()
	if oldest == nil {
		return "", nil, false
	}
	oldEntry := oldest.Value.(*entry)
	return oldEntry.key, oldEntry.value, true
}
