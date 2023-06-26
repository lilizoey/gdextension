/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Godot-types that are Strings.

mod godot_string;
mod macros;
mod node_path;
mod string_chars;
mod string_name;

use godot_ffi::VariantType;
pub use godot_string::*;
pub use node_path::*;
pub use string_name::*;

use super::{meta::VariantMetadata, FromVariant, ToVariant, Variant, VariantConversionError};

impl ToVariant for &str {
    fn to_variant(&self) -> Variant {
        GodotString::from(*self).to_variant()
    }
}

impl ToVariant for String {
    fn to_variant(&self) -> Variant {
        GodotString::from(self).to_variant()
    }
}

impl FromVariant for String {
    fn try_from_variant(variant: &Variant) -> Result<Self, VariantConversionError> {
        Ok(GodotString::try_from_variant(variant)?.to_string())
    }
}

impl VariantMetadata for String {
    fn variant_type() -> VariantType {
        VariantType::String
    }
}

/// Parse a value from a [`GodotString`] or [`StringName`].
///
/// This trait is a Godot-specific version of [`FromStr`](std::str::FromStr). Types that implement this trait
/// can be directly parsed from [`GodotString`] and [`StringName`] without needing to go through [`String`].
pub trait FromGodotStr {
    fn from_godot_string(s: &GodotString) -> Option<Self>
    where
        Self: Sized;
    fn from_string_name(s: &StringName) -> Option<Self>
    where
        Self: Sized;
}

impl FromGodotStr for f64 {
    fn from_godot_string(s: &GodotString) -> Option<Self> {
        if s.as_inner().is_valid_float() {
            Some(s.as_inner().to_float())
        } else {
            None
        }
    }

    fn from_string_name(s: &StringName) -> Option<Self> {
        if s.as_inner().is_valid_float() {
            Some(s.as_inner().to_float())
        } else {
            None
        }
    }
}

impl FromGodotStr for f32 {
    fn from_godot_string(s: &GodotString) -> Option<Self>
    where
        Self: Sized,
    {
        s.parse::<f64>().map(|f| f as f32)
    }

    fn from_string_name(s: &StringName) -> Option<Self>
    where
        Self: Sized,
    {
        s.parse::<f64>().map(|f| f as f32)
    }
}

impl FromGodotStr for i64 {
    fn from_godot_string(s: &GodotString) -> Option<Self>
    where
        Self: Sized,
    {
        if s.as_inner().is_valid_int() {
            Some(s.as_inner().to_int())
        } else {
            None
        }
    }

    fn from_string_name(s: &StringName) -> Option<Self>
    where
        Self: Sized,
    {
        if s.as_inner().is_valid_int() {
            Some(s.as_inner().to_int())
        } else {
            None
        }
    }
}

macro_rules! impl_from_godot_str_try_from {
    ($Base:ty => $Into:ty) => {
        impl FromGodotStr for $Into {
            fn from_godot_string(s: &GodotString) -> Option<Self>
            where
                Self: Sized,
            {
                s.parse::<$Base>()
                    .map(<$Into as TryFrom<$Base>>::try_from)
                    .transpose()
                    .ok()
                    .flatten()
            }

            fn from_string_name(s: &StringName) -> Option<Self>
            where
                Self: Sized,
            {
                s.parse::<$Base>()
                    .map(<$Into as TryFrom<$Base>>::try_from)
                    .transpose()
                    .ok()
                    .flatten()
            }
        }
    };
}

impl_from_godot_str_try_from!(i64 => i32);
impl_from_godot_str_try_from!(i64 => i16);
impl_from_godot_str_try_from!(i64 => i8);
impl_from_godot_str_try_from!(i64 => u128);
impl_from_godot_str_try_from!(i64 => u64);
impl_from_godot_str_try_from!(i64 => u32);
impl_from_godot_str_try_from!(i64 => u16);
impl_from_godot_str_try_from!(i64 => u8);

impl FromGodotStr for i128 {
    fn from_godot_string(s: &GodotString) -> Option<Self>
    where
        Self: Sized,
    {
        s.parse::<i64>().map(<i128 as From<i64>>::from)
    }

    fn from_string_name(s: &StringName) -> Option<Self>
    where
        Self: Sized,
    {
        s.parse::<i64>().map(<i128 as From<i64>>::from)
    }
}
