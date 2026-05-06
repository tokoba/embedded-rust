use hello::calc::{add, multiply, subtract};

#[test]
fn test_add() {
  let a: i32 = 10;
  let b: i32 = -5;
  assert_eq!(5, add(a, b));
}

#[test]
fn test_subtract() {
  // 基本的な減算テスト
  assert_eq!(7, subtract(10, 3));
  assert_eq!(0, subtract(5, 5));
  assert_eq!(-7, subtract(3, 10));

  // 負の数を含むテスト
  assert_eq!(-10, subtract(-5, 5));
  assert_eq!(0, subtract(-3, -3));
  assert_eq!(2, subtract(-1, -3));
}

#[test]
fn test_multiply() {
  // 正常な乗算のテスト
  let a: i32 = 4;
  let b: i32 = 5;
  assert_eq!(20, multiply(a, b));

  // ゼロを掛けるテスト
  assert_eq!(0, multiply(0, 10));
  assert_eq!(0, multiply(10, 0));

  // 負の数の乗算テスト
  assert_eq!(-12, multiply(-3, 4));
  assert_eq!(15, multiply(-3, -5));
}
