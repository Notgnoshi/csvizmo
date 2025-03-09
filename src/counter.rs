use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{Index, IndexMut};

#[derive(Default)]
pub struct Counter<T>
where
    T: Eq + Hash,
{
    counts: HashMap<T, u64>,
}

impl<T> Counter<T>
where
    T: Eq + Hash,
{
    pub fn new(values: impl IntoIterator<Item = T>) -> Counter<T> {
        let mut this = Counter {
            counts: HashMap::new(),
        };
        for value in values.into_iter() {
            this[value] += 1;
        }
        this
    }

    pub fn single_most_common(&self) -> Option<(&T, &u64)> {
        if self.counts.is_empty() {
            return None;
        }

        let mut max = 0;
        let mut candidate = None;
        for (key, count) in self.counts.iter() {
            if count > &max {
                max = *count;
                candidate = Some((key, count));
            }
        }
        candidate
    }

    pub fn most_common(&self, n: usize) -> Vec<(&T, &u64)> {
        // TODO: There's gotta be a more efficient algorithm, possibly one that maintains the
        // sorted order as new items are added / counts adjusted. Maybe that's not worth it?
        let mut counts: Vec<_> = self.counts.iter().collect();
        counts.sort_unstable_by_key(|(_, count)| *count);

        let n = n.min(counts.len());
        let num_to_drop = counts.len() - n;
        counts.drain(..num_to_drop);

        counts
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.counts.len()
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<T, u64> {
        self.counts.iter()
    }

    pub fn iter_mut(&mut self) -> std::collections::hash_map::IterMut<T, u64> {
        self.counts.iter_mut()
    }

    pub fn values(&self) -> std::collections::hash_map::Values<T, u64> {
        self.counts.values()
    }

    pub fn keys(&self) -> std::collections::hash_map::Keys<T, u64> {
        self.counts.keys()
    }
}

impl<T> IntoIterator for Counter<T>
where
    T: Eq + Hash,
{
    type Item = (T, u64);
    type IntoIter = std::collections::hash_map::IntoIter<T, u64>;

    fn into_iter(self) -> Self::IntoIter {
        self.counts.into_iter()
    }
}

impl<T> Index<&T> for Counter<T>
where
    T: Eq + Hash,
{
    type Output = u64;

    fn index(&self, key: &T) -> &u64 {
        self.counts.get(key).unwrap_or(&0)
    }
}

impl<T> Index<T> for Counter<T>
where
    T: Eq + Hash,
{
    type Output = u64;

    fn index(&self, key: T) -> &u64 {
        self.counts.get(&key).unwrap_or(&0)
    }
}

impl<T> IndexMut<&T> for Counter<T>
where
    T: Eq + Hash + Clone,
{
    fn index_mut(&mut self, key: &T) -> &mut u64 {
        self.counts.entry(key.clone()).or_insert(0)
    }
}

impl<T> IndexMut<T> for Counter<T>
where
    T: Eq + Hash,
{
    fn index_mut(&mut self, key: T) -> &mut u64 {
        self.counts.entry(key).or_insert(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_iterator() {
        let values = [1, 2, 2, 3, 3, 3];
        let counter = Counter::new(values);

        assert_eq!(counter[0], 0); // 0 not in the counter
        assert_eq!(counter[1], 1);
        assert_eq!(counter[2], 2);
        assert_eq!(counter[3], 3);
    }

    #[test]
    fn test_index_mut() {
        let values = [1, 2, 2, 3, 3, 3];
        let mut counter = Counter::default();

        for v in values {
            counter[v] += 1;
        }

        assert_eq!(counter[0], 0); // 0 not in the counter
        assert_eq!(counter[1], 1);
        assert_eq!(counter[2], 2);
        assert_eq!(counter[3], 3);
    }

    #[test]
    fn test_most_common() {
        let values = [1, 2, 2, 3, 3, 3];
        let counter = Counter::new(values);

        assert_eq!(counter.single_most_common(), Some((&3, &3)));
        let most_common = counter.most_common(2);
        let expected = vec![(&2, &2), (&3, &3)];
        assert_eq!(most_common, expected);
    }
}
