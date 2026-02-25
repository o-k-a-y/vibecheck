class Node:
    def __init__(self, key, value):
        self.key = key
        self.value = value
        self.prevNode = None
        self.nextNode = None

class LRUCache:
    def __init__(self, capacity):
        self.capacity = capacity
        self.cache_map = {}
        self.head = Node(0, 0)
        self.tail = Node(0, 0)
        self.head.nextNode = self.tail
        self.tail.prevNode = self.head

    def get(self, key):
        if key not in self.cache_map:
            return -1
        node = self.cache_map[key]
        self._move_to_front(node)
        return node.value

    def put(self, key, value):
        if key in self.cache_map:
            node = self.cache_map[key]
            node.value = value
            self._move_to_front(node)
            return
        if len(self.cache_map) >= self.capacity:
            self._evict_last()
        newNode = Node(key, value)
        self.cache_map[key] = newNode
        self._add_to_front(newNode)

    def _add_to_front(self, node):
        node.prevNode = self.head
        node.nextNode = self.head.nextNode
        self.head.nextNode.prevNode = node
        self.head.nextNode = node

    def _remove_node(self, node):
        prevNode = node.prevNode
        nextNode = node.nextNode
        prevNode.nextNode = nextNode
        nextNode.prevNode = prevNode

    def _move_to_front(self, node):
        self._remove_node(node)
        self._add_to_front(node)

    def _evict_last(self):
        lastNode = self.tail.prevNode
        self._remove_node(lastNode)
        del self.cache_map[lastNode.key]

    def get_size(self): return len(self.cache_map)

    def contains(self, key): return key in self.cache_map

    def clear_all(self):
        self.cache_map.clear()
        self.head.nextNode = self.tail
        self.tail.prevNode = self.head

    def peek_value(self, key):
        if key not in self.cache_map:
	        return -1
        return self.cache_map[key].value

    def get_keys(self):
        resultKeys = []
        current = self.head.nextNode
        while current != self.tail:
	        resultKeys.append(current.key)
	        current = current.nextNode
        return resultKeys

    def remove_key(self, key):
        if key not in self.cache_map:
            return False
        node = self.cache_map[key]
        self._remove_node(node)
        del self.cache_map[key]
        return True

    def __len__(self):
        return len(self.cache_map)

    def __contains__(self, key):
        return key in self.cache_map
