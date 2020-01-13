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

pub struct Ptr<T> {
    ptr: NonNull<Entry<T>>,
    pool_id: *const (),
}

#[derive(Debug, Clone, Copy)]
pub struct Ref<'a, T> {
    value: &'a T,
    entry: &'a Entry<T>,
    pool_id: *const (),
}
impl<'a, T> Ref<'a, T> {
    pub fn get(&self) -> &'a T {
        self.value
    }
}
impl<'a, T> Deref for Ref<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<'a, T> From<Ref<'a, T>> for Ptr<T> {
    fn from(src: Ref<'a, T>) -> Self {
        Ptr {
            ptr: src.entry.into(),
            pool_id: src.pool_id,
        }
    }
}

impl<T> Ptr<T> {
    pub const fn dangling() -> Self {
        Ptr {
            ptr: NonNull::dangling(),
            pool_id: std::ptr::null(),
        }
    }
    pub unsafe fn as_ref(&self) -> Option<Ref<T>> {
        let entry = &*self.ptr.as_ptr();
        match entry {
            Entry::Occupied(value) => Some(Ref {
                value,
                entry,
                pool_id: self.pool_id,
            }),
            _ => None,
        }
    }
    pub unsafe fn as_mut(&self) -> Option<&mut T> {
        match &mut *self.ptr.as_ptr() {
            Entry::Occupied(value) => Some(value),
            _ => None,
        }
    }
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

    pub fn insert(&mut self, value: T) -> Ptr<T> {
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
        Ptr {
            ptr: vacant,
            pool_id: self.id.deref() as *const (),
        }
    }

    pub fn remove(&mut self, mut h: Ptr<T>) -> bool {
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

    pub fn get<'a>(&self, h: &'a Ptr<T>) -> Option<Ref<'a, T>> {
        assert!(h.pool_id == self.id());
        unsafe { h.as_ref() }
    }

    pub unsafe fn get_unsafe<'a>(&self, h: &'a Ptr<T>) -> Option<&'a mut T> {
        assert!(h.pool_id == self.id());
        h.as_mut()
    }

    pub fn get_mut<'a>(&mut self, h: &'a Ptr<T>) -> Option<&'a mut T> {
        unsafe { self.get_unsafe(h) }
    }
}

impl<T> std::default::Default for Pool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> std::fmt::Debug for Ptr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Ptr {{ ptr: {:?}, pool_id: {:?} }}",
            self.ptr, self.pool_id
        )
    }
}
impl<T> Clone for Ptr<T> {
    fn clone(&self) -> Self {
        Ptr {
            ptr: self.ptr,
            pool_id: self.pool_id,
        }
    }
}
impl<T> PartialEq for Ptr<T> {
    fn eq(&self, rhs: &Self) -> bool {
        self.ptr == rhs.ptr && self.pool_id == rhs.pool_id
    }
}
impl<T> std::hash::Hash for Ptr<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr.hash(state)
    }
}
impl<T> PartialOrd for Ptr<T> {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        self.ptr.partial_cmp(&rhs.ptr)
    }
}
impl<T> Ord for Ptr<T> {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        self.ptr.cmp(&rhs.ptr)
    }
}
impl<T> Copy for Ptr<T> {}
impl<T> Eq for Ptr<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_insert_and_remove() {
        let mut pool = Pool::new();
        let ptr = pool.insert(3.14);
        assert_eq!(*pool.get(&ptr).unwrap(), 3.14);
        *pool.get_mut(&ptr).unwrap() = 2.7;
        assert_eq!(*pool.get(&ptr).unwrap(), 2.7);
        assert!(pool.remove(ptr));
        assert!(pool.get(&ptr).is_none());
    }

    #[test]
    fn insert_many() {
        let mut pool = Pool::new();
        let mut ptrs = Vec::new();
        for i in 0..1024 {
            ptrs.push(pool.insert(i));
        }
        assert_eq!(pool.blocks.len(), 4);
        assert_eq!(10, *pool.get(&ptrs[10]).unwrap());
        assert_eq!(20, *pool.get(&ptrs[20]).unwrap());
        assert_eq!(300, *pool.get(&ptrs[300]).unwrap());
        assert!(pool.remove(ptrs[30]));
        assert!(pool.get(&ptrs[30]).is_none());
        let h = pool.insert(1111);
        assert_eq!(h, ptrs[30]);
        assert_eq!(pool.blocks.len(), 4);
        pool.insert(2222);
        assert_eq!(pool.blocks.len(), 5);
    }

    struct Node {
        next: Option<Ptr<Node>>,
        prev: Option<Ptr<Node>>,
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
        pool.get_mut(&h1).unwrap().next = Some(h2);
        pool.get_mut(&h2).unwrap().prev = Some(h1);

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
            pool.get_unsafe(&h1).unwrap().next = pool.get(&h2).as_ref().map(Deref::deref);
            pool.get_unsafe(&h2).unwrap().prev = pool.get(&h1).as_ref().map(Deref::deref);
        }

        let mut map = std::collections::HashSet::new();
        map.insert(h1);
        map.insert(h2);

        let mut tree = std::collections::BTreeSet::new();
        tree.insert(h1);
        tree.insert(h2);
    }

    /*
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

        let r = pool.get(b).as_ref().map(Deref::deref);
        pool.get(a).unwrap().other.set(r);
        pool.get(b)
            .unwrap()
            .other
            .set(pool.get(a).as_ref().map(Deref::deref));
        //pool.remove(a);
    }
    */
}
