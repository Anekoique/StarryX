//! `BTreeMap` with weak references.

use alloc::collections::btree_map;
use core::{
    borrow::Borrow,
    fmt,
    iter::FusedIterator,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{StrongRef, WeakRef};

#[derive(Default)]
struct OpsCounter(AtomicUsize);

const OPS_THRESHOLD: usize = 1000;

impl OpsCounter {
    #[inline]
    const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    #[inline]
    fn add(&self, ops: usize) {
        self.0.fetch_add(ops, Ordering::Relaxed);
    }

    #[inline]
    fn bump(&self) {
        self.add(1);
    }

    #[inline]
    fn reset(&mut self) {
        *self.0.get_mut() = 0;
    }

    #[inline]
    fn get(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    #[inline]
    fn reach_threshold(&self) -> bool {
        self.get() >= OPS_THRESHOLD
    }
}

impl Clone for OpsCounter {
    #[inline]
    fn clone(&self) -> Self {
        Self(AtomicUsize::new(self.get()))
    }
}

/// Alias for `BTreeMap<K, V>`.
pub type StrongMap<K, V> = btree_map::BTreeMap<K, V>;

/// A B-Tree map that stores weak references to values.
#[derive(Clone)]
pub struct WeakMap<K, V> {
    inner: btree_map::BTreeMap<K, V>,
    ops: OpsCounter,
}

impl<K, V> WeakMap<K, V> {
    /// Makes a new, empty `WeakMap`.
    ///
    /// Does not allocate anything on its own.
    #[inline]
    pub const fn new() -> Self {
        Self {
            inner: btree_map::BTreeMap::new(),
            ops: OpsCounter::new(),
        }
    }
}

impl<K, V> Default for WeakMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> From<btree_map::BTreeMap<K, V>> for WeakMap<K, V> {
    #[inline]
    fn from(inner: btree_map::BTreeMap<K, V>) -> Self {
        Self {
            inner,
            ops: OpsCounter::new(),
        }
    }
}

impl<K, V> From<WeakMap<K, V>> for btree_map::BTreeMap<K, V> {
    #[inline]
    fn from(map: WeakMap<K, V>) -> Self {
        map.inner
    }
}

impl<K, V> WeakMap<K, V> {
    /// Clears the map, removing all elements.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
        self.ops.reset();
    }

    /// Returns the number of elements in the underlying map.
    #[must_use]
    pub fn raw_len(&self) -> usize {
        self.inner.len()
    }

    /// Gets an iterator over the entries of the map, sorted by key.
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        self.ops.add(self.inner.len());
        Iter(self.inner.iter())
    }

    /// Gets an iterator over the keys of the map, in sorted order.
    #[inline]
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys(self.iter())
    }

    /// Creates a consuming iterator visiting all the keys, in sorted order.
    /// The map cannot be used after calling this.
    #[inline]
    pub fn into_keys(self) -> IntoKeys<K, V> {
        IntoKeys(IntoIter(self.inner.into_iter()))
    }

    /// Gets an iterator over the values of the map, in order by key.
    #[inline]
    pub fn values(&self) -> Values<'_, K, V> {
        Values(self.iter())
    }

    /// Creates a consuming iterator visiting all the values, in order by key.
    /// The map cannot be used after calling this.
    #[inline]
    pub fn into_values(self) -> IntoValues<K, V> {
        IntoValues(IntoIter(self.inner.into_iter()))
    }
}

impl<K, V> WeakMap<K, V>
where
    K: Ord,
    V: WeakRef,
{
    /// Cleans up the map by removing expired values.
    ///
    /// Usually you don't need to call this manually, as it is called
    /// automatically when the number of operations reaches a threshold.
    #[inline]
    pub fn cleanup(&mut self) {
        self.ops.reset();
        self.inner.retain(|_, v| !v.is_expired());
    }

    #[inline]
    fn try_bump(&mut self) {
        self.ops.bump();
        if self.ops.reach_threshold() {
            self.cleanup();
        }
    }

    /// Returns the number of elements in the map, excluding expired values.
    ///
    /// This is a linear operation, as it iterates over all elements in the map.
    ///
    /// The returned value may be less than the result of [`Self::raw_len`].
    #[inline]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Returns `true` if the map contains no valid elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Retains only the elements specified by the predicate.
    #[inline]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, V::Strong) -> bool,
    {
        self.ops.reset();
        self.inner.retain(|k, v| {
            if let Some(v) = v.upgrade() {
                f(k, v)
            } else {
                false
            }
        });
    }

    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    pub fn get<Q>(&self, key: &Q) -> Option<V::Strong>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.ops.bump();
        self.inner.get(key).and_then(V::upgrade)
    }

    /// Returns the key-value pair corresponding to the supplied key. This is
    /// potentially useful:
    /// - for key types where non-identical keys can be considered equal;
    /// - for getting the `&K` stored key value from a borrowed `&Q` lookup key; or
    /// - for getting a reference to a key with the same lifetime as the collection.
    ///
    /// The supplied key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, V::Strong)>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.ops.bump();
        self.inner
            .get_key_value(key)
            .and_then(|(k, v)| v.upgrade().map(|v| (k, v)))
    }

    /// Returns `true` if the map contains a value for the specified key.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.ops.bump();
        self.inner.get(key).is_some_and(|v| !v.is_expired())
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    ///
    /// If the map did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though; this matters for
    /// types that can be `==` without being identical. See the [module-level
    /// documentation] for more.
    ///
    /// [module-level documentation]: https://doc.rust-lang.org/std/collections/index.html#insert-and-complex-keys
    pub fn insert(&mut self, key: K, value: &V::Strong) -> Option<V::Strong> {
        self.try_bump();
        self.inner
            .insert(key, V::Strong::downgrade(value))
            .and_then(|v| v.upgrade())
    }

    /// Removes a key from the map, returning the value at the key if the key
    /// was previously in the map.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V::Strong>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.try_bump();
        self.inner.remove(key).and_then(|v| v.upgrade())
    }

    /// Removes a key from the map, returning the stored key and value if the key
    /// was previously in the map.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    pub fn remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V::Strong)>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.try_bump();
        self.inner
            .remove_entry(key)
            .and_then(|(k, v)| v.upgrade().map(|v| (k, v)))
    }

    /// Gets a mutable iterator over the entries of the map, sorted by key.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        self.ops.add(self.inner.len());
        if self.ops.reach_threshold() {
            self.cleanup();
        }
        IterMut(self.inner.iter_mut())
    }

    /// Upgrade this `WeakMap` to a `StrongMap`.
    pub fn upgrade(&self) -> StrongMap<K, V::Strong>
    where
        K: Clone,
    {
        self.ops.bump();
        let mut map = StrongMap::new();
        for (key, value) in self.iter() {
            map.insert(key.clone(), value);
        }
        map
    }
}

impl<K, V> PartialEq for WeakMap<K, V>
where
    K: Ord,
    V: WeakRef,
{
    fn eq(&self, other: &Self) -> bool {
        self.iter().all(|(key, value)| {
            other
                .get(key)
                .is_some_and(|v| V::Strong::ptr_eq(&value, &v))
        })
    }
}

impl<K, V> Eq for WeakMap<K, V>
where
    K: Ord,
    V: WeakRef,
{
}

impl<K, V> fmt::Debug for WeakMap<K, V>
where
    K: fmt::Debug,
    V: WeakRef,
    V::Strong: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<'a, K, V> FromIterator<(K, &'a V::Strong)> for WeakMap<K, V>
where
    K: Ord,
    V: WeakRef,
{
    #[inline]
    fn from_iter<T: IntoIterator<Item = (K, &'a V::Strong)>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let mut map = WeakMap::new();
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}

impl<K, V, const N: usize> From<[(K, &V::Strong); N]> for WeakMap<K, V>
where
    K: Ord,
    V: WeakRef,
{
    #[inline]
    fn from(array: [(K, &V::Strong); N]) -> Self {
        array.into_iter().collect()
    }
}

impl<K, V> From<&StrongMap<K, V::Strong>> for WeakMap<K, V>
where
    K: Ord + Clone,
    V: WeakRef,
{
    fn from(value: &StrongMap<K, V::Strong>) -> Self {
        let mut map = WeakMap::new();
        for (key, value) in value.iter() {
            map.insert(key.clone(), value);
        }
        map
    }
}

/// An iterator over the entries of a `WeakMap`.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Iter<'a, K, V>(btree_map::Iter<'a, K, V>);

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    V: WeakRef,
{
    type Item = (&'a K, V::Strong);

    fn next(&mut self) -> Option<Self::Item> {
        for (key, value) in self.0.by_ref() {
            if let Some(value) = value.upgrade() {
                return Some((key, value));
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.0.len()))
    }
}

impl<K, V> FusedIterator for Iter<'_, K, V> where V: WeakRef {}

impl<K, V> Default for Iter<'_, K, V> {
    fn default() -> Self {
        Iter(btree_map::Iter::default())
    }
}

impl<K, V> Clone for Iter<'_, K, V> {
    fn clone(&self) -> Self {
        Iter(self.0.clone())
    }
}

impl<K, V> fmt::Debug for Iter<'_, K, V>
where
    K: fmt::Debug,
    V: WeakRef,
    V::Strong: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<'a, K, V> IntoIterator for &'a WeakMap<K, V>
where
    V: WeakRef,
{
    type IntoIter = Iter<'a, K, V>;
    type Item = (&'a K, V::Strong);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A mutable iterator over the entries of a `BTreeMap`.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct IterMut<'a, K, V>(btree_map::IterMut<'a, K, V>);

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.0.len()))
    }
}

impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {}

impl<K, V> FusedIterator for IterMut<'_, K, V> {}

impl<K, V> Default for IterMut<'_, K, V> {
    fn default() -> Self {
        IterMut(btree_map::IterMut::default())
    }
}

impl<'a, K, V> IntoIterator for &'a mut WeakMap<K, V>
where
    K: Ord,
    V: WeakRef,
{
    type IntoIter = IterMut<'a, K, V>;
    type Item = (&'a K, &'a mut V);

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// An iterator over the keys of a `WeakMap`.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Keys<'a, K, V>(Iter<'a, K, V>);

impl<'a, K, V> Iterator for Keys<'a, K, V>
where
    V: WeakRef,
{
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> FusedIterator for Keys<'_, K, V> where V: WeakRef {}

impl<K, V> Default for Keys<'_, K, V> {
    fn default() -> Self {
        Keys(Iter::default())
    }
}

impl<K, V> Clone for Keys<'_, K, V> {
    fn clone(&self) -> Self {
        Keys(self.0.clone())
    }
}

impl<K, V> fmt::Debug for Keys<'_, K, V>
where
    K: fmt::Debug,
    V: WeakRef,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// An iterator over the values of a `WeakMap`.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Values<'a, K, V>(Iter<'a, K, V>);

impl<K, V> Iterator for Values<'_, K, V>
where
    V: WeakRef,
{
    type Item = V::Strong;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> FusedIterator for Values<'_, K, V> where V: WeakRef {}

impl<K, V> Default for Values<'_, K, V> {
    fn default() -> Self {
        Values(Iter::default())
    }
}

impl<K, V> Clone for Values<'_, K, V> {
    fn clone(&self) -> Self {
        Values(self.0.clone())
    }
}

impl<K, V> fmt::Debug for Values<'_, K, V>
where
    V: WeakRef,
    V::Strong: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// An owning iterator over the entries of a `WeakMap`.
pub struct IntoIter<K, V>(btree_map::IntoIter<K, V>);

impl<K, V> Iterator for IntoIter<K, V>
where
    V: WeakRef,
{
    type Item = (K, V::Strong);

    fn next(&mut self) -> Option<Self::Item> {
        for (key, value) in self.0.by_ref() {
            if let Some(value) = value.upgrade() {
                return Some((key, value));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.0.len()))
    }
}

impl<K, V> FusedIterator for IntoIter<K, V> where V: WeakRef {}

impl<K, V> Default for IntoIter<K, V> {
    fn default() -> Self {
        IntoIter(btree_map::IntoIter::default())
    }
}

impl<K, V> IntoIterator for WeakMap<K, V>
where
    V: WeakRef,
{
    type IntoIter = IntoIter<K, V>;
    type Item = (K, V::Strong);

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.inner.into_iter())
    }
}

/// An owning iterator over the keys of a `WeakMap`.
pub struct IntoKeys<K, V>(IntoIter<K, V>);

impl<K, V> Iterator for IntoKeys<K, V>
where
    V: WeakRef,
{
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(key, _)| key)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> FusedIterator for IntoKeys<K, V> where V: WeakRef {}

impl<K, V> Default for IntoKeys<K, V> {
    fn default() -> Self {
        IntoKeys(IntoIter::default())
    }
}

/// An owning iterator over the values of a `WeakMap`.`
pub struct IntoValues<K, V>(IntoIter<K, V>);

impl<K, V> Iterator for IntoValues<K, V>
where
    V: WeakRef,
{
    type Item = V::Strong;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, value)| value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> FusedIterator for IntoValues<K, V> where V: WeakRef {}

impl<K, V> Default for IntoValues<K, V> {
    fn default() -> Self {
        IntoValues(IntoIter::default())
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::{Arc, Weak};

    use super::*;

    #[test]
    fn test_basic() {
        let mut map = WeakMap::<u32, Weak<&str>>::new();

        let elem1 = Arc::new("1");
        map.insert(1, &elem1);

        {
            let elem2 = Arc::new("2");
            map.insert(2, &elem2);
        }

        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&1), Some(elem1));
        assert_eq!(map.get(&2), None);
    }

    #[test]
    fn test_cleanup() {
        let mut map = WeakMap::<usize, Weak<usize>>::new();

        for i in 0..OPS_THRESHOLD * 10 {
            let elem = Arc::new(i);
            map.insert(i, &elem);
        }

        assert_eq!(map.len(), 0);
        assert_eq!(map.raw_len(), 1);
    }
}
