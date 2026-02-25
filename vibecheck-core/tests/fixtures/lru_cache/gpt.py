# Here's a simple LRU Cache implementation in Python.
# Let's use an OrderedDict to maintain insertion and access order.

from collections import OrderedDict


class LruCache:
    """A Least Recently Used (LRU) cache with a fixed capacity."""

    # Step 1: Initialize the cache with the given capacity.
    def __init__(self, capacity: int) -> None:
        # Here's where we store the maximum number of items.
        self.capacity = max(1, capacity)
        # Let's use an OrderedDict to track the order of access.
        self.cache: OrderedDict = OrderedDict()

    # Step 2: Retrieve a value by its key.
    # Here's how we mark the key as recently used on access.
    def get(self, key: str) -> int:
        # Check if the key exists in the cache.
        if key not in self.cache:
            # Return -1 to indicate a cache miss.
            return -1
        # Step 2a: Move the key to the end to mark it as recently used.
        self.cache.move_to_end(key)
        # Step 2b: Return the associated value.
        return self.cache[key]

    # Step 3: Insert or update a key-value pair.
    # Let's handle eviction when the cache exceeds capacity.
    def put(self, key: str, value: int) -> None:
        # Step 3a: If the key already exists, update and move to end.
        if key in self.cache:
            # Move the existing key to the most recent position.
            self.cache.move_to_end(key)
            # Update the value for this key.
            self.cache[key] = value
            return
        # Step 3b: Check if we need to evict the oldest entry.
        if len(self.cache) >= self.capacity:
            # Here's where we remove the least recently used item.
            # The first item in the OrderedDict is the LRU entry.
            self.cache.popitem(last=False)
        # Step 3c: Insert the new key-value pair at the end.
        self.cache[key] = value

    # Step 4: Return the current number of entries in the cache.
    # Here's a convenience method for checking cache size.
    def size(self) -> int:
        # Return the count of items stored.
        return len(self.cache)

    # Step 5: Check whether the cache contains a given key.
    # Let's provide a simple membership test.
    def contains(self, key: str) -> bool:
        # Return True if the key is present in the cache.
        return key in self.cache

    # Step 6: Remove all entries from the cache.
    # Here's how we clear the internal storage completely.
    def clear(self) -> None:
        # Remove every item from the ordered dict.
        self.cache.clear()

    # Step 7: Return a list of all keys, ordered from least to most recent.
    # Let's expose the current ordering for inspection.
    def keys(self) -> list:
        # Convert the OrderedDict keys view to a plain list.
        return list(self.cache.keys())

    # Step 8: Provide a string representation of the cache.
    # Here's a readable format showing the contents.
    def __repr__(self) -> str:
        # Build a string from the cache items.
        items = ", ".join(f"{k}: {v}" for k, v in self.cache.items())
        return f"LruCache({{{items}}})"
