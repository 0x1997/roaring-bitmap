//! An implementation of the Roaring Bitmap
//!
//! * Samy Chambi, Daniel Lemire, Owen Kaser, Robert Godin,
//! [Better bitmap performance with Roaring bitmaps]
//! (http://arxiv.org/abs/1402.6407), in preparation

#![feature(iter_arith)]

extern crate bit_set;

use bit_set::BitSet;

const SPARSE_CHUNK_SIZE_LIMIT: usize = 4096;

// 8kB
enum Container {
    // 2**16 bitmap
    Dense(BitSet),
    // no more than 4096 16-bit integers
    Sparse(Vec<u16>),
}

impl Container {
    fn from_sparse_chunk(from: &Vec<u16>) -> Container {
        Container::Dense(
            from.iter().map(|&val: &u16| val as usize).collect::<BitSet>()
        )
    }

    fn from_dense_chunk(from: &BitSet) -> Container {
        Container::Sparse(
            from.iter().map(|val: usize| val as u16).collect::<Vec<u16>>()
        )
    }

    fn len(&self) -> usize {
        match self {
            &Container::Dense(ref bitset) => bitset.len(),
            &Container::Sparse(ref vec) => vec.len(),
        }
    }

    fn contains(&self, value: u16) -> bool {
        match self {
            &Container::Dense(ref bitset) => bitset.contains(&(value as usize)),
            &Container::Sparse(ref vec) => vec.binary_search(&value).is_ok(),
        }
    }
}

#[inline]
fn key_val_pair(value: u32) -> (u16, u16) {
    ((value >> 16) as u16, (value & ((1 << 16) - 1)) as u16)
}

pub struct RoaringBitMap {
    keys: Vec<u16>,
    containers: Vec<Box<Container>>,
}

impl RoaringBitMap {
    /// Constructs a new `RoaringBitMap`.
    ///
    /// # Examples
    ///
    /// ```
    /// use roaring_bitmap::RoaringBitMap;
    ///
    /// let mut bitmap = RoaringBitMap::new();
    /// ```
    pub fn new() -> RoaringBitMap {
        RoaringBitMap {
            keys: Vec::new(),
            containers: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.containers.iter().map(|c| c.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn clear(&mut self) {
        self.keys.clear();
        self.containers.clear();
    }

    pub fn contains(&self, value: u32) -> bool {
        let (key, val) = key_val_pair(value);
        match self.keys.binary_search(&key) {
            Ok(i) => self.containers[i].contains(val),
            Err(_) => false,
        }
    }

    pub fn insert(&mut self, value: u32) -> bool {
        let (key, val) = key_val_pair(value);
        let mut new_container: Option<(usize, Box<Container>)> = None;
        let inserted = match self.keys.binary_search(&key) {
            Ok(i) => match &mut *self.containers[i] {
                &mut Container::Dense(ref mut bitset) => {
                    bitset.insert(val as usize)
                },
                &mut Container::Sparse(ref mut vec) => {
                    match vec.binary_search(&val) {
                        Ok(_) => false,
                        Err(i) => {
                            vec.insert(i, val);
                            if vec.len() == SPARSE_CHUNK_SIZE_LIMIT {
                                let c = Container::from_sparse_chunk(vec);
                                new_container = Some((i, Box::new(c)));
                            }
                            true
                        }
                    }
                }
            },
            Err(i) => {
                self.keys.insert(i, key);
                self.containers.insert(i,
                    Box::new(Container::Sparse(vec![val])));
                true
            }
        };
        if let Some((i, c)) = new_container {
            self.containers[i] = c;
        };
        inserted
    }

    pub fn remove(&mut self, value: u32) -> bool {
        let (key, val) = key_val_pair(value);
        let mut new_container: Option<(usize, Box<Container>)> = None;
        let mut to_remove: Option<usize> = None;
        let exists = match self.keys.binary_search(&key) {
            Ok(i) => match &mut *self.containers[i] {
                &mut Container::Dense(ref mut bitset) => {
                    let exists = bitset.remove(&(val as usize));
                    if bitset.len() < SPARSE_CHUNK_SIZE_LIMIT {
                        let c = Container::from_dense_chunk(bitset);
                        new_container = Some((i, Box::new(c)));
                    }
                    exists
                }
                &mut Container::Sparse(ref mut vec) => {
                    match vec.binary_search(&val) {
                        Ok(i) => {
                            vec.remove(i);
                            if vec.is_empty() {
                                to_remove = Some(i);
                            }
                            true
                        }
                        Err(_) => false,
                    }
                }
            },
            Err(_) => false,
        };
        if let Some((i, c)) = new_container {
            self.containers[i] = c;
        };
        if let Some(i) = to_remove {
            self.keys.remove(i);
            self.containers.remove(i);
        };
        exists
    }
}

#[cfg(test)]
mod tests {
    use super::RoaringBitMap;

    #[test]
    fn test_sparse() {
        let mut bitmap = RoaringBitMap::new();
        assert_eq!(bitmap.len(), 0);
        assert!(bitmap.is_empty());

        assert!(bitmap.insert(94));
        assert_eq!(bitmap.len(), 1);
        assert!(bitmap.contains(94));
        assert!(!bitmap.insert(94));

        assert!(bitmap.insert(402));
        assert_eq!(bitmap.len(), 2);

        assert!(bitmap.remove(94));
        assert_eq!(bitmap.len(), 1);

        assert!(!bitmap.remove(723));

        assert!(!bitmap.is_empty());
        bitmap.clear();
        assert!(bitmap.is_empty());
    }
}
