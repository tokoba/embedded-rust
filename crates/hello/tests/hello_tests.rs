use hello::greet;

#[test]
fn test_hello_world() {
  let name = "John";
  assert_eq!("Hello, John!", greet(name));
}
