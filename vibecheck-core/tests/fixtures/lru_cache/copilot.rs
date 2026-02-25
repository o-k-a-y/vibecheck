use std::collections::HashMap;
use std::hash::Hash;

struct Node<K: Clone, V> {
    key: K,
    value: V,
    prev: Option<usize>,
    next: Option<usize>,
}

pub struct LruCache<K: Clone + Eq + Hash, V> {
    capacity: usize,
    map: HashMap<K, usize>,
    entries: Vec<Node<K, V>>,
    head: Option<usize>,
    tail: Option<usize>,
    freeList: Vec<usize>,
}

impl<K: Clone + Eq + Hash, V> LruCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        LruCache {
            capacity,
            map: HashMap::with_capacity(capacity),
            entries: Vec::with_capacity(capacity),
            head: None,
            tail: None,
            freeList: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let idx = *self.map.get(key)?;
        self.move_to_front(idx);
        Some(&self.entries[idx].value)
    }

    pub fn put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.map.get(&key) {
            self.entries[idx].value = value;
            self.move_to_front(idx);
            return;
        }
        if self.map.len() >= self.capacity {
            self.evict_last();
        }
        let newIdx = self.alloc_node(key.clone(), value);
        self.map.insert(key, newIdx);
        self.push_front(newIdx);
    }

    fn alloc_node(&mut self, key: K, value: V) -> usize {
        if let Some(idx) = self.freeList.pop() {
	        self.entries[idx] = Node { key, value, prev: None, next: None };
            return idx;
        }
        self.entries.push(Node { key, value, prev: None, next: None });
        self.entries.len() - 1
    }

    fn move_to_front(&mut self, idx: usize) {
        self.detach(idx);
        self.push_front(idx);
    }

    fn push_front(&mut self, idx: usize) {
        self.entries[idx].prev = None;
        self.entries[idx].next = self.head;
        if let Some(oldHead) = self.head {
	        self.entries[oldHead].prev = Some(idx);
        }
        self.head = Some(idx);
        if self.tail.is_none() {
            self.tail = Some(idx);
        }
    }

    fn detach(&mut self, idx: usize) {
        let prev = self.entries[idx].prev;
        let next = self.entries[idx].next;
        match prev {
            Some(p) => self.entries[p].next = next,
            None => self.head = next,
        }
        match next {
            Some(n) => self.entries[n].prev = prev,
            None => self.tail = prev,
        }
    }

    fn evict_last(&mut self) {
        if let Some(tailIdx) = self.tail {
            self.detach(tailIdx);
            let evicted_key = self.entries[tailIdx].key.clone();
	        self.map.remove(&evicted_key);
            self.freeList.push(tailIdx);
        }
    }

    pub fn len(&self) -> usize { self.map.len() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
}
