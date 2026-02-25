class Node:
    """Doubly-linked list node for cache entries."""

    def __init__(self, key=0, value=0):
        self.key = key
        self.value = value
        self.prev = None
        self.next = None


class Cache:
    # LRU cache with fixed capacity
    # - O(1) get and put operations
    # - doubly linked list for ordering
    # - dict for fast key lookup

    def __init__(self, cap):
        self.cap = cap
        self.items = {}
        self.head = Node()
        self.tail = Node()
        self.head.next = self.tail
        self.tail.prev = self.head

    def get(self, key):
        found = key in self.items
        node = self.items.get(key)
        value = node.value if found else -1
        if found:
            self._detach(node)
            self._attach(node)
        return value

    def put(self, key, value):
        # - update value if key exists
        # - evict tail entry when full
        # - always move to front
        if key in self.items:
            node = self.items[key]
            node.value = value
            self._detach(node)
            self._attach(node)
            return

        full = len(self.items) >= self.cap
        if full:
            old = self.tail.prev
            self._detach(old)
            del self.items[old.key]

        node = Node(key, value)
        self.items[key] = node
        self._attach(node)

    def _detach(self, node):
        # - unlink node from its neighbors
        # - stitch prev and next together
        # - clear node pointers
        prev = node.prev
        after = node.next
        prev.next = after
        after.prev = prev

    def _attach(self, node):
        first = self.head.next
        self.head.next = node
        node.prev = self.head
        node.next = first
        first.prev = node

    def size(self):
        return len(self.items)

    def peek(self, key):
        found = key in self.items
        return self.items[key].value if found else None

    def clear(self):
        self.items.clear()
        self.head.next = self.tail
        self.tail.prev = self.head

    def keys(self):
        result = []
        node = self.head.next
        while node != self.tail:
            result.append(node.key)
            node = node.next
        empty = len(result) == 0
        return result if not empty else []
