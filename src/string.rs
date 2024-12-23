use core::{
    borrow::{Borrow, BorrowMut},
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    iter::FromIterator,
    ops, ptr, slice,
    str::{self, Chars, Utf8Error},
};

use alloc::{borrow::Cow, boxed::Box, string::String};

#[cfg(feature = "ffi")]
use std::ffi::{OsStr, OsString};

#[cfg(feature = "serde")]
use core::marker::PhantomData;
#[cfg(feature = "serde")]
use serde::{
    de::{Deserialize, Deserializer, Error, Visitor},
    ser::{Serialize, Serializer},
};
use std::intrinsics::{unchecked_add, unchecked_div, unchecked_mul, unchecked_rem, unchecked_sub};
// use std::ops::Add;

use smallvec::SmallVec;

/// 类似`String`的容器，可以内联存储少量字节。
///
/// `SmallString` 使用 `SmallVec<[u8; 4096]>` 作为其内部存储。
#[derive(Clone, Default)]
pub struct SmallString<const SIZE: usize = { const { 1 << 12 } }> {
    data: SmallVec<u8, SIZE>,
}

impl<const SIZE: usize> SmallString<SIZE> {
    const DIGIT_PAIRS: [u8; 200] = *b"00010203040506070809\
                                    10111213141516171819\
                                    20212223242526272829\
                                    30313233343536373839\
                                    40414243444546474849\
                                    50515253545556575859\
                                    60616263646566676869\
                                    70717273747576777879\
                                    80818283848586878889\
                                    90919293949596979899";

    /// 构造一个空字符串。
    #[inline(always)]
    pub const fn new() -> SmallString<SIZE> {
        SmallString {
            data: SmallVec::<u8, SIZE>::new(),
        }
    }

    /// 构造一个空字符串，并预先分配足够的容量来存储至少`N`个字节。
    ///
    /// 仅当`N`大于内联容量时才会创建堆分配。
    #[inline(always)]
    pub fn with_capacity(n: usize) -> SmallString<SIZE> {
        SmallString {
            data: SmallVec::with_capacity(n),
        }
    }

    /// 通过从`&str`复制数据来构造`SmallString`。
    #[inline(always)]
    pub fn from_str(s: &str) -> SmallString<SIZE> {
        SmallString {
            data: SmallVec::from_slice(s.as_bytes()),
        }
    }

    /// 使用现有分配构造`SmallString`。
    #[inline(always)]
    pub fn from_string(s: String) -> SmallString<SIZE> {
        SmallString {
            data: SmallVec::from_vec(s.into_bytes()),
        }
    }

    /// 使用 UTF-8 字节在堆栈上构造一个新的 `SmallString`。
    ///
    /// 如果提供的字节数组不是有效的 UTF-8，则返回错误。
    #[inline(always)]
    pub fn from_buf(buf: [u8; SIZE]) -> Result<SmallString<SIZE>, Utf8Error> {
        let data = SmallVec::from_buf(buf);

        match str::from_utf8(&data) {
            Ok(_) => Ok(SmallString { data }),
            Err(error) => Err(error),
        }
    }

    /// 使用提供的字节数组在堆栈上构造一个新的`SmallString`，而不检查该数组是否包含有效的 UTF-8。
    ///
    /// # Safety
    ///
    /// 此函数不安全，因为它不检查传递给它的字节是否是有效的`UTF-8`如果违反此约束
    /// 可能会导致内存不安全问题，因为Rust标准库函数假定 `&str` 是有效的 UTF-8。
    #[inline(always)]
    pub unsafe fn from_buf_unchecked(buf: [u8; SIZE]) -> SmallString<SIZE> {
        SmallString {
            data: SmallVec::from_buf(buf),
        }
    }

    /// 该字符串可以内联容纳的最大字节数。
    #[inline(always)]
    pub fn inline_size(&self) -> usize {
        SmallVec::<u8, SIZE>::inline_size()
    }

    /// 返回该字符串的长度（以字节为单位）。
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 如果此字符串为空，则返回`true`。
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 返回该字符串无需重新分配即可容纳的字节数。
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// 如果数据已溢出到单独的堆分配缓冲区中，则返回`true`。
    #[inline(always)]
    pub fn spilled(&self) -> bool {
        self.data.spilled()
    }

    /// 清空字符串并返回其先前内容的迭代器。
    pub fn drain(&mut self) -> Drain {
        unsafe {
            let len = self.len();

            self.data.set_len(0);

            let ptr = self.as_ptr();

            let slice = slice::from_raw_parts(ptr, len);
            let s = str::from_utf8_unchecked(slice);

            Drain { iter: s.chars() }
        }
    }

    /// 将给定的`char`附加到该字符串的末尾。
    ///
    /// # Examples
    ///
    /// ```
    /// use smallstr::SmallString;
    ///
    /// let mut s: SmallString<8> = SmallString::from("foo");
    ///
    /// s.push('x');
    ///
    /// assert_eq!(s, "foox");
    /// ```
    #[inline(always)]
    pub fn push(&mut self, ch: char) {
        match ch.len_utf8() {
            1 => self.data.push(ch as u8),
            _ => self.push_str(ch.encode_utf8(&mut [0; 4])),
        }
    }

    /// 将给定的数字附加到该字符串的末尾。
    ///
    /// # Examples
    ///
    /// ```
    /// use smallstr::SmallString;
    ///
    /// let mut s: SmallString<8> = SmallString::from("foo");
    ///
    /// s.push_integer(12345);
    ///
    /// assert_eq!(s, "foo12345");
    /// ```
    #[inline(always)]
    pub fn push_integer(&mut self, mut num: u64) {
        let num_len = if num == 0 {
            1
        } else {
            // 我们计算出num的个位数数量
            unsafe { unchecked_add((num as f64).log10() as usize, 1) }
        };

        let len = self.data.len();
        if len + num_len > self.data.capacity() {
            // 如果容量不够我们需要增长容量
            self.data.grow(self.data.len() * 2);
        }

        // 如果是num是奇数我们先处理最后一位
        if num_len & 1 == 1 {
            unsafe {
                self.data
                    .as_mut_ptr()
                    .add(unchecked_sub(unchecked_add(len, num_len), 1))
                    .write(unchecked_add(b'0', (num % 10_u64) as u8));
                num = unchecked_div(num, 10);
            }
        }

        for i in (0..(num_len & !1) >> 1).rev() {
            unsafe {
                let pos = unchecked_add(len, unchecked_mul(i, 2));
                let digit_pos = (unchecked_rem(num, 100) << 1) as usize;

                // 我们每次处理16bit在大多数平台多16bit处理效率更高
                (self.data.as_mut_ptr().add(pos) as *mut u16)
                    .write((Self::DIGIT_PAIRS.as_ptr().add(digit_pos) as *const u16).read());
                num = unchecked_div(num, 100);
            }
        }
        // 最后我们需要调整数组的真实长度
        unsafe { self.data.set_len(len + num_len) }
    }

    /// 将给定的字符串切片附加到该字符串的末尾。
    ///
    /// # Examples
    ///
    /// ```
    /// use smallstr::SmallString;
    ///
    /// let mut s: SmallString<8> = SmallString::from("foo");
    ///
    /// s.push_str("bar");
    ///
    /// assert_eq!(s, "foobar");
    /// ```
    #[inline(always)]
    pub fn push_str(&mut self, s: &str) {
        self.data.extend_from_slice(s.as_bytes());
    }

    /// 从此字符串中删除最后一个字符并返回它。
    ///
    /// 如果字符串为空，则返回`None`。
    #[inline(always)]
    pub fn pop(&mut self) -> Option<char> {
        match self.chars().next_back() {
            Some(ch) => unsafe {
                let new_len = self.len() - ch.len_utf8();
                self.data.set_len(new_len);
                Some(ch)
            },
            None => None,
        }
    }

    /// 重新分配以将新容量设置为`new_cap`。
    ///
    /// # Panics
    ///
    /// 如果`new_cap`小于当前长度。
    #[inline(always)]
    pub fn grow(&mut self, new_cap: usize) {
        self.data.grow(new_cap);
    }

    /// 确保该字符串的容量至少比其长度大`additional`字节。
    ///
    /// 为了防止频繁的重新分配，容量可以增加超过`additional`字节。
    #[inline(always)]
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// 确保该字符串的容量比其长度大`additional`字节。
    #[inline(always)]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// 尽可能缩小字符串的容量。
    ///
    /// 如果可能，这会将数据从外部堆缓冲区移动到字符串的内联存储。
    #[inline(always)]
    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }

    /// 缩短字符串，保留前一个`len`字节。
    ///
    /// 这不会重新分配。如果要缩小字符串的容量，请在截断后使用`shrink_to_fit`。
    /// # Panics
    ///
    /// 如果`len`不在`char`边界上。
    #[inline(always)]
    pub fn truncate(&mut self, len: usize) {
        assert!(self.is_char_boundary(len));
        self.data.truncate(len);
    }

    /// 提取包含整个字符串的字符串切片。
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        unsafe { str::from_raw_parts(self.data.as_ptr(), self.data.len()) }
    }

    /// 提取包含整个字符串的字符串切片。
    #[inline(always)]
    pub fn as_mut_str(&mut self) -> &mut str {
        unsafe { str::from_raw_parts_mut(self.data.as_mut_ptr(), self.data.len()) }
    }

    /// 删除字符串的所有内容。
    #[inline(always)]
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// 从此字符串中的字节位置删除`char`并返回它。
    /// # Panics
    ///
    /// 如果`index`不在`char`边界上。
    #[inline(always)]
    pub fn remove(&mut self, index: usize) -> char {
        let ch = match self[index..].chars().next() {
            Some(ch) => ch,
            None => panic!("cannot remove a char from the end of a string"),
        };

        let ch_len = ch.len_utf8();
        let next = index + ch_len;
        let len = self.len();

        unsafe {
            ptr::copy(
                self.as_ptr().add(next),
                self.as_mut_ptr().add(index),
                len - next,
            );
            self.data.set_len(len - ch_len);
        }

        ch
    }

    /// 将`char`插入到该字符串的给定字节位置。
    /// # Panics
    ///
    /// 如果`index`不在`char`边界上。
    #[inline(always)]
    pub fn insert(&mut self, index: usize, ch: char) {
        assert!(self.is_char_boundary(index));

        match ch.len_utf8() {
            1 => self.data.insert(index, ch as u8),
            _ => self.insert_str(index, ch.encode_utf8(&mut [0; 4])),
        }
    }

    /// 将 `&str` 插入到该字符串的给定字节位置。
    /// # Panics
    ///
    /// 如果`index`不在`char`边界上。
    #[inline(always)]
    pub fn insert_str(&mut self, index: usize, s: &str) {
        assert!(self.is_char_boundary(index));

        let len = self.len();
        let amt = s.len();

        self.data.reserve(amt);

        unsafe {
            ptr::copy(
                self.as_ptr().add(index),
                self.as_mut_ptr().add(index + amt),
                len - index,
            );
            ptr::copy_nonoverlapping(s.as_ptr(), self.as_mut_ptr().add(index), amt);
            self.data.set_len(len + amt);
        }
    }

    /// 返回对`SmallString`内容的可变引用。
    ///
    /// # Safety
    ///
    /// 此函数不安全，因为它不检查传递给它的字节是否是有效的 UTF-8。
    /// 如果违反此约束，可能会导致内存不安全问题，因为 Rust 标准库函数假定 `&str` 是有效的 UTF-8。
    #[inline(always)]
    pub unsafe fn as_mut_vec(&mut self) -> &mut SmallVec<u8, SIZE> {
        &mut self.data
    }

    /// 如果`SmallString`已经溢出到堆上、在转换不成中不会重新分配内存。而是使用`Vec::from_raw_parts`构造Vec
    /// 如果`SmallString`没有溢出到堆上、会创建Vec并将栈上数据拷贝至新创建的Vec
    #[inline(always)]
    pub fn into_string(self) -> String {
        unsafe { String::from_utf8_unchecked(self.data.into_vec()) }
    }

    /// 将 `SmallString`转换为`Box<string>`，如果 `SmallString` 已经溢出到堆上，则不进行分配。
    ///
    /// 请注意，这将减少过剩产能。
    #[inline(always)]
    pub fn into_boxed_str(self) -> Box<str> {
        self.into_string().into_boxed_str()
    }

    /// 如果可能的话，将 `SmallString` 转换为 `[u8; SIZE]`。否则，返回`Err(Self)`。
    ///
    /// 如果`SmallString`太短（并且包含未初始化的元素）或者`SmallString`太长（并且元素已溢出到堆中），则此方法返回`Err(self)`。
    #[inline(always)]
    pub fn into_inner(self) -> Result<[u8; SIZE], SmallString<SIZE>> {
        self.data.into_inner().map_err(|data| SmallString { data })
    }

    /// 仅保留谓词指定的字符。
    ///
    /// 换句话说，删除所有字符`c`，以便`f(c)`返回`false`。此方法就地运行并保留保留字符的顺序。
    ///
    /// # Examples
    ///
    /// ```
    /// use smallstr::SmallString;
    ///
    /// let mut s: SmallString<16> = SmallString::from("f_o_ob_ar");
    ///
    /// s.retain(|c| c != '_');
    ///
    /// assert_eq!(s, "foobar");
    /// ```
    #[inline(always)]
    pub fn retain<F: FnMut(char) -> bool>(&mut self, mut f: F) {
        struct SetLenOnDrop<'a, const SIZE: usize> {
            s: &'a mut SmallString<SIZE>,
            idx: usize,
            del_bytes: usize,
        }

        impl<'a, const SIZE: usize> Drop for SetLenOnDrop<'_, SIZE> {
            fn drop(&mut self) {
                let new_len = self.idx - self.del_bytes;
                debug_assert!(new_len <= self.s.len());
                unsafe { self.s.data.set_len(new_len) };
            }
        }

        let len = self.len();
        let mut guard = SetLenOnDrop::<SIZE> {
            s: self,
            idx: 0,
            del_bytes: 0,
        };

        while guard.idx < len {
            let ch = unsafe {
                guard
                    .s
                    .get_unchecked(guard.idx..len)
                    .chars()
                    .next()
                    .unwrap()
            };
            let ch_len = ch.len_utf8();

            if !f(ch) {
                guard.del_bytes += ch_len;
            } else if guard.del_bytes > 0 {
                unsafe {
                    ptr::copy(
                        guard.s.data.as_ptr().add(guard.idx),
                        guard.s.data.as_mut_ptr().add(guard.idx - guard.del_bytes),
                        ch_len,
                    );
                }
            }

            // Point idx to the next char
            guard.idx += ch_len;
        }

        drop(guard);
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.as_ptr() as *mut u8
    }
}

impl<const SIZE: usize> ops::Deref for SmallString<SIZE> {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const SIZE: usize> ops::DerefMut for SmallString<SIZE> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl<const SIZE: usize> AsRef<str> for SmallString<SIZE> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const SIZE: usize> AsMut<str> for SmallString<SIZE> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl<const SIZE: usize> Borrow<str> for SmallString<SIZE> {
    #[inline(always)]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<const SIZE: usize> BorrowMut<str> for SmallString<SIZE> {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl<const SIZE: usize> AsRef<[u8]> for SmallString<SIZE> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.data.as_ptr(), self.data.len()) }
    }
}

impl<const SIZE: usize> fmt::Write for SmallString<SIZE> {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    #[inline(always)]
    fn write_char(&mut self, ch: char) -> fmt::Result {
        self.push(ch);
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl<const SIZE: usize> Serialize for SmallString<SIZE> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de, const SIZE: usize> Deserialize<'de> for SmallString<SIZE> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(SmallStringVisitor {
            phantom: PhantomData,
        })
    }
}

#[cfg(feature = "serde")]
struct SmallStringVisitor<const SIZE: usize> {
    phantom: PhantomData<u8>,
}

#[cfg(feature = "serde")]
impl<'de, const SIZE: usize> Visitor<'de> for SmallStringVisitor<SIZE> {
    type Value = SmallString<SIZE>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a string")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(v.into())
    }

    fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(v.into())
    }
}

impl<const SIZE: usize> From<char> for SmallString<SIZE> {
    #[inline(always)]
    fn from(ch: char) -> SmallString<SIZE> {
        SmallString::from_str(ch.encode_utf8(&mut [0; 4]))
    }
}

impl<'a, const SIZE: usize> From<&'a str> for SmallString<SIZE> {
    #[inline(always)]
    fn from(s: &str) -> SmallString<SIZE> {
        SmallString::from_str(s)
    }
}

impl<const SIZE: usize> From<Box<str>> for SmallString<SIZE> {
    #[inline(always)]
    fn from(s: Box<str>) -> SmallString<SIZE> {
        SmallString::from_string(s.into())
    }
}

impl<const SIZE: usize> From<String> for SmallString<SIZE> {
    #[inline(always)]
    fn from(s: String) -> SmallString<SIZE> {
        SmallString::from_string(s)
    }
}

impl<'a, const SIZE: usize> From<Cow<'a, str>> for SmallString<SIZE> {
    fn from(value: Cow<'a, str>) -> Self {
        match value {
            Cow::Borrowed(s) => Self::from_str(s),
            Cow::Owned(s) => Self::from_string(s),
        }
    }
}

macro_rules! impl_index_str {
    ($index_type: ty) => {
        impl<const SIZE: usize> ops::Index<$index_type> for SmallString<SIZE> {
            type Output = str;

            #[inline(always)]
            fn index(&self, index: $index_type) -> &str {
                &self.as_str()[index]
            }
        }

        impl<const SIZE: usize> ops::IndexMut<$index_type> for SmallString<SIZE> {
            #[inline(always)]
            fn index_mut(&mut self, index: $index_type) -> &mut str {
                &mut self.as_mut_str()[index]
            }
        }
    };
}

impl_index_str!(ops::Range<usize>);
impl_index_str!(ops::RangeFrom<usize>);
impl_index_str!(ops::RangeTo<usize>);
impl_index_str!(ops::RangeFull);

impl<const SIZE: usize> FromIterator<char> for SmallString<SIZE> {
    fn from_iter<I: IntoIterator<Item = char>>(iter: I) -> SmallString<SIZE> {
        let mut s = SmallString::new();
        s.extend(iter);
        s
    }
}

impl<'a, const SIZE: usize> FromIterator<&'a char> for SmallString<SIZE> {
    fn from_iter<I: IntoIterator<Item = &'a char>>(iter: I) -> SmallString<SIZE> {
        let mut s = SmallString::new();
        s.extend(iter.into_iter().cloned());
        s
    }
}

impl<'a, const SIZE: usize> FromIterator<Cow<'a, str>> for SmallString<SIZE> {
    fn from_iter<I: IntoIterator<Item = Cow<'a, str>>>(iter: I) -> SmallString<SIZE> {
        let mut s = SmallString::new();
        s.extend(iter);
        s
    }
}

impl<'a, const SIZE: usize> FromIterator<&'a str> for SmallString<SIZE> {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> SmallString<SIZE> {
        let mut s = SmallString::new();
        s.extend(iter);
        s
    }
}

impl<const SIZE: usize> FromIterator<String> for SmallString<SIZE> {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> SmallString<SIZE> {
        let mut s = SmallString::new();
        s.extend(iter);
        s
    }
}

impl<const SIZE: usize> Extend<char> for SmallString<SIZE> {
    fn extend<I: IntoIterator<Item = char>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (lo, _) = iter.size_hint();

        self.reserve(lo);

        for ch in iter {
            self.push(ch);
        }
    }
}

impl<'a, const SIZE: usize> Extend<&'a char> for SmallString<SIZE> {
    fn extend<I: IntoIterator<Item = &'a char>>(&mut self, iter: I) {
        self.extend(iter.into_iter().cloned());
    }
}

impl<'a, const SIZE: usize> Extend<Cow<'a, str>> for SmallString<SIZE> {
    fn extend<I: IntoIterator<Item = Cow<'a, str>>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(&s);
        }
    }
}

impl<'a, const SIZE: usize> Extend<&'a str> for SmallString<SIZE> {
    fn extend<I: IntoIterator<Item = &'a str>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(s);
        }
    }
}

impl<const SIZE: usize> Extend<String> for SmallString<SIZE> {
    fn extend<I: IntoIterator<Item = String>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(&s);
        }
    }
}

impl<const SIZE: usize> fmt::Debug for SmallString<SIZE> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<const SIZE: usize> fmt::Display for SmallString<SIZE> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

macro_rules! eq_str {
    ( $rhs:ty ) => {
        impl<'a, const SIZE: usize> PartialEq<$rhs> for SmallString<SIZE> {
            #[inline(always)]
            fn eq(&self, rhs: &$rhs) -> bool {
                &self[..] == &rhs[..]
            }

            #[inline(always)]
            fn ne(&self, rhs: &$rhs) -> bool {
                &self[..] != &rhs[..]
            }
        }
    };
}

eq_str!(str);
eq_str!(&'a str);
eq_str!(String);
eq_str!(Cow<'a, str>);

#[cfg(feature = "ffi")]
impl<const SIZE: usize> PartialEq<OsStr> for SmallString<SIZE> {
    #[inline(always)]
    fn eq(&self, rhs: &OsStr) -> bool {
        &self[..] == rhs
    }

    #[inline(always)]
    fn ne(&self, rhs: &OsStr) -> bool {
        &self[..] != rhs
    }
}

#[cfg(feature = "ffi")]
impl<'a, const SIZE: usize> PartialEq<&'a OsStr> for SmallString<SIZE> {
    #[inline(always)]
    fn eq(&self, rhs: &&OsStr) -> bool {
        &self[..] == *rhs
    }

    #[inline(always)]
    fn ne(&self, rhs: &&OsStr) -> bool {
        &self[..] != *rhs
    }
}

#[cfg(feature = "ffi")]
impl<const SIZE: usize> PartialEq<OsString> for SmallString<SIZE> {
    #[inline(always)]
    fn eq(&self, rhs: &OsString) -> bool {
        &self[..] == rhs
    }

    #[inline(always)]
    fn ne(&self, rhs: &OsString) -> bool {
        &self[..] != rhs
    }
}

#[cfg(feature = "ffi")]
impl<'a, const SIZE: usize> PartialEq<Cow<'a, OsStr>> for SmallString<SIZE> {
    #[inline(always)]
    fn eq(&self, rhs: &Cow<OsStr>) -> bool {
        self[..] == **rhs
    }

    #[inline(always)]
    fn ne(&self, rhs: &Cow<OsStr>) -> bool {
        self[..] != **rhs
    }
}

impl<const SIZE: usize> PartialEq<SmallString<SIZE>> for SmallString<SIZE> {
    #[inline(always)]
    fn eq(&self, rhs: &SmallString<SIZE>) -> bool {
        &self[..] == &rhs[..]
    }

    #[inline(always)]
    fn ne(&self, rhs: &SmallString<SIZE>) -> bool {
        &self[..] != &rhs[..]
    }
}

impl<const SIZE: usize> Eq for SmallString<SIZE> {}

impl<const SIZE: usize> PartialOrd for SmallString<SIZE> {
    #[inline(always)]
    fn partial_cmp(&self, rhs: &SmallString<SIZE>) -> Option<Ordering> {
        self[..].partial_cmp(&rhs[..])
    }
}

impl<const SIZE: usize> Ord for SmallString<SIZE> {
    #[inline(always)]
    fn cmp(&self, rhs: &SmallString<SIZE>) -> Ordering {
        self[..].cmp(&rhs[..])
    }
}

impl<const SIZE: usize> Hash for SmallString<SIZE> {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self[..].hash(state)
    }
}

/// `SmallString` 的耗尽迭代器。
///
/// 该结构是由 [`SmallString`] 上的 [`drain`] 方法创建的。
///
/// [`drain`]: struct.SmallString.html#method.drain
/// [`SmallString`]: struct.SmallString.html
pub struct Drain<'a> {
    iter: Chars<'a>,
}

impl<'a> Iterator for Drain<'a> {
    type Item = char;

    #[inline(always)]
    fn next(&mut self) -> Option<char> {
        self.iter.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> DoubleEndedIterator for Drain<'a> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<char> {
        self.iter.next_back()
    }
}
