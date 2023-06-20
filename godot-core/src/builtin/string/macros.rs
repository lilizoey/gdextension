/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

#![macro_use]

macro_rules! impl_rust_string_conv {
    ($Ty:ty) => {
        impl<S> From<S> for $Ty
        where
            S: AsRef<str>,
        {
            fn from(string: S) -> Self {
                let intermediate = GodotString::from(string.as_ref());
                Self::from(&intermediate)
            }
        }

        impl From<&$Ty> for String {
            fn from(string: &$Ty) -> Self {
                let intermediate = GodotString::from(string);
                Self::from(&intermediate)
            }
        }

        impl From<$Ty> for String {
            fn from(string: $Ty) -> Self {
                Self::from(&string)
            }
        }

        impl std::str::FromStr for $Ty {
            type Err = std::convert::Infallible;

            fn from_str(string: &str) -> Result<Self, Self::Err> {
                Ok(Self::from(string))
            }
        }
    };
}

macro_rules! impl_string_common_methods {
    ($Ty:ty) => {
        impl $Ty {
            /// Returns `true` if the given text matches a prefix of this string.
            #[doc(alias = "begins_with")]
            pub fn starts_with<S: Into<GodotString>>(&self, text: S) -> bool {
                self.as_inner().begins_with(text.into())
            }

            /// Returns an array containing the bigrams (pairs of consecutive characters) of this string.
            pub fn bigrams(&self) -> crate::builtin::PackedStringArray {
                self.as_inner().bigrams()
            }

            /// Convertes a string representing a binary number into an integer.
            ///
            /// `-` can be used to represent negative numbers.
            ///
            /// The string may optionally be prefixed by `0b`.
            pub fn bin_to_int(&self) -> i64 {
                self.as_inner().bin_to_int()
            }

            pub fn c_escape(&self) -> GodotString {
                self.as_inner().c_escape()
            }

            pub fn c_unescape(&self) -> GodotString {
                self.as_inner().c_escape()
            }

            /// Convert camelCase, snake_case, PascalCase, and combinations of those into a space separated
            /// string where each word is capitalized.
            #[doc(alias = "capitalize")]
            pub fn to_title_case(&self) -> GodotString {
                self.as_inner().capitalize()
            }

            #[doc(alias = "casecmp_to")]
            pub fn case_cmp(&self, other: &GodotString) -> std::cmp::Ordering {
                use std::cmp::Ordering;

                let ordering = self.as_inner().casecmp_to(other.clone());

                match ordering {
                    -1 => Ordering::Less,
                    0 => Ordering::Equal,
                    1 => Ordering::Greater,
                    _ => unreachable!(),
                }
            }

            pub fn contains<S: Into<GodotString>>(&self, what: S) -> bool {
                self.as_inner().contains(what.into())
            }

            fn count_range(range: impl std::ops::RangeBounds<usize>) -> (usize, usize) {
                use std::ops::Bound::*;

                let start = match range.start_bound() {
                    Unbounded => 0,
                    Included(i) => *i,
                    Excluded(i) => i.saturating_add(1),
                };

                let end = match range.end_bound() {
                    Unbounded => 0,
                    Included(i) => *i,
                    Excluded(i) => i.saturating_sub(1),
                };

                (start, end)
            }

            pub fn count<S: Into<GodotString>>(
                &self,
                what: S,
                range: impl std::ops::RangeBounds<usize>,
            ) -> usize {
                let (start, end) = Self::count_range(range);

                self.as_inner().count(what.into(), start as i64, end as i64) as usize
            }

            #[doc(alias = "countn")]
            pub fn count_ignore_case<S: Into<GodotString>>(
                &self,
                what: S,
                range: impl std::ops::RangeBounds<usize>,
            ) -> usize {
                let (start, end) = Self::count_range(range);

                self.as_inner()
                    .countn(what.into(), start as i64, end as i64) as usize
            }

            pub fn dedent(&self) -> GodotString {
                self.as_inner().dedent()
            }

            pub fn find<S: Into<GodotString>>(
                &self,
                text: S,
                from: Option<usize>,
            ) -> Option<usize> {
                let index = self.as_inner().find(text.into(), from.unwrap_or(0) as i64);

                if index == -1 {
                    None
                } else {
                    Some(index as usize)
                }
            }

            #[doc(alias = "findn")]
            pub fn find_ignore_case<S: Into<GodotString>>(
                &self,
                text: S,
                from: Option<usize>,
            ) -> Option<usize> {
                let index = self.as_inner().findn(text.into(), from.unwrap_or(0) as i64);

                if index == -1 {
                    None
                } else {
                    Some(index as usize)
                }
            }
        }
    };
}

pub(super) use impl_string_common_methods;

// godotstring to add:

// stringname to add:

// intentionally not added:
// String.chr() - we already have From<char>.
// format - we already have `format!`
