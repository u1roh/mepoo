use std::ops::Deref;
use std::ptr::NonNull;

#[derive(Debug)]
enum Entry<T> {
    Vacant(Option<NonNull<Self>>),
    Occupied(T),
}

/// A memory pool of objects of type `T`.
/// This is similar to typed_arena excepting that `Pool` can deallocate each object individually by `remove` method.
#[derive(Debug)]
pub struct Pool<T> {
    blocks: Vec<Box<[Entry<T>]>>,
    vacant: Option<NonNull<Entry<T>>>,
    id: Box<()>,
}

pub struct Handle<T> {
    ptr: NonNull<Entry<T>>,
    pool_id: *const (),
}

impl<T> Pool<T> {
    const BLOCK_SIZE: usize = 256;

    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            vacant: None,
            id: Box::new(()),
        }
    }

    pub fn id(&self) -> *const () {
        self.id.deref() as *const ()
    }

    fn new_block() -> (NonNull<Entry<T>>, Box<[Entry<T>]>) {
        let mut block = Vec::with_capacity(Self::BLOCK_SIZE);
        let mut vacant = None;
        for _ in 0..Self::BLOCK_SIZE {
            block.push(Entry::Vacant(vacant));
            vacant = NonNull::new(block.last_mut().unwrap() as *mut _);
        }
        (vacant.unwrap(), block.into_boxed_slice())
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        let mut vacant = if let Some(vacant) = self.vacant {
            vacant
        } else {
            let (ptr, block) = Self::new_block();
            self.blocks.push(block);
            self.vacant = Some(ptr);
            ptr
        };
        unsafe {
            self.vacant = match vacant.as_ref() {
                Entry::Vacant(ptr) => *ptr,
                _ => panic!("error"),
            };
            *vacant.as_mut() = Entry::Occupied(value);
        }
        Handle {
            ptr: vacant,
            pool_id: self.id.deref() as *const (),
        }
    }

    pub fn remove(&mut self, mut h: Handle<T>) -> bool {
        assert!(h.pool_id == self.id());
        unsafe {
            match h.ptr.as_mut() {
                Entry::Vacant(_) => false,
                _ => {
                    *h.ptr.as_mut() = Entry::Vacant(self.vacant);
                    self.vacant = Some(h.ptr);
                    true
                }
            }
        }
    }

    pub fn get(&self, h: Handle<T>) -> Option<&T> {
        assert!(h.pool_id == self.id());
        unsafe {
            match &*h.ptr.as_ptr() {
                Entry::Occupied(value) => Some(value),
                _ => None,
            }
        }
    }

    pub unsafe fn get_unsafe(&self, h: Handle<T>) -> Option<&mut T> {
        assert!(h.pool_id == self.id());
        match &mut *h.ptr.as_ptr() {
            Entry::Occupied(value) => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, h: Handle<T>) -> Option<&mut T> {
        unsafe { self.get_unsafe(h) }
    }
}

impl<T> std::default::Default for Pool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Handle {{ ptr: {:?}, pool_id: {:?} }}",
            self.ptr, self.pool_id
        )
    }
}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            pool_id: self.pool_id,
        }
    }
}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, rhs: &Self) -> bool {
        self.ptr == rhs.ptr && self.pool_id == rhs.pool_id
    }
}
impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr.hash(state)
    }
}
impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        self.ptr.partial_cmp(&rhs.ptr)
    }
}
impl<T> Ord for Handle<T> {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        self.ptr.cmp(&rhs.ptr)
    }
}
impl<T> Copy for Handle<T> {}
impl<T> Eq for Handle<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_insert_and_remove() {
        let mut pool = Pool::new();
        let h = pool.insert(3.14);
        assert_eq!(*pool.get(h).unwrap(), 3.14);
        *pool.get_mut(h).unwrap() = 2.7;
        assert_eq!(*pool.get(h).unwrap(), 2.7);
        assert!(pool.remove(h));
        assert!(pool.get(h).is_none());
    }

    #[test]
    fn insert_many() {
        let mut pool = Pool::new();
        let mut handles = Vec::new();
        for i in 0..1024 {
            handles.push(pool.insert(i));
        }
        assert_eq!(pool.blocks.len(), 4);
        assert_eq!(10, *pool.get(handles[10]).unwrap());
        assert_eq!(20, *pool.get(handles[20]).unwrap());
        assert_eq!(300, *pool.get(handles[300]).unwrap());
        assert!(pool.remove(handles[30]));
        assert!(pool.get(handles[30]).is_none());
        let h = pool.insert(1111);
        assert_eq!(h, handles[30]);
        assert_eq!(pool.blocks.len(), 4);
        pool.insert(2222);
        assert_eq!(pool.blocks.len(), 5);
    }

    struct Node {
        next: Option<Handle<Node>>,
        prev: Option<Handle<Node>>,
    }

    #[test]
    fn graph() {
        let mut pool = Pool::new();
        let h1 = pool.insert(Node {
            next: None,
            prev: None,
        });
        let h2 = pool.insert(Node {
            next: None,
            prev: None,
        });
        assert_ne!(h1, h2);
        pool.get_mut(h1).unwrap().next = Some(h2);
        pool.get_mut(h2).unwrap().prev = Some(h1);

        let mut map = std::collections::HashSet::new();
        map.insert(h1);
        map.insert(h2);

        let mut tree = std::collections::BTreeSet::new();
        tree.insert(h1);
        tree.insert(h2);
    }

    struct Node2<'a> {
        next: Option<&'a Node2<'a>>,
        prev: Option<&'a Node2<'a>>,
    }

    #[test]
    fn graph2() {
        let mut pool = Pool::new();
        let h1 = pool.insert(Node2 {
            next: None,
            prev: None,
        });
        let h2 = pool.insert(Node2 {
            next: None,
            prev: None,
        });
        assert_ne!(h1, h2);
        unsafe {
            pool.get_unsafe(h1).unwrap().next = pool.get(h2);
            pool.get_unsafe(h2).unwrap().prev = pool.get(h1);
        }

        let mut map = std::collections::HashSet::new();
        map.insert(h1);
        map.insert(h2);

        let mut tree = std::collections::BTreeSet::new();
        tree.insert(h1);
        tree.insert(h2);
    }

    use std::cell::Cell;

    struct Node3<'a> {
        other: Cell<Option<&'a Node3<'a>>>,
    }

    #[test]
    fn graph3() {
        let mut pool = Pool::new();

        let a = pool.insert(Node3 {
            other: Cell::new(None),
        });
        let b = pool.insert(Node3 {
            other: Cell::new(None),
        });

        pool.get(a).unwrap().other.set(pool.get(b));
        pool.get(b).unwrap().other.set(pool.get(a));
        //pool.remove(a);
    }
}
