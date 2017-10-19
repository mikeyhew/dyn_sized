#![no_std]
#![feature(raw, unboxed_closures)]
/*!
Provides the `DynSized` trait, which allows conversion between fat pointers and their (meta, data_pointer) components. `derive_DynSized!` may be used to implement `DynSized` for trait objects.
*/


extern crate fn_move;

use core::{str, slice, ptr};
use core::raw;
#[doc(hidden)]
pub use core::{mem};

/// A trait for dynamically sized types, similar in principle to the `Sized`
/// trait. Allows conversion between fat and thin pointers.
///
/// The assemble and disassemble methods must be safe, i.e. they must not dereference the raw pointers, only use them to extract the metadata in the case of `disassemble`, or to combine with metadata to produce a fat pointer, in the case of `assemble`.
///
/// # Unsafety
///
/// The trait is marked `unsafe`, because implementing it wrong can cause undefined behaviour.
pub unsafe trait DynSized {
    type Meta: Copy;

    fn assemble(meta: Self::Meta, data: *const ()) -> *const Self;

    fn assemble_mut(meta: Self::Meta, data: *mut ()) -> *mut Self {
        unsafe {
            // transmute is safe here, because we're just changing from
            // *const Self to *mut Self
            mem::transmute(Self::assemble(meta, data))
        }
    }

    fn disassemble(ptr: *const Self) -> (Self::Meta, *const ());

    fn disassemble_mut(ptr: *mut Self) -> (Self::Meta, *mut ()) {
        let (meta, data) = Self::disassemble(ptr);
        unsafe {
            (meta, mem::transmute(data))
        }
    }

    fn meta(&self) -> Self::Meta {
        let (meta, _) = Self::disassemble(self);
        meta
    }

    fn data(&self) -> *const () {
        let (_, data) = Self::disassemble(self);
        data
    }

    fn data_mut(&mut self) -> *mut () {
        let (_, data) = Self::disassemble_mut(self);
        data
    }
}

/// A version of mem::size_of_val that requires only the pointer metadata
pub fn size_of_val<T>(meta: T::Meta) -> usize where
    T: DynSized + ?Sized
{
    unsafe {  mem::size_of_val(&*T::assemble(meta, ptr::null())) }
}

/// A version of mem::align_of_val that requires only the pointer metadata
pub fn align_of_val<T>(meta: T::Meta) -> usize where
    T: DynSized + ?Sized
{
    unsafe {  mem::align_of_val(&*T::assemble(meta, ptr::null())) }
}

/// A wrapper type for `Sized` types that implements `DynSized`.
/// 
/// This is unfortunately necessary because a blanket `impl` of `DynSized` for all `Sized` types would conflict with implementations for user-defined structs that are ?Sized.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WrapSized<T>(pub T);

unsafe impl<T> DynSized for WrapSized<T> {
    type Meta = ();

    fn assemble(_: (), data: *const ()) -> *const WrapSized<T> {
        data as *const WrapSized<T>
    }

    fn disassemble(ptr: *const WrapSized<T>) -> ((), *const ()) {
        ((), ptr as *const ())
    }
}

unsafe impl<T> DynSized for [T] {
    type Meta = usize;

    fn assemble(len: usize, data: *const ()) -> *const [T] {
        unsafe {
            slice::from_raw_parts(data as *const T, len)
        }
    }

    fn disassemble(slice: *const [T]) -> (usize, *const ()) {
        let slice = unsafe { &*slice };
        (slice.len(), slice.as_ptr() as *const ())
    }
}

#[test]
fn test_slice() {
    let slice = &[1,2,3] as &[i32];
    let (len, ptr) = DynSized::disassemble(slice);
    let new_slice: &[i32] = unsafe {
        &*DynSized::assemble(len, ptr)
    };
    assert_eq!(new_slice, slice);
}

unsafe impl DynSized for str {
    type Meta = usize;

    fn assemble(len: usize, data: *const ()) -> *const str {
        unsafe {
            str::from_utf8_unchecked(slice::from_raw_parts(data as *const u8, len))
        }
    }

    fn disassemble(s: *const str) -> (usize, *const ()) {
        unsafe {
            DynSized::disassemble((&*s).as_bytes())
        }
    }
}

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

#[derive(Copy, Clone, Debug)]
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
macro_rules! __derive_DynSized_body {
    ($Trait:ty) => {
        type Meta = $crate::Vtable;

        fn assemble(vtable: $crate::Vtable, data: *const ()) -> *const Self {
            unsafe {
                $crate::mem::transmute(
                    $crate::TraitObject::construct(vtable, data as *mut ())
                )
            }
        }

        fn disassemble(ptr: *const Self) -> (Self::Meta, *const ()) {
            let trait_object: $crate::TraitObject = unsafe {
                $crate::mem::transmute(ptr)
            };

            (trait_object.vtable(), trait_object.data())
        }
    };
}

/// Derives the `DynSized` trait for trait objects.
///
/// To use:
///
/// ```
/// #[macro_use] extern crate dyn_sized;
/// # fn main() {
/// trait MyTrait {}
/// derive_DynSized!(MyTrait);
///
/// trait MyGenericTrait<'a, T: 'a> {
///     fn foo(&'a self) -> T;
/// }
/// // type arguments for the impl go after the trait object type.
/// derive_DynSized!(MyGenericTrait<'a, T>, 'a, T: 'a);
/// # }
/// ```
///
#[macro_export]
macro_rules! derive_DynSized {
    ($Trait:ty) => {
        unsafe impl $crate::DynSized for $Trait {
            __derive_DynSized_body!($Trait);
        }
    };

    ($Trait:ty, $($args:tt)+ ) => {
        unsafe impl<$($args)+> $crate::DynSized for $Trait {
            __derive_DynSized_body!($Trait);
        }
    };
}

use core::any::Any;
use fn_move::FnMove;

derive_DynSized!(Any);
derive_DynSized!(Any + Send);
derive_DynSized!(Fn<Args, Output=Output> + 'a, 'a, Args, Output);
derive_DynSized!(Fn<Args, Output=Output> + Send + 'a, 'a, Args, Output);
derive_DynSized!(Fn<Args, Output=Output> + Sync + 'a, 'a, Args, Output);
derive_DynSized!(Fn<Args, Output=Output> + Send + Sync + 'a, 'a, Args, Output);
derive_DynSized!(FnMut<Args, Output=Output> + 'a, 'a, Args, Output);
derive_DynSized!(FnMut<Args, Output=Output> + Send + 'a, 'a, Args, Output);
derive_DynSized!(FnOnce<Args, Output=Output> + 'a, 'a, Args, Output);
derive_DynSized!(FnOnce<Args, Output=Output> + Send + 'a, 'a, Args, Output);
derive_DynSized!(FnMove<Args, Output=Output> + 'a, 'a, Args, Output);
derive_DynSized!(FnMove<Args, Output=Output> + Send + 'a, 'a, Args, Output);

#[test]
#[allow(non_snake_case)]
fn test_derive_DynSized() {
    use core::borrow::Borrow;
    trait MyBorrow<Borrowed>: Borrow<Borrowed> {}
    derive_DynSized!(MyBorrow<Borrowed>, Borrowed);
}

/// An extension trait adding .meta() and .data() convenience methods
/// to built-in pointer types
pub trait PtrExt {
    type Referent: DynSized + ?Sized;

    fn meta(&self) -> <Self::Referent as DynSized>::Meta;

    fn data(&self) -> *const ();
}

/// adds the `data_mut` method to `*mut T`
pub trait PtrMutExt: PtrExt {
    fn data_mut(&self) -> *mut ();
}

impl<T: DynSized + ?Sized> PtrExt for *const T {
    type Referent = T;

    fn meta(&self) -> T::Meta  {
        let (meta, _) = T::disassemble(*self);
        meta
    }

    fn data(&self) -> *const () {
        let (_, data) = T::disassemble(*self);
        data
    }
}

impl<T: DynSized + ?Sized> PtrExt for *mut T {
    type Referent = T;

    fn meta(&self) -> T::Meta  {
        (*self as *const T).meta()
    }

    fn data(&self) -> *const () {
        let (_, data) = T::disassemble(*self);
        data
    }
}

impl<T: DynSized + ?Sized> PtrMutExt for *mut T {

    fn data_mut(&self) -> *mut () {
        let (_, data) = T::disassemble_mut(*self);
        data
    }
}

#[test]
#[allow(non_snake_case)]
fn test_PtrExt() {
    let slice: &mut [i32] = &mut [1,2,3];

    let len: <[i32] as DynSized>::Meta = slice.meta();
    assert_eq!(len, 3usize);
    let len: <[i32] as DynSized>::Meta = (slice as &[i32]).meta();
    assert_eq!(len, 3usize);
    let len: <[i32] as DynSized>::Meta = (slice as *mut [i32]).meta();
    assert_eq!(len, 3usize);
    let len: <[i32] as DynSized>::Meta = (slice as *const [i32]).meta();
    assert_eq!(len, 3usize);

    let data: *const () = slice.data();
    assert_eq!(slice as *const [_] as *const (), data);
    let data: *const () = (slice as &[i32]).data();
    assert_eq!(slice as *const [_] as *const (), data);
    let data: *const () = (slice as *const [i32]).data();
    assert_eq!(slice as *const [_] as *const (), data);
    let data: *const () = (slice as *mut [i32]).data();
    assert_eq!(slice as *const [_] as *const (), data);

    let data: *mut () = slice.data_mut();
    assert_eq!(slice as *mut [_] as *mut (), data);
    let data: *mut () = (slice as *mut [i32]).data_mut();
    assert_eq!(slice as *mut [_] as *mut (), data);
}
