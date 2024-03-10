use core::fmt;
use std::{cell::RefCell, iter, rc::Rc, slice};

use serde::{Deserialize, Serialize};

const DEFAULT_CAPACITY: usize = 4;

/// https://github.com/fitzgen/generational-arena/blob/master/src/lib.rs
/// https://www.youtube.com/watch?v=aKLntZcp27M
/// https://kyren.github.io/2018/09/14/rustconf-talk.html
///
/// inspiration from:
/// - RustConf 2018 - Closing Keynote - Using Rust For Game Development by Catherine West
/// - https://github.com/fitzgen/generational-arena

#[derive(Debug, Default, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub struct Index {
    index: usize,
    version: u64,
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.index, self.version)
    }
}

/// ///////////////////////////////
///
/// Allocator
///
/// ///////////////////////////////
#[derive(Debug, Clone, Copy)]
enum AllocatorEntry {
    Occupied { version: u64 },
    Free { next: Option<usize> },
}

impl Default for AllocatorEntry {
    fn default() -> Self {
        Self::Free { next: None }
    }
}

#[derive(Debug)]
pub struct Allocator {
    entries: Vec<AllocatorEntry>,
    next: Option<usize>,
    version: u64,
    length: usize,
}

impl Default for Allocator {
    fn default() -> Self {
        Self {
            entries: Vec::<AllocatorEntry>::with_capacity(DEFAULT_CAPACITY),
            next: None,
            version: 0,
            length: 0,
        }
    }
}

impl Allocator {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::<AllocatorEntry>::with_capacity(capacity),
            next: None,
            version: 0,
            length: 0,
        }
    }

    #[inline]
    pub fn allocate(&mut self) -> Index {
        let index = match self.try_allocate() {
            None => {
                let i = self.grow();

                Index {
                    index: i,
                    version: self.version,
                }
            }
            Some(index) => index,
        };

        self.entries[index.index] = AllocatorEntry::Occupied {
            version: self.version,
        };

        self.length += 1;

        index
    }

    #[inline]
    fn try_allocate(&mut self) -> Option<Index> {
        match self.next {
            Some(i) => match self.entries[i] {
                AllocatorEntry::Occupied { .. } => panic!("corrupt indexed array"),
                AllocatorEntry::Free { next } => {
                    self.next = next;

                    Some(Index {
                        index: i,
                        version: self.version,
                    })
                }
            },
            None => None,
        }
    }

    #[inline]
    pub fn deallocate(&mut self, index: Index) {
        if self.validate(&index) {
            self.entries[index.index] = AllocatorEntry::Free { next: self.next };

            self.next = Some(index.index);
            self.version += 1;

            self.length -= 1;
        }
    }

    #[inline]
    pub fn is_allocated(&self, index: &Index) -> bool {
        match self.entries.get(index.index) {
            None => false,
            Some(entry) => match entry {
                AllocatorEntry::Occupied { version } => *version == index.version,
                AllocatorEntry::Free { .. } => false,
            },
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn valid_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, AllocatorEntry::Occupied { .. }))
            .count()
    }

    fn grow(&mut self) -> usize {
        if self.entries.len() > self.entries.capacity() {
            self.entries.reserve(self.entries.capacity() * 2);
        }

        self.entries.push(AllocatorEntry::Free { next: None });

        self.entries.len() - 1
    }

    pub fn reset(&mut self) {
        self.entries.clear();
        self.next = None;
        self.version = 0;
        self.length = 0;
    }

    #[inline]
    pub fn validate(&self, index: &Index) -> bool {
        match self.entries.get(index.index) {
            Some(AllocatorEntry::Occupied { version }) => *version == index.version,
            _ => false,
        }
    }

    #[inline]
    fn index_at(&self, index: usize) -> Option<Index> {
        match self.entries.get(index) {
            Some(AllocatorEntry::Occupied { version }) => Some(Index {
                index,
                version: *version,
            }),
            _ => None,
        }
    }
}

/// ///////////////////////////////
///
/// Indexed Array
///
/// ///////////////////////////////
#[derive(Debug)]
pub struct IndexedArray<T> {
    allocator: Rc<RefCell<Allocator>>,
    list: Vec<Option<Entry<T>>>,
}

#[derive(Debug, Default)]
pub struct Entry<T> {
    value: T,
    version: u64,
}

impl<T> IndexedArray<T> {
    pub(super) fn new(allocator: Rc<RefCell<Allocator>>) -> Self {
        Self {
            allocator,
            list: Vec::<Option<Entry<T>>>::with_capacity(DEFAULT_CAPACITY),
        }
    }

    pub(super) fn set(&mut self, index: &Index, value: T) {
        let i = index.index;

        if i >= self.list.capacity() {
            self.list.reserve(self.list.capacity() * 2);
        }

        if i >= self.list.len() {
            self.list.resize_with(i + 4, || None);
        }

        self.list[i] = Some(Entry {
            version: index.version,
            value,
        });
    }

    pub fn unset(&mut self, index: &Index) {
        let i = index.index;

        if i >= self.list.len() {
            return;
        }

        self.list[i] = None;
    }

    pub(super) fn get(&self, index: &Index) -> Option<&T> {
        match self.list.get(index.index) {
            Some(Some(entry)) => {
                if entry.version == index.version {
                    Some(&entry.value)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(super) fn get_mut(&mut self, index: &Index) -> Option<&mut T> {
        match self.list.get_mut(index.index) {
            None => None,
            Some(None) => None,
            Some(Some(entry)) => {
                if entry.version == index.version {
                    Some(&mut entry.value)
                } else {
                    None
                }
            }
        }
    }

    pub(super) fn get_entities(&self) -> Vec<Index> {
        self.list
            .iter()
            .enumerate()
            .filter_map(|(i, wrapped)| match wrapped {
                Some(entry) => {
                    let index = Index {
                        index: i,
                        version: entry.version,
                    };

                    match self.allocator.borrow().validate(&index) {
                        true => Some(index),
                        false => None,
                    }
                }
                None => None,
            })
            .collect()
    }

    pub fn iter(&self) -> Iter<T> {
        Iter::<T>::new(self.allocator.clone(), self.list.iter().enumerate())
    }

    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut::<T>::new(self.allocator.clone(), self.list.iter_mut().enumerate())
    }
}

/// /////////////////////////////////
///
/// Iterator for indexed array
///
/// /////////////////////////////////
pub struct Iter<'a, T: 'a> {
    remaining: usize,
    allocator: Rc<RefCell<Allocator>>,
    inner: iter::Enumerate<slice::Iter<'a, Option<Entry<T>>>>,
}

impl<'a, T> Iter<'a, T> {
    pub fn new(
        allocator: Rc<RefCell<Allocator>>,
        inner: iter::Enumerate<slice::Iter<'a, Option<Entry<T>>>>,
    ) -> Self {
        Self {
            remaining: inner.len(),
            allocator,
            inner,
        }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Index, Option<&'a T>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some((i, entry)) = self.inner.next() else {
                debug_assert_eq!(self.remaining, 0);

                return None;
            };

            self.remaining -= 1;

            let Some(index) = self.allocator.borrow().index_at(i) else {
                continue;
            };

            return Some((
                index,
                match entry {
                    Some(entry) => Some(&entry.value),
                    _ => None,
                },
            ));
        }
    }
}

/// /////////////////////////////////
///
/// mutable Iterator for indexed array
///
/// /////////////////////////////////
pub struct IterMut<'a, T: 'a> {
    remaining: usize,
    allocator: Rc<RefCell<Allocator>>,
    inner: iter::Enumerate<slice::IterMut<'a, Option<Entry<T>>>>,
}

impl<'a, T> IterMut<'a, T> {
    pub fn new(
        allocator: Rc<RefCell<Allocator>>,
        inner: iter::Enumerate<slice::IterMut<'a, Option<Entry<T>>>>,
    ) -> Self {
        Self {
            remaining: inner.len(),
            allocator,
            inner,
        }
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Index, Option<&'a mut T>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some((i, entry)) = self.inner.next() else {
                debug_assert_eq!(self.remaining, 0);

                return None;
            };

            self.remaining -= 1;

            let Some(index) = self.allocator.borrow().index_at(i) else {
                continue;
            };

            return Some((
                index,
                match entry {
                    Some(entry) => Some(&mut entry.value),
                    _ => None,
                },
            ));
        }
    }
}

/// ///////////////////////////////
///
/// Tests
///
/// ///////////////////////////////
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Entity(String);

    type EntityMap<T> = IndexedArray<T>;

    #[test]
    fn indexed_array_getting_setting_removing() {
        let allocator = Rc::new(RefCell::new(Allocator::default()));
        let mut entities = EntityMap::<Entity>::new(allocator.clone());

        let player_id = allocator.borrow_mut().allocate();
        let npc_id = allocator.borrow_mut().allocate();
        let enemy_id = allocator.borrow_mut().allocate();

        entities.set(&player_id, Entity("player".to_string()));
        entities.set(&npc_id, Entity("npc".to_string()));
        entities.set(&enemy_id, Entity("enemy".to_string()));

        assert_eq!(allocator.borrow().len(), 3);

        let mut bullets = Vec::<Index>::new();
        for _ in 0..3 {
            let bullet = allocator.borrow_mut().allocate();

            entities.set(&bullet, Entity("bullet".to_string()));

            bullets.push(bullet);
        }

        assert_eq!(allocator.borrow().len(), 6);
        assert_eq!(
            entities.get(&player_id),
            Some(&Entity("player".to_string()))
        );
        assert_eq!(entities.get(&enemy_id), Some(&Entity("enemy".to_string())));
        assert_eq!(entities.get(&npc_id), Some(&Entity("npc".to_string())));

        // npc_id is no longer valid after this call because the remove function destroys it
        allocator.borrow_mut().deallocate(npc_id);

        assert_eq!(allocator.borrow().valid_count(), 5);

        for bullet in bullets {
            allocator.borrow_mut().deallocate(bullet);
        }

        assert_eq!(allocator.borrow().valid_count(), 2);
        assert_eq!(
            entities.get(&player_id),
            Some(&Entity("player".to_string()))
        );
        assert_eq!(entities.get(&enemy_id), Some(&Entity("enemy".to_string())));
    }

    #[test]
    fn indexed_array_versioning() {
        let allocator = Rc::new(RefCell::new(Allocator::default()));
        let mut entities = EntityMap::<Entity>::new(allocator.clone());

        let player_id = allocator.borrow_mut().allocate();
        let npc_id = allocator.borrow_mut().allocate();
        let enemy_id = allocator.borrow_mut().allocate();

        entities.set(&player_id, Entity("player".to_string()));
        entities.set(&npc_id, Entity("npc".to_string()));
        entities.set(&enemy_id, Entity("enemy".to_string()));

        assert_eq!(
            entities.get(&Index {
                index: 1,
                version: 0
            }),
            Some(&Entity("npc".to_string()))
        );

        allocator.borrow_mut().deallocate(npc_id);

        // used to hold npc
        assert!(!allocator.borrow().is_allocated(&Index {
            index: 1,
            version: 0
        }));

        let npc_id = allocator.borrow_mut().allocate();
        entities.set(&npc_id, Entity("npc".to_string()));

        assert_eq!(
            entities.get(&Index {
                index: 1,
                version: 1
            }),
            Some(&Entity("npc".to_string()))
        );

        assert_eq!(
            entities.get(&Index {
                index: 1,
                version: 0
            }),
            None
        );

        // version 1 is allocated while version 0 is not
        assert!(!allocator.borrow().is_allocated(&Index {
            index: 1,
            version: 0
        }));
        assert!(allocator.borrow().is_allocated(&Index {
            index: 1,
            version: 1
        }));
    }

    #[test]
    fn iterate_over_array() {
        #[derive(Debug, PartialEq)]
        struct Container(pub u32);

        let allocator = Rc::new(RefCell::new(Allocator::with_capacity(4)));
        let mut array = IndexedArray::<Container>::new(allocator.clone());

        let i1 = allocator.borrow_mut().allocate();
        let i2 = allocator.borrow_mut().allocate();

        array.set(&i1, Container(123));
        array.set(&i2, Container(456));

        let mut iterator = array.iter();

        assert_eq!(iterator.next(), Some((i1, Some(&Container(123)))));
        assert_eq!(iterator.next(), Some((i2, Some(&Container(456)))));

        allocator.borrow_mut().deallocate(i1);

        let mut iterator = array.iter();

        assert_eq!(iterator.next(), Some((i2, Some(&Container(456)))));
        assert_eq!(iterator.next(), None);

        for (_, cont) in array.iter_mut() {
            if let Some(val) = cont {
                val.0 = 457
            }
        }

        let mut iterator = array.iter();

        assert_eq!(iterator.next(), Some((i2, Some(&Container(457)))));
        assert_eq!(iterator.next(), None);
    }
}
