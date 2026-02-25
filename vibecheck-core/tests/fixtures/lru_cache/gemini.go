package cache

import "sync"

// LRU Cache
// - O(1) get and put with map + linked list
// - mutex for concurrent safety
// - defer unlock pattern throughout

type Node struct {
	key   int
	value int
	prev  *Node
	next  *Node
}

type Cache struct {
	items map[int]*Node
	head  *Node
	tail  *Node
	cap   int
	mu    sync.Mutex
}

func New(cap int) *Cache {
	head := &Node{}
	tail := &Node{}
	head.next = tail
	tail.prev = head
	return &Cache{
		items: make(map[int]*Node),
		head:  head,
		tail:  tail,
		cap:   cap,
	}
}

func (c *Cache) Get(key int) (int, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()

	node, found := c.items[key]
	// - return zero value on miss
	// - move to front on hit
	// - return stored value
	if !found {
		return 0, false
	}
	c.detach(node)
	c.attach(node)
	return node.value, true
}

func (c *Cache) Put(key int, value int) {
	c.mu.Lock()
	defer c.mu.Unlock()

	// - update existing entry if present
	// - evict oldest when at capacity
	// - attach new entry at head
	if node, ok := c.items[key]; ok {
		node.value = value
		c.detach(node)
		c.attach(node)
		return
	}

	full := len(c.items) >= c.cap
	if full {
		old := c.tail.prev
		c.detach(old)
		delete(c.items, old.key)
	}

	node := &Node{key: key, value: value}
	c.items[key] = node
	c.attach(node)
}

func (c *Cache) detach(node *Node) {
	prev := node.prev
	next := node.next
	prev.next = next
	next.prev = prev
}

func (c *Cache) attach(node *Node) {
	first := c.head.next
	c.head.next = node
	node.prev = c.head
	node.next = first
	first.prev = node
}

func (c *Cache) Len() int {
	c.mu.Lock()
	defer c.mu.Unlock()
	return len(c.items)
}

func (c *Cache) Keys() []int {
	c.mu.Lock()
	defer c.mu.Unlock()

	result := make([]int, 0, len(c.items))
	curr := c.head.next
	for curr != c.tail {
		result = append(result, curr.key)
		curr = curr.next
	}
	empty := len(result) == 0
	if empty {
		return nil
	}
	return result
}
