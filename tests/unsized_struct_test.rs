
extern crate dyn_sized;

use dyn_sized::DynSized;

use std::mem;

#[allow(dead_code)]
struct MyStruct<T: ?Sized> {
    x: usize,
    value: T
}

unsafe impl<T: DynSized + ?Sized> DynSized for MyStruct<T> {
    type Meta = T::Meta;

    fn assemble(meta: T::Meta, data: *const ()) -> *const Self {
        // note: safe because T::assemble does not dereference *data
        let t_ptr: *const T = T::assemble(meta, data);
        unsafe {
            // safe, assuming the compiler always represents pointers to unsized struct the same way as pointers to their unsized value
            mem::transmute(t_ptr)
        }
    }

    fn disassemble(ptr: *const Self) -> (T::Meta, *const ()) {
        let t_ptr: *mut T = unsafe {
            // again, this is safe because of the way the compiler represents pointers to unsized structs
            mem::transmute(ptr)
        };
        T::disassemble(t_ptr)
    }
}

#[test]
fn slice() {
    let my_struct = MyStruct {
        x: 0,
        value: [1i32,2,3,4]
    };

    let my_struct_ptr = &my_struct as &MyStruct<[i32]>;

    assert_eq!(my_struct_ptr.meta(), 4);
    assert_eq!(&my_struct_ptr.value, &[1,2,3,4]);
    assert_eq!(my_struct_ptr.data(), &my_struct as *const _ as *const ());

    let my_struct_ptr_assembled = MyStruct::assemble(4usize, &my_struct as *const _ as *const ());

    assert_eq!(my_struct_ptr as *const _, my_struct_ptr_assembled);
}

#[test]
fn trait_object() {
    trait Foo {
        fn foo(&self) -> i32;
    }

    impl Foo for i32 {
        fn foo(&self) -> i32 {
            *self + 1
        }
    }

    let my_struct = MyStruct {
        x: 5,
        value: 3i32
    };

    assert_eq!((&my_struct as &MyStruct<Foo>).value.foo(), 4);
}
