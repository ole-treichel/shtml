use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{
    Attribute, GenericArgument, Ident, ItemFn, Lifetime, PathArguments, PatType, Result, Signature,
    Type, TypeReference,
};

/// Per-parameter information gathered while transforming a component function.
struct FieldInfo {
    /// A lifetime the generated struct needs to carry (for reference parameters).
    lifetime: Option<Lifetime>,
    /// The field / setter identifier.
    name: Ident,
    /// The value type of the field (e.g. `&'a str` or `Option<u8>`).
    value_ty: TokenStream2,
    /// Whether the field is an `Option<T>`, i.e. a skippable optional prop.
    optional: bool,
    /// For `#[context]` params: the function `build()` calls when the prop was not
    /// passed (`expect_context::<T>` or, for `Option<T>`, `use_context::<T>`).
    context: Option<TokenStream2>,
}

/// Returns `T` if the type is `Option<T>`.
fn option_inner(type_path: &syn::TypePath) -> Option<&Type> {
    let seg = type_path.path.segments.last()?;
    if seg.ident != "Option" {
        return None;
    }
    match &seg.arguments {
        PathArguments::AngleBracketed(args) => args.args.iter().find_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        }),
        _ => None,
    }
}

/// Returns the `#[context]` attribute of a parameter, rejecting arguments.
fn context_attr(attrs: &[Attribute]) -> Result<Option<&Attribute>> {
    let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident("context")) else {
        return Ok(None);
    };
    if !matches!(attr.meta, syn::Meta::Path(_)) {
        return Err(syn::Error::new_spanned(attr, "`#[context]` takes no arguments."));
    }
    #[cfg(not(feature = "context"))]
    return Err(syn::Error::new_spanned(
        attr,
        "`#[context]` requires the `context` feature: `shtml = { features = [\"chaos\", \"context\"] }`.",
    ));
    #[cfg(feature = "context")]
    Ok(Some(attr))
}

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

    let infos = inputs
        .iter()
        .enumerate()
        .map(|(i, fn_arg)| match fn_arg {
            syn::FnArg::Receiver(_) => unreachable!(),
            syn::FnArg::Typed(PatType { attrs, pat, ty, .. }) => {
                let name = match &**pat {
                    syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                    _ => {
                        return Err(syn::Error::new_spanned(
                            pat,
                            "Component parameters must be simple identifiers (e.g., `name: &str`).",
                        ))
                    }
                };

                let context = context_attr(attrs)?;

                match &**ty {
                    Type::Path(type_path) => {
                        let inner = option_inner(type_path);
                        let context = context.map(|_| match inner {
                            Some(inner) => quote! { ::shtml::context::use_context::<#inner> },
                            None => quote! { ::shtml::context::expect_context::<#type_path> },
                        });
                        Ok(FieldInfo {
                            lifetime: None,
                            name,
                            value_ty: quote! { #type_path },
                            optional: inner.is_some() && context.is_none(),
                            context,
                        })
                    }
                    Type::Reference(_) if context.is_some() => Err(syn::Error::new_spanned(
                        ty,
                        "`#[context]` params must be owned types (`Clone + 'static`); the value is cloned out of the context.",
                    )),
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

                        Ok(FieldInfo {
                            lifetime: Some(lifetime.clone()),
                            name,
                            value_ty: quote! { #and_token #lifetime #mutability #elem },
                            optional: false,
                            context: None,
                        })
                    }
                    _ => Err(syn::Error::new_spanned(
                        ty,
                        format!(
                            "Unsupported parameter type for `{}`. Component parameters must be concrete types or references (e.g., `String`, `&str`). Tuple types, trait objects, and function pointers are not supported.",
                            pat.to_token_stream()
                        ),
                    )),
                }
            }
        })
        .collect::<Result<Vec<_>>>()?;

    let lifetime_tokens = infos
        .iter()
        .filter_map(|info| info.lifetime.as_ref())
        .collect::<Vec<_>>();
    let lifetime_tokens = match lifetime_tokens.is_empty() {
        true => quote! {},
        false => quote! {
            <#(#lifetime_tokens,)*>
        },
    };

    let field_names = infos.iter().map(|info| &info.name).collect::<Vec<_>>();

    let struct_fields = infos.iter().map(|info| {
        let name = &info.name;
        let ty = &info.value_ty;
        quote! { #name: #ty }
    });

    let builder_ident = Ident::new(&format!("{ident}Builder"), ident.span());

    // Builder fields track whether each prop was supplied. Required and context
    // fields are wrapped in an extra `Option` (`None` until set); optional
    // `Option<T>` props are stored as-is (`None` already means "skipped").
    let builder_fields = infos.iter().map(|info| {
        let name = &info.name;
        let ty = &info.value_ty;
        if info.optional {
            quote! { #name: #ty }
        } else {
            quote! { #name: Option<#ty> }
        }
    });

    let builder_inits = infos.iter().map(|info| {
        let name = &info.name;
        quote! { #name: None }
    });

    let builder_setters = infos.iter().map(|info| {
        let name = &info.name;
        let ty = &info.value_ty;
        if info.optional {
            quote! {
                pub fn #name(mut self, value: #ty) -> Self {
                    self.#name = value;
                    self
                }
            }
        } else {
            quote! {
                pub fn #name(mut self, value: #ty) -> Self {
                    self.#name = Some(value);
                    self
                }
            }
        }
    });

    let build_fields = infos.iter().map(|info| {
        let name = &info.name;
        if let Some(lookup) = &info.context {
            quote! { #name: self.#name.unwrap_or_else(#lookup) }
        } else if info.optional {
            quote! { #name: self.#name }
        } else {
            let msg = format!("missing required prop `{name}`");
            quote! { #name: self.#name.expect(#msg) }
        }
    });

    let output = quote! {
        #(#attrs)*
        #vis struct #ident #lifetime_tokens {
            #(#struct_fields,)*
        }

        #vis struct #builder_ident #lifetime_tokens {
            #(#builder_fields,)*
        }

        #[allow(clippy::new_without_default)]
        impl #lifetime_tokens #builder_ident #lifetime_tokens {
            pub fn new() -> Self {
                Self { #(#builder_inits,)* }
            }

            #(#builder_setters)*

            pub fn build(self) -> #ident #lifetime_tokens {
                #ident { #(#build_fields,)* }
            }
        }

        impl #lifetime_tokens #ident #lifetime_tokens {
            pub fn builder() -> #builder_ident #lifetime_tokens {
                #builder_ident::new()
            }

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
