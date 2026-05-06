//! 計算モジュール
//!
//! 基本的な算術演算機能を提供します。
//!
//! # 例
//!
//! ```
//! use hello::calc::{add, subtract, multiply};
//!
//! let sum = add(2, 3);
//! let diff = subtract(10, 4);
//! let product = multiply(5, 6);
//! ```
//!
//! # 利用可能な関数
//!
//! - [`add()`] - 加算
//! - [`subtract()`] - 減算
//! - [`multiply()`] - 乗算

pub mod add;
pub mod multiply;
pub mod subtract;

// add関数を再エクスポートして、hello::calc::addとして直接アクセスできるようにする
pub use add::add;

// subtract関数を再エクスポートして、hello::calc::subtractとして直接アクセスできるようにする
pub use subtract::subtract;

// multiply関数を再エクスポートして、hello::calc::multiplyとして直接アクセスできるようにする
pub use multiply::multiply;
