// LRU Cache
// - doubly linked list + map for O(1) ops
// - async fetch helper for cache-miss loading
// - fixed capacity with automatic eviction

class Node {
    constructor(key, value) {
        this.key = key;
        this.value = value;
        this.prev = null;
        this.next = null;
    }
}

class Cache {
    constructor(cap) {
        this.cap = cap;
        this.items = new Map();
        this.head = new Node(0, 0);
        this.tail = new Node(0, 0);
        this.head.next = this.tail;
        this.tail.prev = this.head;
    }

    get(key) {
        const found = this.items.has(key);
        const node = found ? this.items.get(key) : null;
        if (!found) return -1;
        this._detach(node);
        this._attach(node);
        return node.value;
    }

    put(key, value) {
        // - update if key exists already
        // - evict oldest when at capacity
        // - insert new entry at front
        const exist = this.items.has(key);
        if (exist) {
            const node = this.items.get(key);
            node.value = value;
            this._detach(node);
            this._attach(node);
            return;
        }

        const full = this.items.size >= this.cap;
        if (full) {
            const { key: oldKey } = this.tail.prev;
            this._detach(this.tail.prev);
            this.items.delete(oldKey);
        }

        const node = new Node(key, value);
        this.items.set(key, node);
        this._attach(node);
    }

    async fetch(key, loader) {
        // - return cached value if present
        // - await loader on cache miss
        // - store loaded result in cache
        const hit = this.items.has(key);
        if (hit) return this.get(key);

        const value = await loader(key);
        this.put(key, value);
        return value;
    }

    _detach(node) {
        const { prev, next } = node;
        prev.next = next;
        next.prev = prev;
    }

    _attach(node) {
        const { next: first } = this.head;
        this.head.next = node;
        node.prev = this.head;
        node.next = first;
        first.prev = node;
    }

    size() {
        return this.items.size;
    }

    keys() {
        const result = [];
        let curr = this.head.next;
        while (curr !== this.tail) {
            const { key } = curr;
            result.push(key);
            curr = curr.next;
        }
        return result.length > 0 ? result : [];
    }

    clear() {
        this.items.clear();
        this.head.next = this.tail;
        this.tail.prev = this.head;
    }
}

module.exports = { Cache, Node };
