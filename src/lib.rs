#![no_std]
#![feature(raw, allow_internal_unstable)]


use core::{str, slice, ptr};
use core::raw;
#[doc(hidden)]
pub use core::{mem};

/// A trait that is implemented for all types except trait objects and
/// user-defined structs. Can be derived for trait ojects with the
/// `derive_DynSized!` macro.
pub trait DynSized {
    type Meta: Copy;

    /// these should really be supplied by the compiler
    unsafe fn assemble(meta: Self::Meta, data: *const ()) -> *const Self;

    unsafe fn assemble_mut(meta: Self::Meta, data: *mut ()) -> *mut Self {
        mem::transmute(Self::assemble(meta, data))
    }

    fn disassemble(ptr: *const Self) -> (Self::Meta, *const ());

    fn disassemble_mut(ptr: *mut Self) -> (Self::Meta, *mut ()) {
        let (meta, data) = Self::disassemble(ptr);
        unsafe {
            (meta, mem::transmute(data))
        }
    }
}

pub unsafe trait AssembleSafe: DynSized {
    fn assemble_safe(meta: Self::Meta, data: *const ()) -> *const Self {
        unsafe { <Self as DynSized>::assemble(meta, data) }
    }

    fn assemble_mut_safe(meta: Self::Meta, data: *mut ()) -> *mut Self {
        unsafe { <Self as DynSized>::assemble_mut(meta, data) }
    }
}

pub fn size_of_val<T: AssembleSafe + ?Sized>(meta: T::Meta) -> usize {
    let r = unsafe {
        &*T::assemble(meta, ptr::null())
    };

    mem::size_of_val(r)
}

pub fn align_of_val<T: AssembleSafe + ?Sized>(meta: T::Meta) -> usize {
    let r = unsafe {
        &*T::assemble(meta, ptr::null())
    };

    mem::align_of_val(r)
}

pub struct WrapSized<T>(pub T);


impl<T> DynSized for WrapSized<T> {
    type Meta = ();

    unsafe fn assemble(_: (), data: *const ()) -> *const WrapSized<T> {
        data as *const WrapSized<T>
    }

    fn disassemble(ptr: *const WrapSized<T>) -> ((), *const ()) {
        ((), ptr as *const ())
    }
}

// unsafe impl<T> AssembleSafe for T {}

impl<T> DynSized for [T] {
    type Meta = usize;

    unsafe fn assemble(len: usize, data: *const ()) -> *const [T] {
        slice::from_raw_parts(data as *const T, len)
    }

    fn disassemble(slice: *const [T]) -> (usize, *const ()) {
        let slice = unsafe { &*slice };
        (slice.len(), slice.as_ptr() as *const ())
    }
}

unsafe impl<T> AssembleSafe for [T] {}

#[test]
fn test_slice() {
    let slice = &[1,2,3] as &[i32];
    let (len, ptr) = DynSized::disassemble(slice);
    let new_slice: &[i32] = unsafe {
        &*DynSized::assemble(len, ptr)
    };
    assert_eq!(new_slice, slice);
}

impl DynSized for str {
    type Meta = usize;

    unsafe fn assemble(len: usize, data: *const ()) -> *const str {
        str::from_utf8_unchecked(slice::from_raw_parts(data as *const u8, len))
    }

    fn disassemble(s: *const str) -> (usize, *const ()) {
        unsafe {
            DynSized::disassemble((&*s).as_bytes())
        }
    }
}

unsafe impl AssembleSafe for str {}

#[test]
fn test_str() {
    let s = "Yolo123";
    let (ptr, len) = DynSized::disassemble(s);
    let new_s: &str = unsafe {
        &*DynSized::assemble(ptr, len)
    };
    assert_eq!(s, new_s);
}

#[derive(Copy, Clone)]
#[doc(hidden)]
pub struct TraitObject(raw::TraitObject);

#[derive(Copy, Clone)]
#[doc(hidden)]
pub struct Vtable(*mut ());

impl TraitObject {
    pub fn construct(vtable: Vtable, data: *mut ()) -> TraitObject {
        TraitObject(raw::TraitObject {
            data: data,
            vtable: vtable.0,
        })
    }

    pub fn data(self) -> *mut () {
        self.0.data
    }

    pub fn vtable(self) -> Vtable {
        Vtable(self.0.vtable)
    }
}

#[macro_export]
#[doc(hidden)]
#[allow_internal_unstable]
macro_rules! __derive_DynSized_body {
    ($Trait:ty) => {
        type Meta = $crate::Vtable;

        unsafe fn assemble(vtable: $crate::Vtable, data: *const ()) -> *const Self {
            $crate::mem::transmute(
                $crate::TraitObject::construct(vtable, data as *mut ())
            )
        }

        fn disassemble(ptr: *const Self) -> (Self::Meta, *const ()) {
            let trait_object: $crate::TraitObject = unsafe {
                $crate::mem::transmute(ptr)
            };

            (trait_object.vtable(), trait_object.data())
        }
    };
}

#[macro_export]
macro_rules! derive_DynSized {
    ($Trait:ty) => {
        impl $crate::DynSized for $Trait {
            __derive_DynSized_body!($Trait);
        }

        unsafe impl $crate::AssembleSafe for $Trait {}
    };

    ($Trait:ty, $($args:tt)+ ) => {
        impl<$($args)+> $crate::DynSized for $Trait {
            __derive_DynSized_body!($Trait);
        }

        unsafe impl<$($args)+> $crate::AssembleSafe for $Trait {}
    };
}

#[test]
#[allow(non_snake_case)]
fn test_derive_DynSized() {
    use core::borrow::Borrow;
    trait MyBorrow<Borrowed>: Borrow<Borrowed> {}
    derive_DynSized!(MyBorrow<Borrowed>, Borrowed);
}

pub trait PtrExt {
    type Referent: DynSized + ?Sized;
    type DataPtr: Copy;

    fn meta(&self) -> <Self::Referent as DynSized>::Meta;
    
    fn data(&self) -> Self::DataPtr;
}

impl<T: DynSized + ?Sized> PtrExt for *const T {
    type Referent = T;
    type DataPtr = *const ();

    fn meta(&self) -> T::Meta  {
        let (meta, _) = T::disassemble(*self);
        meta
    }

    fn data(&self) -> *const () {
        let (_, data) = T::disassemble(*self);
        data
    }
}

impl<'a, T: DynSized + ?Sized + 'a> PtrExt for &'a T {
    type Referent = T;
    type DataPtr = *const ();

    fn meta(&self) -> T::Meta  {
        (*self as *const T).meta()
    }

    fn data(&self) -> *const () {
        (*self as *const T).data()
    }
}

impl<T: DynSized + ?Sized> PtrExt for *mut T {
    type Referent = T;
    type DataPtr = *mut ();

    fn meta(&self) -> T::Meta  {
        (*self as *const T).meta()
    }

    fn data(&self) -> *mut () {
        let (_, data) = T::disassemble_mut(*self);
        data
    }
}

impl<'a, T: DynSized + ?Sized + 'a> PtrExt for &'a mut T {
    type Referent = T;
    type DataPtr = *mut ();

    fn meta(&self) -> T::Meta  {
        (*self as *const T).meta()
    }

    fn data(&self) -> *mut () {
        (*self as *const T as *mut T).data()
    }
}

#[test]
#[allow(non_snake_case)]
fn test_PtrExt() {
    let slice: &mut [i32] = &mut [1,2,3];

    let len: <[i32] as DynSized>::Meta = slice.meta();
    assert_eq!(len, 3usize);

    let data: *mut () = slice.data();
    assert_eq!(slice as *mut [_] as *mut (), data);
}
