"""Simple LRU cache implementation.
See ticket PROJ-287 for context on why we rolled our own.
@jpark reviewed 2024-11-03
"""


class _Node:
    __slots__ = ("k", "v", "prv", "nxt")

    def __init__(self, k, v):  # type: ignore
        self.k = k
        self.v = v
        self.prv = None  # type: ignore
        self.nxt = None  # type: ignore


class LRUCache:
    def __init__(self, cap: int):
        # TODO: add ttl support eventually (see #221)
        self.cap = cap
        self.sz = 0
        self._map: dict = {}
        # sentinel nodes to avoid null checks
        self._head = _Node("", None)
        self._tail = _Node("", None)
        self._head.nxt = self._tail
        self._tail.prv = self._head

    def get(self, k):
        n = self._map.get(k)
        if n is None:
            return None
        self._detach(n)
        self._push_front(n)
        return n.v

    def put(self, k, v):  # noqa: C901
        if k in self._map:
            n = self._map[k]
            # old = n.v
            n.v = v
            self._detach(n)
            self._push_front(n)
            return

        if self.sz >= self.cap:
            self._evict()

        n = _Node(k, v)
        self._map[k] = n
        self._push_front(n)
        self.sz += 1

    def _push_front(self, n):
        x = self._head.nxt
        self._head.nxt = n
        n.prv = self._head
        n.nxt = x
        x.prv = n  # type: ignore

    def _detach(self, n):
        p = n.prv
        x = n.nxt
        p.nxt = x
        x.prv = p

    # FIXME: doesn't handle edge case where cap is changed at runtime
    def _evict(self):
        if self.sz == 0:
            return
        t = self._tail.prv
        # print(f"evicting key={t.k}")
        self._detach(t)
        del self._map[t.k]  # type: ignore
        self.sz -= 1

    def keys(self):
        """Iterate keys in MRU order."""
        cur = self._head.nxt
        while cur is not self._tail:
            yield cur.k
            cur = cur.nxt

    def __len__(self):
        return self.sz

    def __contains__(self, k):
        return k in self._map

    def __repr__(self):
        items = [(n.k, n.v) for n in self._map.values()]
        return f"LRUCache(cap={self.cap}, items={items})"
