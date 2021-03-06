// Copyright 2017 Matthew Plant. This file is part of MGF.
//
// MGF is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// MGF is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with MGF. If not, see <http://www.gnu.org/licenses/>.

use std::mem;
use std::slice;
use std::iter::{Enumerate, FilterMap};
use std::ops::{Index, IndexMut};
use std::vec::Vec;

use serde::{Serialize, Deserialize};

/// Internal storage type used by Pool.
#[derive(Serialize, Deserialize)]
pub enum PoolEntry<T> {
    FreeListEnd,
    FreeListPtr {
        next_free: usize,
    },
    Occupied(T)
}

/// Growable array type that allows items to be removed and inserted without
/// changing the indices of other entries.
#[derive(Serialize, Deserialize)]
pub struct Pool<T> {
    len: usize,
    free_list: Option<usize>,
    entries: Vec<PoolEntry<T>>,
}

impl<T> Pool<T> {
    /// Create an empty Pool.
    pub fn new() -> Self {
        Pool {
            len: 0,
            free_list: None,
            entries: Vec::new(),
        }
    }

    /// Create an empty Pool large enough to fix cap items.
    pub fn with_capacity(cap: usize) -> Self {
        Pool {
            len: 0,
            free_list: None,
            entries: Vec::with_capacity(cap),
        }
    }

    /// Determines if the Pool is empty.
    pub fn empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of occupied slots in the pool.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Removes all entries from the pool.
    pub fn clear(&mut self) {
        self.len = 0;
        self.free_list = None;
        self.entries.clear();
    }

    /// Push a new item to the pool. Attempts to use spots left empty from
    /// removed items before performing a heap allocation.
    pub fn push(&mut self, item: T) -> usize {
        self.len += 1;
        if let Some(free_item) = self.free_list {
            self.free_list = match self.entries[free_item] {
                PoolEntry::FreeListEnd => None,
                PoolEntry::FreeListPtr{ next_free } => Some(next_free),
                _ => unreachable!(),
            };
            self.entries[free_item] = PoolEntry::Occupied(item);
            free_item
        } else {
            let i = self.entries.len();
            self.entries.push(PoolEntry::Occupied(item));
            i
        }
    }

    /// Marks an index as empty and adds it to the free list, allowing the
    /// spot to be reclaimed later.
    pub fn remove(&mut self, i: usize) -> T {
        let new_entry = if let Some(free_item) = self.free_list {
                PoolEntry::FreeListPtr{ next_free: free_item } 
        } else {
                PoolEntry::FreeListEnd
        };
        self.free_list = Some(i);
        if let PoolEntry::Occupied(item) = mem::replace(&mut self.entries[i], new_entry) {
            self.len -= 1;
            item
        } else {
            panic!("index {} is not occupied", i);
        }
    }

    /// Returns the next available id for reuse if one exists.
    pub fn next_free(&self) -> Option<usize> {
        if let Some(free) = self.free_list {
            Some(free)
        } else {
            None
        }
    }
    
    pub fn iter<'a>(&'a self) -> impl Iterator<Item=(usize, &'a T)> {
        self.into_iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item=(usize, &'a mut T)> { 
        self.into_iter()
    }

    /// Returns a reference to an object at the given index and None if it is
    /// unoccupied.
    pub fn get<'a>(&'a self, i: usize) -> Option<&'a T> {
        if let Some(PoolEntry::Occupied(ref item)) = self.entries.get(i) {
            Some(&item)
        } else {
            None
        }
    }

    /// Returns a mutable reference to an object at the given index and None if
    /// it is unoccupied.
    pub fn get_mut<'a>(&'a mut self, i: usize) -> Option<&'a mut T> {
        if let Some(PoolEntry::Occupied(ref mut item)) = self.entries.get_mut(i) {
            Some(item)
        } else {
            None
        }
    }
}
        
impl<T> Index<usize> for Pool<T> {
    type Output = T;

    fn index(&self, i: usize) -> &T {
        if let PoolEntry::Occupied(ref item) = self.entries[i] {
            item
        } else {
            panic!("index {} is not occupied", i)
        }
    }
}

impl<T> IndexMut<usize> for Pool<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        if let PoolEntry::Occupied(ref mut item) = self.entries[i] {
            item
        } else {
            panic!("index {} is not occupied", i)
        }
    }
}

impl<T> Clone for PoolEntry<T>
where
    T: Clone
{
    fn clone(&self) -> Self {
        use self::PoolEntry::*;
        match self {
            &FreeListEnd => FreeListEnd,
            &FreeListPtr{ next_free } => FreeListPtr{ next_free },
            &Occupied(ref item) => Occupied(item.clone()),
        }
    }
}

impl<T> Clone for Pool<T>
where
    T: Clone
{
    fn clone(&self) -> Self {
        Pool {
            len: self.len,
            free_list: self.free_list,
            entries: self.entries.clone(),
        }
    }
}

impl<L, T> From<L> for Pool<T>
where
    L: IntoIterator<Item = T>
{
    fn from(iter: L) -> Pool<T> {
        let mut pool = Pool::new();
        for item in iter.into_iter() {
            pool.push(item);
        }
        pool
    }
}

fn filter_pool<'a, T>((i, item): (usize, &'a PoolEntry<T>)) -> Option<(usize, &'a T)> {
    if let &PoolEntry::Occupied(ref item) = item {
        Some((i, item))
    } else {
        None
    }
}

impl<'a, T> IntoIterator for &'a Pool<T> {
    type Item = (usize, &'a T);
    type IntoIter = FilterMap<Enumerate<slice::Iter<'a, PoolEntry<T>>>, fn((usize, &PoolEntry<T>)) -> Option<(usize, &T)>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().enumerate().filter_map(filter_pool)
    }
}

fn filter_pool_mut<'a, T>((i, item): (usize, &'a mut PoolEntry<T>)) -> Option<(usize, &'a mut T)> {
    if let &mut PoolEntry::Occupied(ref mut item) = item {
        Some((i, item))
    } else {
        None
    }
}

impl<'a, T> IntoIterator for &'a mut Pool<T> {
    type Item = (usize, &'a mut T);
    type IntoIter = FilterMap<Enumerate<slice::IterMut<'a, PoolEntry<T>>>, fn((usize, &mut PoolEntry<T>)) -> Option<(usize, &mut T)>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter_mut().enumerate().filter_map(filter_pool_mut)
    }
}

#[cfg(test)]
mod tests {
    mod pool {
        use crate::pool::*;
 
        #[test]
        fn test_manual_code() {
            let mut pool: Pool<usize> = Pool::new();

            let id0 = pool.push(0);
            let id1 = pool.push(1);
            let id2 = pool.push(2);
            let id3 = pool.push(3);

            assert_eq!(id0, 0);
            assert_eq!(id3, 3);

            pool.remove(id1);
            pool.remove(id2);

            assert_eq!(pool[id0], 0);
            assert_eq!(pool[id3], 3);

            assert_eq!(pool.iter().map(|(_i, &u)|{u}).collect::<Vec<usize>>(), vec![0, 3]);
        }

        #[test]
        fn test_pool() {
            // Test inserting 8 items
            {
                let mut pool: Pool<usize> = Pool::new();
                for i in 0..8 {
                    pool.push(i);
                }
                let ids = [ 0, 1, 2, 3, 4, 5, 6, 7 ];
                for (i, item) in pool.iter().enumerate() {
                    assert_eq!(*item.1, ids[i]);
                }
                // Remove every other item
                for i in 0..4 {
                    pool.remove(i * 2);
                }
                let ids = [ 1, 3, 5, 7 ];
                for (i, item) in pool.iter().enumerate() {
                    assert_eq!(*item.1, ids[i]);
                }
                {
                    let _removed = pool.remove(1);
                    let ids = [ 3, 5, 7 ];
                    for (i, item) in pool.iter().enumerate() {
                        assert_eq!(*item.1, ids[i]);
                    }
                }
            }
            // Test inserting 16 items
            {
                let mut pool: Pool<usize> = Pool::new();
                for i in 0..16 {
                    pool.push(i);
                }
                let ids = [ 0, 1, 2, 3, 4, 5, 6, 7,
                            8, 9, 10, 11, 12, 13, 14, 15 ];
                for (i, item) in pool.iter().enumerate() {
                    assert_eq!(*item.1, ids[i]);
                }
                // Remove every other item
                for i in 0..8 {
                    pool.remove(i * 2);
                }
                let ids = [ 1, 3, 5, 7, 9, 11, 13, 15 ];
                for (i, item) in pool.iter().enumerate() {
                    assert_eq!(*item.1, ids[i]);
                }
                {
                    let _removed = pool.remove(1);
                    let ids = [ 3, 5, 7, 9, 11, 13, 15 ];
                    for (i, item) in pool.iter().enumerate() {
                        assert_eq!(*item.1, ids[i]);
                    }
                }
            }
            // Test inserting 16 items and removing the first 8.
            {

                let mut pool: Pool<usize> = Pool::new();
                for i in 0..16 {
                    pool.push(i);
                }
                let ids = [ 0, 1, 2, 3, 4, 5, 6, 7,
                            8, 9, 10, 11, 12, 13, 14, 15 ];
                for (i, item) in pool.iter().enumerate() {
                    assert_eq!(*item.1, ids[i]);
                }
                for i in 0..8 {
                    pool.remove(i);
                }
                let ids = [ 8, 9, 10, 11, 12, 13, 14, 15 ];
                for (i, item) in pool.iter().enumerate() {
                    assert_eq!(*item.1, ids[i]);
                }
                {
                    let _removed = pool.remove(8);
                    let ids = [ 9, 10, 11, 12, 13, 14, 15 ];
                    for (i, item) in pool.iter().enumerate() {
                        assert_eq!(*item.1, ids[i]);
                    }
                }
            }
            // Test inserting 24 items and removing the middle 8.
            {

                let mut pool: Pool<usize> = Pool::new();
                for i in 0..24 {
                    pool.push(i);
                }
                let ids = [ 0, 1, 2, 3, 4, 5, 6, 7,
                            8, 9, 10, 11, 12, 13, 14, 15,
                            16, 17, 18, 19, 20, 21, 22, 23 ];
                for (i, item) in pool.iter().enumerate() {
                    assert_eq!(*item.1, ids[i]);
                }
                for i in 8..16 {
                    pool.remove(i);
                }
                let ids = [ 0, 1, 2, 3, 4, 5, 6, 7, 
                            16, 17, 18, 19, 20, 21, 22, 23 ];
                for (i, item) in pool.iter().enumerate() {
                    assert_eq!(*item.1, ids[i]);
                }
                {
                    let _removed1 = pool.remove(23);
                    let _removed2 = pool.remove(18);
                    let _removed2 = pool.remove(19);
                    let ids = [ 0, 1, 2, 3, 4, 5, 6, 7,
                                16, 17, 20, 21, 22 ];
                    for (i, item) in pool.iter().enumerate() {
                        assert_eq!(*item.1, ids[i]);
                    }
                }
            }
        }
    }
}
