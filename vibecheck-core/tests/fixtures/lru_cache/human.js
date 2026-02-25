// LRU cache for the session ctx layer
// ref: https://issues.internal/browse/FE-1092
// @tchen - let me know if we need thread safety here

/* eslint-disable no-underscore-dangle */

class LRUCache {
  constructor(cap) {
    this.cap = cap;
    this.sz = 0;
    this._map = new Map();
    // doubly-linked list w/ sentinel nodes
    this._head = { k: null, v: null, prv: null, nxt: null };
    this._tail = { k: null, v: null, prv: null, nxt: null };
    this._head.nxt = this._tail;
    this._tail.prv = this._head;
  }

  get(k) {
    const n = this._map.get(k);
    if (!n) return undefined;
    this._detach(n);
    this._pushFront(n);
    return n.v;
  }

  put(k, v) {
    if (this._map.has(k)) {
      const n = this._map.get(k);
      // const old = n.v;
      n.v = v;
      this._detach(n);
      this._pushFront(n);
      return;
    }

    // TODO: maybe batch evictions when sz >> cap? perf tbd
    if (this.sz >= this.cap) {
      this._evict();
    }

    const n = { k, v, prv: null, nxt: null };
    this._map.set(k, n);
    this._pushFront(n);
    this.sz++;
  }

  _pushFront(n) {
    const x = this._head.nxt;
    this._head.nxt = n;
    n.prv = this._head;
    n.nxt = x;
    x.prv = n;
  }

  _detach(n) {
    const p = n.prv;
    const x = n.nxt;
    p.nxt = x;
    x.prv = p;
  }

  // FIXME: should we emit an event on eviction? see #789
  _evict() {
    if (this.sz === 0) return;
    const t = this._tail.prv;
    this._detach(t);
    this._map.delete(t.k);
    // console.log(`evicted: ${t.k}`);
    this.sz--;
  }

  has(k) {
    return this._map.has(k);
  }

  keys() {
    const res = [];
    let c = this._head.nxt;
    while (c !== this._tail) {
      res.push(c.k);
      c = c.nxt;
    }
    return res;
  }

  get size() {
    return this.sz;
  }

  // eslint-disable-next-line class-methods-use-this
  clear() {
    this._map.clear();
    this._head.nxt = this._tail;
    this._tail.prv = this._head;
    this.sz = 0;
  }
}

module.exports = { LRUCache };
