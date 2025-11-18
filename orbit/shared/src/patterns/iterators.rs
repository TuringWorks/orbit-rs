//! Custom iterator patterns and implementations
//!
//! Demonstrates implementing custom iterators, iterator adaptors, and
//! advanced iteration patterns in Rust.

use crate::error::{OrbitError, OrbitResult};
use std::collections::VecDeque;

// ===== Window Iterator =====

/// Iterator that yields sliding windows over a collection
pub struct WindowIterator<'a, T> {
    data: &'a [T],
    window_size: usize,
    position: usize,
}

impl<'a, T> WindowIterator<'a, T> {
    pub fn new(data: &'a [T], window_size: usize) -> Self {
        Self {
            data,
            window_size,
            position: 0,
        }
    }
}

impl<'a, T> Iterator for WindowIterator<'a, T> {
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.position + self.window_size <= self.data.len() {
            let window = &self.data[self.position..self.position + self.window_size];
            self.position += 1;
            Some(window)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.data.len().saturating_sub(self.position + self.window_size - 1);
        (remaining, Some(remaining))
    }
}

impl<'a, T> ExactSizeIterator for WindowIterator<'a, T> {
    fn len(&self) -> usize {
        self.data.len().saturating_sub(self.position + self.window_size - 1)
    }
}

// Extension trait for window iteration
pub trait WindowExt<T> {
    fn windows_iter(&self, window_size: usize) -> WindowIterator<'_, T>;
}

impl<T> WindowExt<T> for [T] {
    fn windows_iter(&self, window_size: usize) -> WindowIterator<'_, T> {
        WindowIterator::new(self, window_size)
    }
}

// ===== Chunk Iterator =====

/// Iterator that yields non-overlapping chunks
pub struct ChunkIterator<'a, T> {
    data: &'a [T],
    chunk_size: usize,
    position: usize,
}

impl<'a, T> ChunkIterator<'a, T> {
    pub fn new(data: &'a [T], chunk_size: usize) -> Self {
        Self {
            data,
            chunk_size,
            position: 0,
        }
    }
}

impl<'a, T> Iterator for ChunkIterator<'a, T> {
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.position < self.data.len() {
            let end = (self.position + self.chunk_size).min(self.data.len());
            let chunk = &self.data[self.position..end];
            self.position = end;
            Some(chunk)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.data.len() - self.position + self.chunk_size - 1) / self.chunk_size;
        (remaining, Some(remaining))
    }
}

// ===== Circular Buffer Iterator =====

/// Iterator over a circular buffer
pub struct CircularBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T> IntoIterator for CircularBuffer<T> {
    type Item = T;
    type IntoIter = std::collections::vec_deque::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffer.into_iter()
    }
}

// ===== Flatten Iterator (Custom) =====

/// Custom flatten implementation for nested iterators
pub struct Flatten<I>
where
    I: Iterator,
    I::Item: IntoIterator,
{
    outer: I,
    inner: Option<<I::Item as IntoIterator>::IntoIter>,
}

impl<I> Flatten<I>
where
    I: Iterator,
    I::Item: IntoIterator,
{
    pub fn new(iter: I) -> Self {
        Self {
            outer: iter,
            inner: None,
        }
    }
}

impl<I> Iterator for Flatten<I>
where
    I: Iterator,
    I::Item: IntoIterator,
{
    type Item = <I::Item as IntoIterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut inner) = self.inner {
                if let Some(item) = inner.next() {
                    return Some(item);
                }
                self.inner = None;
            }

            match self.outer.next() {
                Some(next_inner) => {
                    self.inner = Some(next_inner.into_iter());
                }
                None => return None,
            }
        }
    }
}

// ===== Paginated Iterator =====

/// Iterator that yields items in pages
pub struct PaginatedIterator<I: Iterator> {
    inner: I,
    page_size: usize,
    current_page: usize,
}

impl<I: Iterator> PaginatedIterator<I> {
    pub fn new(inner: I, page_size: usize) -> Self {
        Self {
            inner,
            page_size,
            current_page: 0,
        }
    }

    pub fn next_page(&mut self) -> Option<Vec<I::Item>> {
        let mut page = Vec::with_capacity(self.page_size);

        for _ in 0..self.page_size {
            match self.inner.next() {
                Some(item) => page.push(item),
                None => break,
            }
        }

        if page.is_empty() {
            None
        } else {
            self.current_page += 1;
            Some(page)
        }
    }

    pub fn current_page_number(&self) -> usize {
        self.current_page
    }
}

// ===== Batched Iterator =====

/// Iterator that yields batches of items
pub struct BatchIterator<I: Iterator> {
    inner: I,
    batch_size: usize,
}

impl<I: Iterator> BatchIterator<I> {
    pub fn new(inner: I, batch_size: usize) -> Self {
        Self { inner, batch_size }
    }
}

impl<I: Iterator> Iterator for BatchIterator<I> {
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut batch = Vec::with_capacity(self.batch_size);

        for _ in 0..self.batch_size {
            match self.inner.next() {
                Some(item) => batch.push(item),
                None => break,
            }
        }

        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    }
}

// ===== Peekable N Iterator =====

/// Iterator that can peek ahead N items
pub struct PeekableN<I: Iterator> {
    inner: I,
    peeked: VecDeque<I::Item>,
}

impl<I: Iterator> PeekableN<I> {
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            peeked: VecDeque::new(),
        }
    }

    /// Peek at the nth item ahead (0-indexed)
    pub fn peek_nth(&mut self, n: usize) -> Option<&I::Item>
    where
        I::Item: Clone,
    {
        while self.peeked.len() <= n {
            match self.inner.next() {
                Some(item) => self.peeked.push_back(item),
                None => return None,
            }
        }

        self.peeked.get(n)
    }

    /// Peek at all items in the buffer
    pub fn peek_all(&self) -> &VecDeque<I::Item> {
        &self.peeked
    }
}

impl<I: Iterator> Iterator for PeekableN<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.peeked.pop_front() {
            Some(item)
        } else {
            self.inner.next()
        }
    }
}

// ===== Unique Iterator =====

/// Iterator that only yields unique items
pub struct UniqueIterator<I: Iterator>
where
    I::Item: Eq + std::hash::Hash + Clone,
{
    inner: I,
    seen: std::collections::HashSet<I::Item>,
}

impl<I: Iterator> UniqueIterator<I>
where
    I::Item: Eq + std::hash::Hash + Clone,
{
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            seen: std::collections::HashSet::new(),
        }
    }
}

impl<I: Iterator> Iterator for UniqueIterator<I>
where
    I::Item: Eq + std::hash::Hash + Clone,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(item) => {
                    if self.seen.insert(item.clone()) {
                        return Some(item);
                    }
                    // Item was already seen, continue to next
                }
                None => return None,
            }
        }
    }
}

// ===== Result Iterator (Fallible) =====

/// Iterator that can fail during iteration
pub struct ResultIterator<I, E>
where
    I: Iterator,
{
    inner: I,
    mapper: Box<dyn FnMut(I::Item) -> Result<I::Item, E>>,
}

impl<I, E> ResultIterator<I, E>
where
    I: Iterator,
{
    pub fn new<F>(inner: I, mapper: F) -> Self
    where
        F: FnMut(I::Item) -> Result<I::Item, E> + 'static,
    {
        Self {
            inner,
            mapper: Box::new(mapper),
        }
    }

    /// Collect all results, stopping at first error
    pub fn try_collect<B>(mut self) -> Result<B, E>
    where
        B: FromIterator<I::Item>,
    {
        let mut items = Vec::new();
        for item in self.inner.by_ref() {
            match (self.mapper)(item) {
                Ok(mapped) => items.push(mapped),
                Err(e) => return Err(e),
            }
        }
        Ok(items.into_iter().collect())
    }
}

// ===== Extension Trait for Iterator Combinators =====

pub trait IteratorExtensions: Iterator + Sized {
    /// Batch items into groups of specified size
    fn batched(self, batch_size: usize) -> BatchIterator<Self> {
        BatchIterator::new(self, batch_size)
    }

    /// Get unique items only
    fn unique(self) -> UniqueIterator<Self>
    where
        Self::Item: Eq + std::hash::Hash + Clone,
    {
        UniqueIterator::new(self)
    }

    /// Create a peekable iterator that can peek N items ahead
    fn peekable_n(self) -> PeekableN<Self> {
        PeekableN::new(self)
    }

    /// Paginate the iterator
    fn paginated(self, page_size: usize) -> PaginatedIterator<Self> {
        PaginatedIterator::new(self, page_size)
    }
}

impl<I: Iterator> IteratorExtensions for I {}

// ===== Infinite Iterator Adaptors =====

/// Cycling iterator that repeats a sequence
pub struct Cycle<I: Iterator + Clone> {
    original: I,
    current: I,
}

impl<I: Iterator + Clone> Cycle<I> {
    pub fn new(iter: I) -> Self {
        Self {
            original: iter.clone(),
            current: iter,
        }
    }
}

impl<I: Iterator + Clone> Iterator for Cycle<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current.next() {
            Some(item) => Some(item),
            None => {
                self.current = self.original.clone();
                self.current.next()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_iterator() {
        let data = vec![1, 2, 3, 4, 5];
        let windows: Vec<_> = data.windows_iter(3).collect();

        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0], &[1, 2, 3]);
        assert_eq!(windows[1], &[2, 3, 4]);
        assert_eq!(windows[2], &[3, 4, 5]);
    }

    #[test]
    fn test_chunk_iterator() {
        let data = vec![1, 2, 3, 4, 5, 6, 7];
        let chunks: Vec<_> = ChunkIterator::new(&data, 3).collect();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2, 3]);
        assert_eq!(chunks[1], &[4, 5, 6]);
        assert_eq!(chunks[2], &[7]);
    }

    #[test]
    fn test_circular_buffer() {
        let mut buffer = CircularBuffer::new(3);

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        buffer.push(4); // Should evict 1

        let items: Vec<_> = buffer.iter().copied().collect();
        assert_eq!(items, vec![2, 3, 4]);
    }

    #[test]
    fn test_flatten_iterator() {
        let data = vec![vec![1, 2], vec![3, 4], vec![5]];
        let flattened: Vec<_> = Flatten::new(data.into_iter()).collect();

        assert_eq!(flattened, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_paginated_iterator() {
        let data = vec![1, 2, 3, 4, 5, 6, 7];
        let mut paginated = PaginatedIterator::new(data.into_iter(), 3);

        assert_eq!(paginated.next_page(), Some(vec![1, 2, 3]));
        assert_eq!(paginated.next_page(), Some(vec![4, 5, 6]));
        assert_eq!(paginated.next_page(), Some(vec![7]));
        assert_eq!(paginated.next_page(), None);
    }

    #[test]
    fn test_batch_iterator() {
        let data = vec![1, 2, 3, 4, 5];
        let batches: Vec<_> = data.into_iter().batched(2).collect();

        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], vec![1, 2]);
        assert_eq!(batches[1], vec![3, 4]);
        assert_eq!(batches[2], vec![5]);
    }

    #[test]
    fn test_peekable_n() {
        let data = vec![1, 2, 3, 4, 5];
        let mut peekable = PeekableN::new(data.into_iter());

        assert_eq!(peekable.peek_nth(0), Some(&1));
        assert_eq!(peekable.peek_nth(2), Some(&3));
        assert_eq!(peekable.next(), Some(1));
        assert_eq!(peekable.next(), Some(2));
        assert_eq!(peekable.peek_nth(0), Some(&3));
    }

    #[test]
    fn test_unique_iterator() {
        let data = vec![1, 2, 2, 3, 1, 4, 3, 5];
        let unique: Vec<_> = data.into_iter().unique().collect();

        assert_eq!(unique, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_cycle_iterator() {
        let data = vec![1, 2, 3];
        let mut cycle = Cycle::new(data.into_iter());

        assert_eq!(cycle.next(), Some(1));
        assert_eq!(cycle.next(), Some(2));
        assert_eq!(cycle.next(), Some(3));
        assert_eq!(cycle.next(), Some(1)); // Cycles back
        assert_eq!(cycle.next(), Some(2));
    }
}
