use std::collections::HashMap;

// LRU Cache implementation
// - fixed capacity eviction
// - O(1) get and put via HashMap + linked list
// - tracks access order for recency

struct Node {
    key: i32,
    value: i32,
    prev: Option<usize>,
    next: Option<usize>,
}

struct Cache {
    items: HashMap<i32, usize>,
    nodes: Vec<Node>,
    head: Option<usize>,
    tail: Option<usize>,
    cap: usize,
}

impl Cache {
    fn new(cap: usize) -> Self {
        Cache {
            items: HashMap::new(),
            nodes: Vec::new(),
            head: None,
            tail: None,
            cap,
        }
    }

    fn get(&mut self, key: i32) -> Option<i32> {
        let idx = *self.items.get(&key)?;
        self.detach(idx);
        self.attach(idx);
        Some(self.nodes[idx].value)
    }

    fn put(&mut self, key: i32, value: i32) {
        // - update existing entry if present
        // - evict oldest when at capacity
        // - attach new entry at head
        if let Some(&idx) = self.items.get(&key) {
            self.nodes[idx].value = value;
            self.detach(idx);
            self.attach(idx);
            return;
        }
        let evict = self.items.len() >= self.cap;
        if evict {
            let old = self.tail.unwrap();
            self.detach(old);
            self.items.remove(&self.nodes[old].key);
        }
        let idx = self.nodes.len();
        let node = Node {
            key,
            value,
            prev: None,
            next: None,
        };
        self.nodes.push(node);
        self.items.insert(key, idx);
        self.attach(idx);
    }

    fn detach(&mut self, idx: usize) {
        let prev = self.nodes[idx].prev;
        let next = self.nodes[idx].next;
        // - relink neighbors around this node
        // - update head/tail if needed
        // - clear own pointers
        match prev {
            Some(p) => self.nodes[p].next = next,
            None => self.head = next,
        }
        match next {
            Some(n) => self.nodes[n].prev = prev,
            None => self.tail = prev,
        }
        self.nodes[idx].prev = None;
        self.nodes[idx].next = None;
    }

    fn attach(&mut self, idx: usize) {
        self.nodes[idx].next = self.head;
        let had = self.head.is_some();
        if had {
            let old = self.head.unwrap();
            self.nodes[old].prev = Some(idx);
        }
        self.head = Some(idx);
        self.tail = if had { self.tail } else { Some(idx) };
    }

    fn len(&self) -> usize {
        self.items.len()
    }
}
