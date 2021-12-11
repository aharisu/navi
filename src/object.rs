#![allow(unused_unsafe)]

use crate::value::{self, *};
use crate::mm::{Heap};
use crate::world::World;

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;

pub struct Object {
    heap: RefCell<Heap>,
    world: World,
    frames: Vec<Vec<(NPtr<symbol::Symbol>, NPtr<Value>)>>,
    nbox_root: Cell<Option<NonNull<Capture<Value>>>>,
}

impl Object {
    pub fn new<T: Into<String>>(name: T) -> Self {
        Object {
            heap: RefCell::new(Heap::new(1024 * 8, name)),
            world: World::new(),
            frames: Vec::new(),
            nbox_root: Cell::new(None),
        }
    }

    pub fn push_local_frame<T, U>(&mut self, frame: &[(&T, &U)])
    where
        T: crate::value::AsPtr<symbol::Symbol>,
        U: crate::value::AsPtr<Value>
    {
        let mut vec = Vec::<(NPtr<symbol::Symbol>, NPtr<Value>)>::new();
        for (symbol, v) in frame {
            vec.push((NPtr::new(symbol.as_mut_ptr()), NPtr::new(v.as_mut_ptr())));
        }

        self.frames.push(vec);
    }

    pub fn pop_local_frame(&mut self) {
        self.frames.pop();
    }

    pub fn find_value(&self, symbol: &Capture<symbol::Symbol>) -> Option<NPtr<Value>> {
        //ローカルフレームから対応する値を探す
        for frame in self.frames.iter().rev() {
            let result = frame.iter().find(|(sym, _v)| {
                symbol.as_ref().eq(sym.as_ref())
            });

            if let Some((_, v)) = result {
                return Some(NPtr::new(v.as_mut_ptr()));
            }
        }

        //ローカルフレーム上になければ、グローバルスペースから探す
        if let Some(v) = self.world.get(symbol.as_ref()) {
            Some(NPtr::new(v.as_mut_ptr()))
        } else {
            None
        }
    }

    pub fn alloc<T: NaviType>(&self) -> NPtr<T> {
        self.heap.borrow_mut().alloc::<T>(self)
    }

    pub fn alloc_with_additional_size<T: NaviType>(&self, additional_size: usize) -> NPtr<T> {
        self.heap.borrow_mut().alloc_with_additional_size::<T>(additional_size, self)
    }

    pub fn define_value<K, T>(&mut self, key: K, v: &T)
    where
        K: AsRef<str>,
        T: crate::value::AsPtr<Value>,
    {
        (&mut self.world).set(key, NPtr::new(v.as_mut_ptr()))
    }

    pub fn add_capture(&self, nbox: &mut Capture<Value>) {
        //ポインタ以外の値はキャプチャの必要がないので何もしない
        if value::value_is_pointer(nbox.as_ref()) == false {
            return
        }

        unsafe {
            let nbox_ptr= NonNull::new_unchecked(nbox as *mut Capture<Value>);

            match &mut self.nbox_root.get() {
                Some(root) => {
                    nbox.next = Some(*root);
                    root.as_mut().prev = Some(nbox_ptr);
                }
                None => { }
            }

            nbox.prev = None;

            self.nbox_root.set(Some(nbox_ptr));
        }
    }

    pub fn drop_capture(&self, nbox: &mut Capture<Value>) {
        //ポインタ以外の値はキャプチャの必要がないので何もしない
        if value::value_is_pointer(nbox.as_ref()) == false {
            return
        }

        match nbox.prev {
            Some(prev) => {
                unsafe {
                    (*prev.as_ptr()).next = nbox.next;
                }
            }
            None => {
                self.nbox_root.set(nbox.next);
            }
        };

        match nbox.next {
            Some(next) => {
                unsafe { (*next.as_ptr()).prev = nbox.prev }
            }
            None => { }
        };
    }

    pub(crate) fn for_each_all_alived_value(&self, arg: usize, callback: fn(&NPtr<Value>, usize)) {
        //ローカルフレーム内で保持している値
        for frame in self.frames.iter() {
            for (sym, v) in frame.iter() {
                callback(sym.cast_value(), arg);
                callback(v, arg);
            }
        }

        //グローバルスペース内で保持している値
        for v in self.world.get_all_values().iter() {
            callback(v, arg);
        }

        //ローカル変数として捕捉している値
        let mut node = self.nbox_root.get();
        loop {
            match node {
                Some(capture_ptr) => {
                    let capture = unsafe { capture_ptr.as_ref() };
                    callback(&capture.v, arg);
                    node = capture.next;
                }
                None => break,
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn heap_used(&self) -> usize {
        self.heap.borrow().used()
    }

    #[allow(dead_code)]
    pub(crate) fn do_gc(&self) {
        self.heap.borrow_mut().gc(self);
    }

}

impl Drop for Object {
    fn drop(&mut self) {
        self.heap.borrow_mut().free();
    }
}

#[macro_export]
macro_rules! new_cap {
    ($ptr:expr, $ctx:expr) => {
        crate::object::Capture {
            v: $ptr,
            ctx: unsafe { std::ptr::NonNull::new_unchecked( $ctx as *const Object as *mut Object) },
            next: None,
            prev: None,
            _pinned: std::marker::PhantomPinned,
        }
    };
}

#[macro_export]
macro_rules! let_cap {
    ($name:ident, $ptr:expr, $ctx:expr) => {
        #[allow(dead_code)]
        let $name = new_cap!($ptr, $ctx);
        pin_utils::pin_mut!($name);
        ($ctx).add_capture(&mut ($name).cast_value());
    };
}

#[macro_export]
macro_rules! with_cap {
    ($name:ident, $ptr:expr, $ctx:expr, $block:expr) => {
        {
            let_cap!($name, $ptr, $ctx);
            $block
        }
    };
}

pub struct Capture<T: NaviType> {
    pub v: NPtr<T>,
    pub ctx: NonNull<Object>,
    pub next: Option<NonNull<Capture<Value>>>,
    pub prev: Option<NonNull<Capture<Value>>>,
    pub _pinned: std::marker::PhantomPinned,
}

impl <T: NaviType> Capture<T> {
    pub fn nptr(&self) -> &NPtr<T> {
        &self.v
    }

    pub fn cast_value(&self) -> &mut Capture<Value> {
        unsafe { &mut *(self as *const Capture<T> as *const Capture<Value> as *mut Capture<Value>) }
    }
}

impl Capture<Value> {
    pub fn try_cast<U: NaviType>(&self) -> Option<&Capture<U>> {
        if self.as_ref().is::<U>() {
            Some(unsafe { &*(self as *const Capture<Value> as *const Capture<U>) })

        } else {
            None
        }
    }

    pub fn is<U: NaviType>(&self) -> bool {
        self.as_ref().is::<U>()
    }

    pub fn is_type(&self, typeinfo: crate::util::non_null_const::NonNullConst<TypeInfo>) -> bool {
        self.as_ref().is_type(typeinfo)
    }
}


impl <T: NaviType> AsRef<T> for Capture<T> {

    fn as_ref(&self) -> &T {
        self.v.as_ref()
    }
}

impl <T: NaviType> AsMut<T> for Capture<T> {

    fn as_mut(&mut self) -> &mut T {
        self.v.as_mut()
    }
}

impl <T: NaviType> AsPtr<T> for Capture<T> {
    fn as_ptr(&self) -> *const T {
        self.v.as_ptr()
    }

    fn as_mut_ptr(&self) -> *mut T {
        self.v.as_mut_ptr()
    }
}

impl <T: NaviType> std::fmt::Debug for Capture<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl <T: NaviType> Drop for Capture<T> {
    fn drop(&mut self) {
        unsafe { self.ctx.as_ref() }.drop_capture(self.cast_value())
    }
}


impl <T: NaviType> AsPtr<T> for std::pin::Pin<&mut Capture<T>> {
    fn as_mut_ptr(&self) -> *mut T {
        (**self).as_mut_ptr()
    }

    fn as_ptr(&self) -> *const T {
        (**self).as_ptr()
    }
}
