//! hello library

pub mod calc;

/// 挨拶を返す
///
/// # 引数
///
/// * `name` - 挨拶する相手の名前
///
/// # 戻り値
///
/// 挨拶メッセージ
///
/// # 例
///
/// ```
/// use hello::greet;
///
/// assert_eq!(greet("World"), "Hello, World!");
/// ```
pub fn greet(name: &str) -> String {
  format!("Hello, {name}!")
}
