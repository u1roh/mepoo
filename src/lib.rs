use std::ops::Deref;
use std::ptr::NonNull;

enum Entry<T> {
    Vacant(Option<NonNull<Self>>),
    Occupied(T),
}

impl<T> Entry<T> {
    fn new_block(
        capacity: usize,
        mut next_vacant: Option<NonNull<Self>>,
    ) -> (NonNull<Self>, Box<[Self]>) {
        let mut block = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            block.push(Entry::Vacant(next_vacant));
            next_vacant = NonNull::new(block.last_mut().unwrap() as *mut _);
        }
        (next_vacant.unwrap(), block.into_boxed_slice())
    }
}

pub struct Pool<T> {
    blocks: Vec<Box<[Entry<T>]>>,
    next_vacant: Option<NonNull<Entry<T>>>,
    id: Box<()>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Handle<T> {
    ptr: NonNull<Entry<T>>,
    pool_id: *const (),
}

impl<T> Pool<T> {
    const BLOCK_SIZE: usize = 256;

    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            next_vacant: None,
            id: Box::new(()),
        }
    }

    pub fn id(&self) -> *const () {
        self.id.deref() as *const ()
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        let mut next_vacant = if let Some(next_vacant) = self.next_vacant {
            next_vacant
        } else {
            let (ptr, block) = Entry::new_block(Self::BLOCK_SIZE, None);
            self.blocks.push(block);
            self.next_vacant = Some(ptr);
            ptr
        };
        unsafe {
            self.next_vacant = match next_vacant.as_ref() {
                Entry::Vacant(ptr) => *ptr,
                _ => panic!("error"),
            };
            *next_vacant.as_mut() = Entry::Occupied(value);
        }
        Handle {
            ptr: next_vacant,
            pool_id: self.id.deref() as *const (),
        }
    }

    pub fn remove(&mut self, mut h: Handle<T>) -> bool {
        if h.pool_id != self.id() {
            panic!("pool id not matched");
        }
        unsafe {
            match h.ptr.as_mut() {
                Entry::Vacant(_) => false,
                _ => {
                    *h.ptr.as_mut() = Entry::Vacant(self.next_vacant);
                    self.next_vacant = Some(h.ptr);
                    true
                }
            }
        }
    }

    pub fn get(&self, h: Handle<T>) -> Option<&T> {
        unimplemented!()
    }

    pub fn get_mut(&mut self, h: Handle<T>) -> Option<&mut T> {
        unimplemented!()
    }
}

mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
