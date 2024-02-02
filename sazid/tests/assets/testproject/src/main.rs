mod bar;
mod foo;
use foo::foo;

pub fn hello(v: String) -> String {
  format!("Hello, {}!", v)
}
fn main() {
  println!("omg wtf!");
  hello("world".to_string());
  foo("bar".to_string());
}
