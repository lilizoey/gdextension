/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::generator::functions_common::FnCode;
use crate::generator::{docs, functions_common};
use crate::models::domain::{
    ApiView, Class, ClassLike, ClassMethod, FnQualifier, Function, TyName,
};
use crate::util::ident;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

use super::functions_common::FnDefinition;

pub fn make_virtual_methods_trait(
    class: &Class,
    all_base_names: &[TyName],
    trait_name_str: &str,
    notification_enum_name: &Ident,
    view: &ApiView,
) -> TokenStream {
    let trait_name = ident(trait_name_str);

    let mut virtual_method_fns = make_all_virtual_methods(class, all_base_names, view);
    let special_virtual_methods = special_virtual_methods(notification_enum_name);

    let has_unsafe = !virtual_method_fns.unsafe_trait_methods.is_empty();

    let safe_trait_doc = docs::make_virtual_trait_doc(trait_name_str, class.name(), has_unsafe);

    let safe_trait_methods = virtual_method_fns
        .safe_trait_methods
        .into_iter()
        .map(FnDefinition::into_functions_only)
        .collect::<Vec<_>>();

    let safe_trait = quote! {
        #[doc = #safe_trait_doc]
        #[allow(unused_variables)]
        #[allow(clippy::unimplemented)]
        pub trait #trait_name: crate::obj::GodotClass + crate::private::You_forgot_the_attribute__godot_api<false> {
            #special_virtual_methods
            #( #safe_trait_methods )*
        }
    };

    let unsafe_trait_name = ident(&class.name().unsafe_virtual_trait_name());

    let unsafe_trait_safety_doc = virtual_method_fns
        .unsafe_trait_methods
        .iter()
        .map(|method| method.function_safety.trait_safety_doc())
        .flatten()
        .collect::<Vec<_>>();

    let unsafe_trait_methods = virtual_method_fns
        .unsafe_trait_methods
        .into_iter()
        .map(FnDefinition::into_functions_only)
        .collect::<Vec<_>>();

    let unsafe_trait = if !unsafe_trait_methods.is_empty() {
        quote! {
            /// # Safety
            #( #[doc = #unsafe_trait_safety_doc] )*
            #[allow(unused_variables)]
            #[allow(clippy::unimplemented)]
            pub unsafe trait #unsafe_trait_name: crate::obj::GodotClass + crate::private::You_forgot_the_attribute__godot_api<true> + #trait_name {
                #( #unsafe_trait_methods )*
            }
        }
    } else {
        TokenStream::new()
    };

    quote! {
        #safe_trait
        #unsafe_trait
    }
}

struct VirtualMethodTrait {
    trait_name: Ident,
    is_unsafe: bool,

    special_virtual_methods: Option<TokenStream>,
    virtual_methods: Vec<TokenStream>,
}

impl VirtualMethodTrait {
    fn new(class: &Class, is_unsafe: bool) -> Self {
        let trait_name = if is_unsafe {
            class.name().unsafe_virtual_trait_name()
        } else {
            class.name().virtual_trait_name()
        };

        Self {
            trait_name: ident(&trait_name),
            is_unsafe,
            special_virtual_methods: None,
            virtual_methods: Vec::new(),
        }
    }

    fn set_virtual_methods(&mut self, special_virtual_methods: TokenStream) {
        self.special_virtual_methods = Some(special_virtual_methods);
    }

    fn extend_virtual_methods(&mut self, virtual_methods: Vec<TokenStream>) {
        self.virtual_methods.extend(virtual_methods.into_iter())
    }

    fn into_token_stream(self, trait_doc: String) -> TokenStream {
        let Self {
            is_unsafe,
            trait_name,
            special_virtual_methods,
            virtual_methods,
        } = self;

        let unsafe_ = if is_unsafe {
            Some(quote! { unsafe })
        } else {
            None
        };

        quote! {
            #[doc = #trait_doc]
            #[allow(unused_variables)]
            #[allow(clippy::unimplemented)]
            pub #unsafe_ trait #trait_name: crate::obj::GodotClass + crate::private::You_forgot_the_attribute__godot_api {
                #special_virtual_methods
                #( #virtual_methods )*
            }
        }
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Implementation

fn special_virtual_methods(notification_enum_name: &Ident) -> TokenStream {
    quote! {
        #[doc(hidden)]
        fn register_class(builder: &mut crate::builder::ClassBuilder<Self>) {
            unimplemented!()
        }

        /// Godot constructor, accepting an injected `base` object.
        ///
        /// `base` refers to the base instance of the class, which can either be stored in a `Base<T>` field or discarded.
        /// This method returns a fully-constructed instance, which will then be moved into a [`Gd<T>`][crate::obj::Gd] pointer.
        ///
        /// If the class has a `#[class(init)]` attribute, this method will be auto-generated and must not be overridden.
        fn init(base: crate::obj::Base<Self::Base>) -> Self {
            unimplemented!()
        }

        /// String representation of the Godot instance.
        ///
        /// Override this method to define how the instance is represented as a string.
        /// Used by `impl Display for Gd<T>`, as well as `str()` and `print()` in GDScript.
        fn to_string(&self) -> crate::builtin::GString {
            unimplemented!()
        }

        /// Called when the object receives a Godot notification.
        ///
        /// The type of notification can be identified through `what`. The enum is designed to hold all possible `NOTIFICATION_*`
        /// constants that the current class can handle. However, this is not validated in Godot, so an enum variant `Unknown` exists
        /// to represent integers out of known constants (mistakes or future additions).
        ///
        /// This method is named `_notification` in Godot, but `on_notification` in Rust. To _send_ notifications, use the
        /// [`Object::notify`][crate::engine::Object::notify] method.
        ///
        /// See also in Godot docs:
        /// * [`Object::_notification`](https://docs.godotengine.org/en/stable/classes/class_object.html#class-object-method-notification).
        /// * [Notifications tutorial](https://docs.godotengine.org/en/stable/tutorials/best_practices/godot_notifications.html).
        fn on_notification(&mut self, what: #notification_enum_name) {
            unimplemented!()
        }

        /// Called whenever [`get()`](crate::engine::Object::get) is called or Godot gets the value of a property.
        ///
        /// Should return the given `property`'s value as `Some(value)`, or `None` if the property should be handled normally.
        ///
        /// See also in Godot docs:
        /// * [`Object::_get`](https://docs.godotengine.org/en/stable/classes/class_object.html#class-object-private-method-get).
        fn get_property(&self, property: StringName) -> Option<Variant> {
            unimplemented!()
        }

        /// Called whenever Godot [`set()`](crate::engine::Object::set) is called or Godot sets the value of a property.
        ///
        /// Should set `property` to the given `value` and return `true`, or return `false` to indicate the `property`
        /// should be handled normally.
        ///
        /// See also in Godot docs:
        /// * [`Object::_set`](https://docs.godotengine.org/en/stable/classes/class_object.html#class-object-private-method-set).
        fn set_property(&mut self, property: StringName, value: Variant) -> bool {
            unimplemented!()
        }

    }
}

fn make_virtual_method(method: &ClassMethod) -> Option<FnDefinition> {
    if !method.is_virtual() {
        return None;
    }

    // Virtual methods are never static.
    let qualifier = method.qualifier();
    assert!(matches!(qualifier, FnQualifier::Mut | FnQualifier::Const));

    let definition = functions_common::make_function_definition(
        method,
        &FnCode {
            receiver: functions_common::make_receiver(qualifier, TokenStream::new()),
            // make_return() requests following args, but they are not used for virtual methods. We can provide empty streams.
            varcall_invocation: TokenStream::new(),
            ptrcall_invocation: TokenStream::new(),
        },
        None,
        None,
    );

    Some(definition)
}

struct VirtualMethods {
    safe_trait_methods: Vec<FnDefinition>,
    unsafe_trait_methods: Vec<FnDefinition>,
}

fn make_all_virtual_methods(
    class: &Class,
    all_base_names: &[TyName],
    view: &ApiView,
) -> VirtualMethods {
    let mut safe_trait_methods = Vec::new();
    let mut unsafe_trait_methods = Vec::new();

    for method in class.methods.iter() {
        // Assumes that inner function filters on is_virtual.
        if let Some(method) = make_virtual_method(method) {
            if method.function_safety.is_trait_unsafe() {
                unsafe_trait_methods.push(method)
            } else {
                safe_trait_methods.push(method)
            }
        }
    }

    for base_name in all_base_names {
        let base_class = view.get_engine_class(base_name);
        for method in base_class.methods.iter() {
            if let Some(method) = make_virtual_method(method) {
                if method.function_safety.is_trait_unsafe() {
                    unsafe_trait_methods.push(method)
                } else {
                    safe_trait_methods.push(method)
                }
            }
        }
    }

    VirtualMethods {
        safe_trait_methods,
        unsafe_trait_methods,
    }
}
