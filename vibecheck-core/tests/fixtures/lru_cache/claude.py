"""A least-recently-used cache built on top of Python's OrderedDict.

The OrderedDict maintains insertion order and supports efficient
reordering, which makes it well-suited for tracking access recency
without a separate data structure.
"""

from collections import OrderedDict
from typing import Generic, Hashable, Optional, TypeVar

KeyType = TypeVar("KeyType", bound=Hashable)
ValueType = TypeVar("ValueType")


class LeastRecentlyUsedCache(Generic[KeyType, ValueType]):
    """A fixed-capacity cache that evicts the least recently used entry."""

    def __init__(self, maximum_capacity: int) -> None:
        """Initialize the cache with the given maximum capacity.

        Note that the capacity must be a positive integer. A zero or
        negative capacity would make the cache unable to store anything.
        """
        if maximum_capacity <= 0:
            raise ValueError("Cache capacity must be a positive integer")
        self.maximum_capacity = maximum_capacity
        self.ordered_storage: OrderedDict[KeyType, ValueType] = OrderedDict()

    def insert_entry(self, cache_key: KeyType, cache_value: ValueType) -> None:
        """Insert or update a key-value pair in the cache.

        If the key already exists, its value is updated and it is moved
        to the most-recently-used position. If the cache is at capacity,
        the least recently used entry is evicted before the new entry is
        added. This ensures the cache never exceeds its configured bound.
        """
        if cache_key in self.ordered_storage:
            self.ordered_storage.move_to_end(cache_key)
            self.ordered_storage[cache_key] = cache_value
            return

        if len(self.ordered_storage) >= self.maximum_capacity:
            self._evict_least_recently_used_entry()

        self.ordered_storage[cache_key] = cache_value

    def retrieve_entry(self, cache_key: KeyType) -> Optional[ValueType]:
        """Retrieve the value for the given key, if it exists.

        Accessing an entry promotes it to the most-recently-used
        position, which protects it from near-term eviction.
        """
        if cache_key not in self.ordered_storage:
            return None
        self.ordered_storage.move_to_end(cache_key)
        return self.ordered_storage[cache_key]

    def contains_key(self, cache_key: KeyType) -> bool:
        """Check whether the cache contains the given key.

        Note that this method does not count as an access and therefore
        does not affect the eviction order of entries.
        """
        return cache_key in self.ordered_storage

    def current_entry_count(self) -> int:
        """Return the number of entries currently stored in the cache."""
        return len(self.ordered_storage)

    def _evict_least_recently_used_entry(self) -> None:
        """Remove the entry at the front of the ordered storage.

        The front of the OrderedDict corresponds to the least recently
        used entry, since every access moves its key to the end.
        """
        self.ordered_storage.popitem(last=False)


if __name__ == "__main__":
    demonstration_cache: LeastRecentlyUsedCache[str, int] = LeastRecentlyUsedCache(
        maximum_capacity=3
    )
    demonstration_cache.insert_entry("alpha_key", 10)
    demonstration_cache.insert_entry("beta_key", 20)
    demonstration_cache.insert_entry("gamma_key", 30)

    # This access promotes "alpha_key" so it will not be the next evicted
    retrieved_value = demonstration_cache.retrieve_entry("alpha_key")
    assert retrieved_value == 10

    demonstration_cache.insert_entry("delta_key", 40)

    # Note that "beta_key" was evicted because it was the least recently used
    assert not demonstration_cache.contains_key("beta_key")
    assert demonstration_cache.current_entry_count() == 3
