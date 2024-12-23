#![feature(str_from_raw_parts)]
//! 实现 `SmallString`，一个类似 `String` 的小字符串容器
//!
//! ## `no_std` support
//!
//! 默认情况下，`smallstr`不依赖于`std`。 `std`功能可能已启用！添加 `std` 依赖项。 `ffi`功能也意味着`std`。
//!
//! ## `ffi` feature
//!
//! `ffi`功能将向`SmallString`添加以下特征实现：
//!
//! * `PartialEq<OsStr>`
//! * `PartialEq<&'_ OsStr>`
//! * `PartialEq<OsString>`
//! * `PartialEq<Cow<'_, OsString>>`
//!
//! 此功能还添加`std`作为依赖项。
//!
//! ## `serde` 支持
//!
//! 当启用 `serde` 功能时，特征 `serde::Deserialize` 和 ! `serde::Serialize` 是为 `SmallString` 实现的。
//!
//! 默认情况下禁用此功能。
//!
//! 默认情况下，`serde`依赖项是使用`no_std`编译的。 ！如果启用了`std`功能，`std`也会作为依赖项添加到`serde`中。

#![cfg_attr(not(any(feature = "ffi", feature = "std")), no_std)]
#![deny(missing_docs)]
extern crate alloc;

pub use string::*;

mod string;
