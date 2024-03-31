mod bar;
mod foo;
use foo::foo;

pub fn hello(v: String) -> String {
  format!("Hello, {}!", v)
}
fn main() {
  fn inline_fn() {
    println!("I'm an inline function!");
  }
  println!("omg wtf!");
  hello("world".to_string());
  foo("bar".to_string());
}

struct test_struct {
  pub a: i32,
}

impl test_struct {
  fn new() -> test_struct {
    test_struct { a: 0 }
  }
}
