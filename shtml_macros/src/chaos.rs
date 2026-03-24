use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{Ident, ItemFn, Lifetime, PatType, Result, Signature, Type, TypeReference};

pub fn component_macro(item_fn: ItemFn) -> Result<TokenStream2> {
    let ItemFn {
        vis,
        sig,
        block,
        attrs,
        ..
    } = item_fn;
    let Signature {
        ident,
        inputs,
        // TODO generics
        // TODO verify output type
        ..
    } = sig;
    // Check for `self` receiver early
    for arg in &inputs {
        if let syn::FnArg::Receiver(receiver) = arg {
            return Err(syn::Error::new_spanned(
                receiver,
                "Component functions cannot use `self`. Define components as free functions: `fn MyComponent(attr: Type) -> Component`.",
            ));
        }
    }

    let field_names = inputs
        .iter()
        .map(|fn_arg| match fn_arg {
            syn::FnArg::Receiver(_) => unreachable!(),
            syn::FnArg::Typed(pat_type) => &pat_type.pat,
        })
        .collect::<Vec<_>>();

    let fields = inputs
        .iter()
        .enumerate()
        .map(|(i, fn_arg)| match fn_arg {
            syn::FnArg::Receiver(_) => unreachable!(),
            syn::FnArg::Typed(PatType { pat, ty, .. }) => match &**ty {
                Type::Path(type_path) => Ok((None, quote! { #pat: #type_path })),
                Type::Reference(TypeReference {
                    and_token,
                    lifetime,
                    mutability,
                    elem,
                }) => {
                    if i > 25 {
                        return Err(syn::Error::new_spanned(
                            ty,
                            "Component functions support at most 26 reference parameters.",
                        ));
                    }
                    let lifetime = match lifetime {
                        Some(lifetime) => lifetime.to_owned(),
                        None => Lifetime {
                            apostrophe: Span::call_site(),
                            ident: Ident::new(
                                &(((i + 97) as u8) as char).to_string(),
                                Span::call_site(),
                            ),
                        },
                    };

                    Ok((
                        Some(lifetime.clone()),
                        quote! { #pat: #and_token #lifetime #mutability #elem },
                    ))
                }
                _ => Err(syn::Error::new_spanned(
                    ty,
                    format!(
                        "Unsupported parameter type for `{}`. Component parameters must be concrete types or references (e.g., `String`, `&str`). Tuple types, trait objects, and function pointers are not supported.",
                        pat.to_token_stream()
                    ),
                )),
            },
        })
        .collect::<Result<Vec<_>>>()?;

    let lifetime_tokens = fields
        .iter()
        .filter_map(|(lifetime, _)| match lifetime {
            Some(lifetime) => Some(lifetime),
            None => None,
        })
        .collect::<Vec<_>>();
    let lifetime_tokens = match lifetime_tokens.is_empty() {
        true => quote! {},
        false => quote! {
            <#(#lifetime_tokens,)*>
        },
    };

    let fields = fields.iter().map(|(_, field)| field).collect::<Vec<_>>();

    let output = quote! {
        #(#attrs,)*
        #vis struct #ident #lifetime_tokens {
            #(#fields,)*
        }

        impl #lifetime_tokens #ident #lifetime_tokens {
            pub fn to_component(&self) -> Component {
                let Self { #(#field_names,)* } = self;
                #block
            }
        }

        impl #lifetime_tokens Render for #ident #lifetime_tokens {
            fn render_to_string(&self, buffer: &mut String) {
                buffer.push_str(&self.to_component().to_string())
            }
        }
    };

    Ok(output)
}
