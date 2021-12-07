
#[repr(transparent)]
pub struct NonNullConst<T: ?Sized> {
    pointer: *const T,
}


impl <T: ?Sized> NonNullConst<T> {

    pub const fn new_unchecked(ptr: *const T) -> Self {
        NonNullConst { pointer: ptr as _ }
    }

    pub fn new(ptr: *const T) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self::new_unchecked(ptr))
        }
    }

    pub fn as_ptr(self) -> *const T {
        self.pointer
    }

    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        &*self.as_ptr()
    }

}

//TODO 勉強
unsafe impl <T: ?Sized> Sync for NonNullConst<T> {}
unsafe impl <T: ?Sized> Send for NonNullConst<T> {}

impl<T: ?Sized> Clone for NonNullConst<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for NonNullConst<T> {}

impl<T: ?Sized> std::fmt::Debug for NonNullConst<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

impl<T: ?Sized> std::fmt::Pointer for NonNullConst<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

impl<T: ?Sized> Eq for NonNullConst<T> {}

impl<T: ?Sized> PartialEq for NonNullConst<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr()
    }
}

impl<T: ?Sized> std::cmp::Ord for NonNullConst<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ptr().cmp(&other.as_ptr())
    }
}

impl<T: ?Sized> std::cmp::PartialOrd for NonNullConst<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_ptr().partial_cmp(&other.as_ptr())
    }
}

impl<T: ?Sized> std::hash::Hash for NonNullConst<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state)
    }
}