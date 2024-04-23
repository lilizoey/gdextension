/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::generator::default_parameters;
use crate::models::domain::{FnParam, FnQualifier, Function, RustTy};
use crate::util::safe_ident;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

pub struct FnReceiver {
    /// `&self`, `&mut self`, (none)
    pub param: TokenStream,

    /// `ptr::null_mut()`, `self.object_ptr`, `self.sys_ptr`, (none)
    pub ffi_arg: TokenStream,

    /// `Self::`, `self.`
    pub self_prefix: TokenStream,
}

impl FnReceiver {
    /// No receiver, not even static `Self`
    pub fn global_function() -> FnReceiver {
        FnReceiver {
            param: TokenStream::new(),
            ffi_arg: TokenStream::new(),
            self_prefix: TokenStream::new(),
        }
    }
}

pub struct FnCode {
    pub receiver: FnReceiver,
    pub varcall_invocation: TokenStream,
    pub ptrcall_invocation: TokenStream,
}

pub struct FnDefinition {
    pub functions: TokenStream,
    pub builders: TokenStream,
    pub function_safety: FunctionSafety,
}

impl FnDefinition {
    pub fn into_functions_only(self) -> TokenStream {
        assert!(
            self.builders.is_empty(),
            "definition of this function should not have any builders"
        );

        self.functions
    }
}

pub struct FnDefinitions {
    pub functions: TokenStream,
    pub builders: TokenStream,
}

impl FnDefinitions {
    /// Combines separate code from multiple function definitions into one, split by functions and builders.
    pub fn expand(definitions: impl Iterator<Item = FnDefinition>) -> FnDefinitions {
        // Collect needed because borrowed by 2 closures
        let definitions: Vec<_> = definitions.collect();
        let functions = definitions.iter().map(|def| &def.functions);
        let structs = definitions.iter().map(|def| &def.builders);

        FnDefinitions {
            functions: quote! { #( #functions )* },
            builders: quote! { #( #structs )* },
        }
    }
}

pub fn make_function_definition(
    sig: &dyn Function,
    code: &FnCode,
    custom_function_safety_doc: Option<String>,
    custom_trait_safety_doc: Option<String>,
) -> FnDefinition {
    let has_default_params = default_parameters::function_uses_default_params(sig);
    let vis = if has_default_params {
        // Public API mapped by separate function.
        // Needs to be crate-public because default-arg builder lives outside of the module.
        quote! { pub(crate) }
    } else {
        make_vis(sig.is_private())
    };

    let mut function_safety = FunctionSafety::from_sig(sig);

    function_safety.custom_function_safety_doc = custom_function_safety_doc;
    function_safety.custom_trait_safety_doc = custom_trait_safety_doc;

    let maybe_unsafe = function_safety.function_unsafe();
    let maybe_safety_doc = function_safety.function_safety_doc();

    let maybe_safety_doc = if !maybe_safety_doc.is_empty() {
        quote! {
            /// # Safety
            #( #[doc = #maybe_safety_doc] )*
        }
    } else {
        TokenStream::new()
    };

    let [params, param_types, arg_names] = make_params_exprs(sig.params());

    let rust_function_name_str = sig.name();
    let primary_fn_name = if has_default_params {
        format_ident!("{}_full", safe_ident(rust_function_name_str))
    } else {
        safe_ident(rust_function_name_str)
    };

    let (default_fn_code, default_structs_code) = if has_default_params {
        default_parameters::make_function_definition_with_defaults(sig, code, &primary_fn_name)
    } else {
        (TokenStream::new(), TokenStream::new())
    };

    let return_ty = &sig.return_value().type_tokens();
    let call_sig = quote! {
        ( #return_ty, #(#param_types),* )
    };

    let return_decl = &sig.return_value().decl;

    let receiver_param = &code.receiver.param;
    let primary_function = if sig.is_virtual() {
        // Virtual functions

        quote! {
            #maybe_safety_doc
            #maybe_unsafe fn #primary_fn_name(
                #receiver_param
                #( #params, )*
            ) #return_decl {
                unimplemented!()
            }
        }
    } else if sig.is_vararg() {
        // Varargs (usually varcall, but not necessarily -- utilities use ptrcall)

        // If the return type is not Variant, then convert to concrete target type
        let varcall_invocation = &code.varcall_invocation;

        // TODO Utility functions: update as well.
        if code.receiver.param.is_empty() {
            quote! {
                #maybe_safety_doc
                #vis #maybe_unsafe fn #primary_fn_name(
                    #receiver_param
                    #( #params, )*
                    varargs: &[Variant]
                ) #return_decl {
                    type CallSig = #call_sig;

                    let args = (#( #arg_names, )*);

                    unsafe {
                        #varcall_invocation
                    }
                }
            }
        } else {
            let try_return_decl = &sig.return_value().call_result_decl();
            let try_fn_name = format_ident!("try_{}", rust_function_name_str);

            // Note: all varargs functions are non-static, which is why there are some shortcuts in try_*() argument forwarding.
            // This can be made more complex if ever necessary.

            quote! {
                /// # Panics
                /// This is a _varcall_ method, meaning parameters and return values are passed as `Variant`.
                /// It can detect call failures and will panic in such a case.
                #maybe_safety_doc
                #vis #maybe_unsafe fn #primary_fn_name(
                    #receiver_param
                    #( #params, )*
                    varargs: &[Variant]
                ) #return_decl {
                    Self::#try_fn_name(self, #( #arg_names, )* varargs)
                        .unwrap_or_else(|e| panic!("{e}"))
                }

                /// # Return type
                /// This is a _varcall_ method, meaning parameters and return values are passed as `Variant`.
                /// It can detect call failures and will return `Err` in such a case.
                #maybe_safety_doc
                #vis #maybe_unsafe fn #try_fn_name(
                    #receiver_param
                    #( #params, )*
                    varargs: &[Variant]
                ) #try_return_decl {
                    type CallSig = #call_sig;

                    let args = (#( #arg_names, )*);

                    unsafe {
                        #varcall_invocation
                    }
                }
            }
        }
    } else {
        // Always ptrcall, no varargs

        let ptrcall_invocation = &code.ptrcall_invocation;

        quote! {
            #maybe_safety_doc
            #vis #maybe_unsafe fn #primary_fn_name(
                #receiver_param
                #( #params, )*
            ) #return_decl {
                type CallSig = #call_sig;

                let args = (#( #arg_names, )*);

                unsafe {
                    #ptrcall_invocation
                }
            }
        }
    };

    FnDefinition {
        functions: quote! {
            #primary_function
            #default_fn_code
        },
        builders: default_structs_code,
        function_safety,
    }
}

pub fn make_receiver(qualifier: FnQualifier, ffi_arg_in: TokenStream) -> FnReceiver {
    assert_ne!(qualifier, FnQualifier::Global, "expected class");

    let param = match qualifier {
        FnQualifier::Const => quote! { &self, },
        FnQualifier::Mut => quote! { &mut self, },
        FnQualifier::Static => quote! {},
        FnQualifier::Global => quote! {},
    };

    let (ffi_arg, self_prefix);
    if matches!(qualifier, FnQualifier::Static) {
        ffi_arg = quote! { std::ptr::null_mut() };
        self_prefix = quote! { Self:: };
    } else {
        ffi_arg = ffi_arg_in;
        self_prefix = quote! { self. };
    };

    FnReceiver {
        param,
        ffi_arg,
        self_prefix,
    }
}

pub fn make_params_and_args(method_args: &[&FnParam]) -> (Vec<TokenStream>, Vec<TokenStream>) {
    method_args
        .iter()
        .map(|param| {
            let param_name = &param.name;
            let param_ty = &param.type_;

            (quote! { #param_name: #param_ty }, quote! { #param_name })
        })
        .unzip()
}

pub fn make_vis(is_private: bool) -> TokenStream {
    if is_private {
        quote! { pub(crate) }
    } else {
        quote! { pub }
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Implementation

fn make_params_exprs(method_args: &[FnParam]) -> [Vec<TokenStream>; 3] {
    let mut params = vec![];
    let mut param_types = vec![];
    let mut arg_names = vec![];

    for param in method_args.iter() {
        let param_name = &param.name;
        let param_ty = &param.type_;

        params.push(quote! { #param_name: #param_ty });
        param_types.push(quote! { #param_ty });
        arg_names.push(quote! { #param_name });
    }

    [params, param_types, arg_names]
}

/// Whether a function can be completely safe, or if some `unsafe` is needed.
pub struct FunctionSafety {
    /// The function has pointer arguments, and so must be `unsafe`.
    pub pointer_args: Vec<Ident>,
    /// The function returns a pointer, and so must be declared in an `unsafe` trait.
    pub has_pointer_return: bool,

    /// The function has a custom safety document, and so must be `unsafe`.
    pub custom_function_safety_doc: Option<String>,
    /// The function has a custom trait safety document, and so must be declared in an `unsafe` trait.
    pub custom_trait_safety_doc: Option<String>,
}

impl FunctionSafety {
    fn from_sig(sig: &dyn Function) -> Self {
        // A function must be an unsafe function if any of its arguments are pointers, since the caller must provide valid pointers.
        let pointer_args = sig
            .params()
            .iter()
            .filter(|param| matches!(param.type_, RustTy::RawPointer { .. }))
            .map(|param| param.name.clone())
            .collect();

        // A function must be declared in an unsafe trait if it returns a pointer, since the implementor must return valid pointers.
        let has_pointer_return =
            matches!(sig.return_value().type_, Some(RustTy::RawPointer { .. }));

        Self {
            pointer_args,
            has_pointer_return,
            custom_function_safety_doc: None,
            custom_trait_safety_doc: None,
        }
    }

    pub fn is_function_unsafe(&self) -> bool {
        !self.pointer_args.is_empty() || self.custom_function_safety_doc.is_some()
    }

    pub fn function_unsafe(&self) -> TokenStream {
        if self.is_function_unsafe() {
            quote! { unsafe }
        } else {
            TokenStream::new()
        }
    }

    pub fn function_safety_doc(&self) -> Vec<String> {
        let Self {
            pointer_args,
            custom_function_safety_doc,
            ..
        } = self;

        let mut doc = Vec::new();

        for arg in pointer_args {
            doc.push(format!("* The caller must ensure {arg} is a valid pointer according to what Godot expects this function to be called with."));
        }

        if let Some(custom_doc) = custom_function_safety_doc.as_ref() {
            doc.push(custom_doc.clone())
        }

        doc
    }

    pub fn is_trait_unsafe(&self) -> bool {
        self.has_pointer_return || self.custom_trait_safety_doc.is_some()
    }

    pub fn trait_safety_doc(&self) -> Vec<String> {
        let Self {
            has_pointer_return,
            custom_trait_safety_doc,
            ..
        } = self;

        let mut doc = Vec::new();

        if *has_pointer_return {
            doc.push(format!("This function returns a pointer, the implementer must ensure the returned pointer is valid for what Godot expects."))
        }

        if let Some(custom_doc) = custom_trait_safety_doc.as_ref() {
            doc.push(custom_doc.clone())
        }

        doc
    }
}
