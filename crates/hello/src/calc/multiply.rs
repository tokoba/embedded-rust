/// 乗算関数
///
/// 2つのi32値を受け取り、その積を返します
///
/// # 引数
///
/// * `a` - 最初の数値
/// * `b` - 2番目の数値
///
/// # 戻り値
///
/// 2つの引数の積
///
/// # 例
///
/// ```
/// use hello::calc::multiply;
///
/// let result = multiply(4, 5);
/// assert_eq!(result, 20);
/// ```
pub fn multiply(a: i32, b: i32) -> i32 {
  a * b
}
