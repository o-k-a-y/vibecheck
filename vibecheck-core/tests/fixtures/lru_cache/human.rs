use std::collections::HashMap;

// Basic LRU cache - see JIRA-4821 for perf requirements
// @dmiller suggested we keep this under 128 entries for now

#[allow(dead_code)]
struct Node<V> {
    val: V,
    prev: Option<usize>,
    next: Option<usize>,
}

#[allow(dead_code)]
pub struct LruCache<V> {
    cap: usize,
    map: HashMap<String, usize>,
    buf: Vec<Node<V>>,
    head: Option<usize>,
    tail: Option<usize>,
    sz: usize,
}

#[allow(unused)]
impl<V: Clone> LruCache<V> {
    pub fn new(cap: usize) -> Self {
        // TODO: should we panic on cap=0 or just clamp to 1?
        let c = if cap == 0 { 1 } else { cap };
        Self {
            cap: c,
            map: HashMap::with_capacity(c),
            buf: Vec::with_capacity(c),
            head: None,
            tail: None,
            sz: 0,
        }
    }

    pub fn get(&mut self, k: &str) -> Option<&V> {
        let idx = *self.map.get(k)?;
        self.move_to_front(idx);
        Some(&self.buf[idx].val)
    }

    pub fn put(&mut self, k: String, v: V) {
        if let Some(&idx) = self.map.get(&k) {
            self.buf[idx].val = v;
            // let old_val = std::mem::replace(&mut self.buf[idx].val, v);
            self.move_to_front(idx);
            return;
        }

        // FIXME: reuse freed slots instead of always pushing (#456)
        if self.sz >= self.cap {
            self.evict_lru();
        }

        let n = self.buf.len();
        self.buf.push(Node { val: v, prev: None, next: self.head });

        if let Some(h) = self.head {
            self.buf[h].prev = Some(n);
        }
        self.head = Some(n);
        if self.tail.is_none() {
            self.tail = Some(n);
        }

        self.map.insert(k, n);
        self.sz += 1;
    }

    fn move_to_front(&mut self, idx: usize) {
        if self.head == Some(idx) { return; }
        let p = self.buf[idx].prev;
        let x = self.buf[idx].next;

        if let Some(p) = p { self.buf[p].next = x; }
        if let Some(x) = x { self.buf[x].prev = p; }
        if self.tail == Some(idx) { self.tail = p; }

        self.buf[idx].prev = None;
        self.buf[idx].next = self.head;
        if let Some(h) = self.head { self.buf[h].prev = Some(idx); }
        self.head = Some(idx);
    }

    fn evict_lru(&mut self) {
        if let Some(t) = self.tail {
            let _prev = self.buf[t].prev;
            // TODO: this linear scan is bad, need reverse map - JIRA-4823
            self.map.retain(|_k, &mut i| i != t);
            self.tail = self.buf[t].prev;
            if let Some(new_t) = self.tail {
                self.buf[new_t].next = None;
            }
            self.sz -= 1;
        }
    }

    pub fn len(&self) -> usize { self.sz }
}
