#[macro_use]
extern crate dyn_sized;

trait Foo {}
derive_DynSized!(Foo);

trait MyTrait<'a, T: 'a> {
    fn borrow_it(&self, arg: &'a T);
}
derive_DynSized!(MyTrait<'a, T>, 'a, T: 'a);
