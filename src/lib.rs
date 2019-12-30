use std::ops::Deref;
use std::ptr::NonNull;

#[derive(Debug)]
enum Entry<T> {
    Vacant(Option<NonNull<Self>>),
    Occupied(T),
}

#[derive(Debug)]
pub struct Pool<T> {
    blocks: Vec<Box<[Entry<T>]>>,
    vacant: Option<NonNull<Entry<T>>>,
    id: Box<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    pub fn get_mut(&mut self, h: Handle<T>) -> Option<&mut T> {
        assert!(h.pool_id == self.id());
        unsafe {
            match &mut *h.ptr.as_ptr() {
                Entry::Occupied(value) => Some(value),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut pool = Pool::new();
        let h = pool.insert(3.14);
        assert_eq!(*pool.get(h).unwrap(), 3.14);
        *pool.get_mut(h).unwrap() = 2.7;
        assert_eq!(*pool.get(h).unwrap(), 2.7);
        assert!(pool.remove(h));
        assert!(pool.get(h).is_none());
    }
}
