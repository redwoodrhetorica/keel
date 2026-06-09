//! Generational typed arena. Slots are transient memory addresses,
//! NEVER identity: `EntityId` is identity (spec D9, kernel/14 doctrine).
//! Iteration over an arena is for internal traversal only; anything
//! that influences output iterates in EntityId order via `Body::ids`.

use core::marker::PhantomData;

/// Typed generational key into an [`Arena<T>`].
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
pub struct Key<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

// Manual impls so `T` need not satisfy any bounds.
impl<T> Clone for Key<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Key<T> {}
impl<T> PartialEq for Key<T> {
    fn eq(&self, o: &Self) -> bool {
        self.index == o.index && self.generation == o.generation
    }
}
impl<T> Eq for Key<T> {}
impl<T> PartialOrd for Key<T> {
    fn partial_cmp(&self, o: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl<T> Ord for Key<T> {
    fn cmp(&self, o: &Self) -> core::cmp::Ordering {
        (self.index, self.generation).cmp(&(o.index, o.generation))
    }
}
impl<T> core::hash::Hash for Key<T> {
    fn hash<H: core::hash::Hasher>(&self, h: &mut H) {
        self.index.hash(h);
        self.generation.hash(h);
    }
}
impl<T> core::fmt::Debug for Key<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Key({}g{})", self.index, self.generation)
    }
}

impl<T> Key<T> {
    #[inline]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Crate-private sentinel that can never resolve (no arena reaches
    /// u32::MAX slots). Exists only transiently during construction of
    /// ring links; never present in a validated body.
    pub(crate) fn sentinel() -> Self {
        Key {
            index: u32::MAX,
            generation: u32::MAX,
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
enum Slot<T> {
    Occupied { generation: u32, value: T },
    Free { generation: u32 },
}

/// Deterministic generational arena (LIFO free list).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn insert(&mut self, value: T) -> Key<T> {
        self.len += 1;
        if let Some(i) = self.free.pop() {
            let generation = match self.slots[i as usize] {
                Slot::Free { generation } => generation + 1,
                Slot::Occupied { .. } => unreachable!("free list corrupt"),
            };
            self.slots[i as usize] = Slot::Occupied { generation, value };
            Key {
                index: i,
                generation,
                _marker: PhantomData,
            }
        } else {
            let i = self.slots.len() as u32;
            self.slots.push(Slot::Occupied {
                generation: 0,
                value,
            });
            Key {
                index: i,
                generation: 0,
                _marker: PhantomData,
            }
        }
    }

    pub fn get(&self, k: Key<T>) -> Option<&T> {
        match self.slots.get(k.index as usize) {
            Some(Slot::Occupied { generation, value }) if *generation == k.generation => {
                Some(value)
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, k: Key<T>) -> Option<&mut T> {
        match self.slots.get_mut(k.index as usize) {
            Some(Slot::Occupied { generation, value }) if *generation == k.generation => {
                Some(value)
            }
            _ => None,
        }
    }

    pub fn contains(&self, k: Key<T>) -> bool {
        self.get(k).is_some()
    }

    pub fn remove(&mut self, k: Key<T>) -> Option<T> {
        match self.slots.get_mut(k.index as usize) {
            Some(slot @ Slot::Occupied { .. }) => {
                let generation = match slot {
                    Slot::Occupied { generation, .. } if *generation == k.generation => *generation,
                    _ => return None,
                };
                let old = core::mem::replace(slot, Slot::Free { generation });
                self.free.push(k.index);
                self.len -= 1;
                match old {
                    Slot::Occupied { value, .. } => Some(value),
                    Slot::Free { .. } => unreachable!(),
                }
            }
            _ => None,
        }
    }

    /// Internal traversal only; order is allocation-history-dependent.
    pub fn iter(&self) -> impl Iterator<Item = (Key<T>, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| match s {
            Slot::Occupied { generation, value } => Some((
                Key {
                    index: i as u32,
                    generation: *generation,
                    _marker: PhantomData,
                },
                value,
            )),
            Slot::Free { .. } => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_generation_protects_against_stale_keys() {
        let mut a: Arena<u32> = Arena::new();
        let k = a.insert(7);
        assert_eq!(a.remove(k), Some(7));
        let k2 = a.insert(9);
        assert_eq!(k.index(), k2.index()); // slot reused
        assert!(a.get(k).is_none()); // stale generation rejected
        assert_eq!(a.get(k2), Some(&9));
        assert_eq!(a.remove(k), None); // stale remove rejected too
        assert_eq!(a.len(), 1);
    }

    #[test]
    fn iteration_skips_free_slots() {
        let mut a: Arena<i32> = Arena::new();
        let k0 = a.insert(0);
        let _k1 = a.insert(1);
        let _k2 = a.insert(2);
        a.remove(k0);
        let vals: Vec<i32> = a.iter().map(|(_, v)| *v).collect();
        assert_eq!(vals, vec![1, 2]);
    }
}
