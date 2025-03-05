/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::util::ident;
use crate::SubmitFn;
use proc_macro2::{Ident, Literal, Punct, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use regex::Regex;
use std::fs;
use std::path::Path;

pub fn generate_sys_interface_file(
    h_path: &Path,
    sys_gen_path: &Path,
    is_godot_4_0: bool,
    submit_fn: &mut SubmitFn,
) {
    let code = if is_godot_4_0 {
        // Compat for 4.0.x
        // Most polyfills are in godot_exe.rs, fn polyfill_legacy_header()
        // Module `compat_4_0` is directly imported in Rust code, behind #[cfg].
        TokenStream::new()
    } else {
        generate_proc_address_funcs(h_path)
    };

    submit_fn(sys_gen_path.join("interface.rs"), code);
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Implementation

struct GodotFuncPtr {
    name: Ident,
    func_ptr_ty: Ident,
    doc: String,
    ret: TokenStream,
    params: Vec<(Ident, TokenStream)>,
}

fn generate_proc_address_funcs(h_path: &Path) -> TokenStream {
    let header_code = fs::read_to_string(h_path)
        .expect("failed to read gdextension_interface.h for header parsing");
    let func_ptrs = parse_function_pointers(&header_code);

    let mut fptr_decls = vec![];
    let mut fptr_inits = vec![];
    let mut fptr_mock = vec![];
    let mut fptr_mock_construct = vec![];
    let mut fptr_methods = vec![];
    for fptr in func_ptrs {
        let GodotFuncPtr {
            name,
            func_ptr_ty,
            doc,
            ret,
            params,
        } = fptr;

        let name_str = Literal::byte_string(format!("{}\0", name).as_bytes());

        let decl = quote! {
            #[doc = #doc]
            pub #name: crate::#func_ptr_ty,
        };

        // SAFETY: transmute relies on Option<F1> and Option<F2> having the same layout.
        // It might be better to transmute the raw function pointers, but then we have no type names.
        let init = quote! {
            #name: std::mem::transmute::<
                crate::GDExtensionInterfaceFunctionPtr,
                crate::#func_ptr_ty
            >(get_proc_address(crate::c_str(#name_str))),
        };

        let param_names = params.iter().map(|p| &p.0).collect::<Vec<_>>();
        let tys = params.iter().map(|p| &p.1).collect::<Vec<_>>();

        let mock = quote! {
            pub(super) unsafe extern "C" fn #name(#(#param_names: #tys),*) -> #ret {
                panic!("library has not been initialized yet")
            }
        };

        let mock_construct = quote! {
            #name: Some(mock::#name),
        };

        let method = quote! {
            pub unsafe fn #name(&self, #(#param_names: #tys),*) -> #ret {
                self.#name(#(#param_names),*)
            }
        };

        fptr_decls.push(decl);
        fptr_inits.push(init);
        fptr_mock.push(mock);
        fptr_mock_construct.push(mock_construct);
        fptr_methods.push(method);
    }

    // Do not derive Copy -- even though the struct is bitwise-copyable, this is rarely needed and may point to an error.
    let code = quote! {
        pub struct GDExtensionInterface {
            #( #fptr_decls )*
        }

        impl GDExtensionInterface {
            // TODO: Figure out the right safety preconditions. This currently does not have any because incomplete safety docs
            // can cause issues with people assuming they are sufficient.
            #[allow(clippy::missing_safety_doc)]
            pub(crate) unsafe fn load(
                get_proc_address: crate::GDExtensionInterfaceGetProcAddress,
            ) -> Self {
                let get_proc_address = get_proc_address.expect("invalid get_proc_address function pointer");

                Self {
                    #( #fptr_inits )*
                }
            }

            pub(crate) const fn new() -> Self {
                Self {
                    #( #fptr_mock_construct )*
                }
            }
        }

        impl GDExtensionInterface {
            #(#fptr_methods)*
        }

        #[allow(unused_variables)]
        mod mock {
            #(#fptr_mock)*
        }
    };

    code
}

fn parse_function_pointers(header_code: &str) -> Vec<GodotFuncPtr> {
    // See https://docs.rs/regex/latest/regex for docs.
    let regex = Regex::new(
        r"(?xms)
        # x: ignore whitespace and allow line comments (starting with `#`)
        # m: multi-line mode, ^ and $ match start and end of line
        # s: . matches newlines; would otherwise require (:?\n|\r\n|\r)
        ^
        # Start of comment           /**
        /\*\*
        # followed by any characters
        [^*].*?
        # Identifier                 @name variant_can_convert
        @name\s(?P<name>[a-z0-9_]+)
        (?P<doc>
            .+?
        )
        #(?:@param\s([a-z0-9_]+))*?
        #(?:\n|.)+?
        # End of comment             */
        \*/
        .+?
        # Return type:               typedef GDExtensionBool
        # or pointers with space:    typedef void *
        #typedef\s[A-Za-z0-9_]+?\s\*?
        typedef\s(?P<ret>[^(]+?)
        # Function pointer:          (*GDExtensionInterfaceVariantCanConvert)
        \(\*(?P<type>[A-Za-z0-9_]+?)\)
        # Parameters:                (GDExtensionVariantType p_from, GDExtensionVariantType p_to);
        \((?P<params>.*?)\);
        # $ omitted, because there can be comments after `;`
    ",
    )
    .unwrap();

    let mut func_ptrs = vec![];
    'outer: for cap in regex.captures_iter(header_code) {
        let name = cap.name("name");
        let funcptr_ty = cap.name("type");
        let doc = cap.name("doc");
        let ret = cap.name("ret");
        let params = cap.name("params");

        let (Some(name), Some(funcptr_ty), Some(doc), Some(ret), Some(params)) =
            (name, funcptr_ty, doc, ret, params)
        else {
            // Skip unparseable ones, instead of breaking build (could just be a /** */ comment around something else)
            continue;
        };

        let mut params_vec = vec![];

        for param in params.as_str().split(",") {
            let (split, is_pointer) = if param.contains("*") {
                (param.trim().split("*"), true)
            } else {
                (param.trim().split(" "), false)
            };
            let values = split.collect::<Vec<_>>();

            let mut ty = values[0..values.len() - 1].join(" ");
            if is_pointer {
                ty.push_str(" *");
            }

            let name = values[values.len() - 1];

            let Some(ty) = parse_cpp_type(&ty) else {
                continue 'outer;
            };

            params_vec.push((ident(name), ty));
        }

        let Some(ret) = parse_cpp_type(ret.as_str()) else {
            continue;
        };

        func_ptrs.push(GodotFuncPtr {
            name: ident(name.as_str()),
            func_ptr_ty: ident(funcptr_ty.as_str()),
            doc: doc.as_str().replace("\n *", "\n").trim().to_string(),
            ret: ret,
            params: params_vec,
        });
    }

    func_ptrs
}

fn parse_cpp_type(mut s: &str) -> Option<TokenStream> {
    if s.contains(['(', ')']) {
        return None;
    }
    let mut final_ty = vec![];
    let mut is_pointer = false;

    while s.ends_with("*") {
        is_pointer = true;
        s = s.trim();

        if s.starts_with("const ") {
            s = s[6..s.len() - 1].trim();
            final_ty.push(quote! { *const });
        } else {
            s = s[0..s.len() - 1].trim();
            final_ty.push(quote! { *mut });
        }
    }

    let ty = match s.trim() {
        "void" if is_pointer => quote! { std::ffi::c_void },
        "void" => quote! { () },
        "int64_t" => ident("i64").into_token_stream(),
        "int32_t" => ident("i32").into_token_stream(),
        "int16_t" => ident("i16").into_token_stream(),
        "int8_t" => ident("i8").into_token_stream(),
        "uint64_t" => ident("u64").into_token_stream(),
        "uint32_t" => ident("u32").into_token_stream(),
        "uint16_t" => ident("u16").into_token_stream(),
        "uint8_t" => ident("u8").into_token_stream(),
        "size_t" => ident("usize").into_token_stream(),
        "float" => quote! { std::ffi::c_float },
        "double" => quote! { std::ffi::c_double },
        "char" => quote! { std::ffi::c_char },
        other => {
            let id = ident(other);
            quote! {
                crate::#id
            }
        }
    };

    final_ty.push(ty);

    Some(final_ty.into_iter().collect())
}

// fn doxygen_to_rustdoc(c_doc: &str) -> String {
//     // Remove leading stars
//     let mut doc = c_doc .replace("\n * ", "\n");
//
//     // FIXME only compile once
//     let param_regex = Regex::new(r#"@p"#)
// }

#[test]
fn test_parse_function_pointers() {
    let header_code = r#"
/* INTERFACE: ClassDB Extension */

/**
 * @name classdb_register_extension_class
 *
 * Registers an extension class in the ClassDB.
 *
 * Provided struct can be safely freed once the function returns.
 *
 * @param p_library A pointer the library received by the GDExtension's entry point function.
 * @param p_class_name A pointer to a StringName with the class name.
 * @param p_parent_class_name A pointer to a StringName with the parent class name.
 * @param p_extension_funcs A pointer to a GDExtensionClassCreationInfo struct.
 */
typedef void (*GDExtensionInterfaceClassdbRegisterExtensionClass)(GDExtensionClassLibraryPtr p_library, GDExtensionConstStringNamePtr p_class_name, GDExtensionConstStringNamePtr p_parent_class_name, const GDExtensionClassCreationInfo *p_extension_funcs);
		"#;

    let func_ptrs = parse_function_pointers(header_code);
    assert_eq!(func_ptrs.len(), 1);

    let func_ptr = &func_ptrs[0];
    assert_eq!(
        func_ptr.name.to_string(),
        "classdb_register_extension_class"
    );

    assert_eq!(
        func_ptr.func_ptr_ty.to_string(),
        "GDExtensionInterfaceClassdbRegisterExtensionClass"
    );

    assert_eq!(
        func_ptr.doc,
        r#"
 Registers an extension class in the ClassDB.

 Provided struct can be safely freed once the function returns.

 @param p_library A pointer the library received by the GDExtension's entry point function.
 @param p_class_name A pointer to a StringName with the class name.
 @param p_parent_class_name A pointer to a StringName with the parent class name.
 @param p_extension_funcs A pointer to a GDExtensionClassCreationInfo struct.
		 "#
        .trim()
    );
}
