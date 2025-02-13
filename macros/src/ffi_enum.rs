use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Attribute, Error, Ident, ItemEnum, Meta, Path, PathArguments, PathSegment, Result, Token, Type,
    TypePath,
};

use crate::utils::extract_meta_from_lists;

pub struct Args(Punctuated<Meta, Token![,]>);

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self(Punctuated::parse_terminated(input)?))
    }
}

pub fn handle(Args(args): Args, input: ItemEnum) -> Result<TokenStream> {
    let ItemEnum {
        ref attrs,
        ref vis,
        ref enum_token,
        ref ident,
        ref variants,
        ..
    } = input;

    if let Some(v) = variants.iter().find(|v| !v.fields.is_empty()) {
        return Err(Error::new(
            v.fields.span(),
            "ffi_enum cannot contain fields",
        ));
    }

    let mut deferred_error = None;
    let repr = extract_repr(attrs)?
        .unwrap_or_else(|| {
            const MAX_8: u64 = (u8::MAX as u64) + 1;
            const MAX_16: u64 = (u16::MAX as u64) + 1;
            const MAX_32: u64 = (u32::MAX as u64) + 1;
            let name = if variants.iter().all(|v| v.discriminant.is_none()) {
                match variants.len() as _ {
                    ..MAX_8 => "u8",
                    ..MAX_16 => "u16",
                    ..MAX_32 => "u32",
                    _ => "u64",
                }
            } else {
                let span = variants
                    .iter()
                    .find_map(|v| v.discriminant.as_ref())
                    .unwrap()
                    .1
                    .span();
                deferred_error = Some(
                    Error::new(
                        span,
                        "Cannot infer repr type for enum with manually specified discriminants. Try manually specifiying the repr type using the `repr` attribute.",
                    )
                    .into_compile_error()
                );
                "u64"
            };
            Type::Path(TypePath {
                qself: None,
                path: Ident::new(name, enum_token.span).into(),
            })
        });

    let target = Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: None,
            segments: [
                PathSegment {
                    ident: Ident::new("self", ident.span()),
                    arguments: PathArguments::None,
                },
                PathSegment {
                    ident: ident.clone(),
                    arguments: PathArguments::None,
                },
            ]
            .into_iter()
            .collect(),
        },
    });

    let origin = Type::Path(TypePath {
        qself: None,
        path: ident.clone().into(),
    });

    let variant_ids = variants
        .iter()
        .map(|v| {
            let mut ident = v.ident.clone();
            ident.set_span(Span::mixed_site());
            ident
        })
        .collect::<Vec<_>>();

    let args = args.iter();

    let name = ident.to_string();
    let mut impls = Default::default();

    extract_meta_from_lists(attrs, "derive").for_each(super::delegate::delegate(
        &name, &target, &origin, &attrs, &mut impls,
    ));

    Ok(quote! {
        #[allow(unreachable_code)]
        const _: () = {
            #[non_exhaustive]
            #input

            impl ::ffi_enum::FfiEnum for #target {
                type Enum = #origin;
                type Repr = #repr;
                const UNKNOWN: Self = {
                    let mut result: #repr = 0;
                    while #(result == #target::#variant_ids.repr ||)* false {
                        result += 1;
                    }
                    #target { repr: result }
                };
            }

            #[allow(dead_code, non_upper_case_globals)]
            impl #target {
                #(pub const #variant_ids: Self = Self { repr: #origin::#variant_ids as _ };)*
            }

            impl ::core::convert::TryFrom<#target> for #origin {
                type Error = ::ffi_enum::Error;

                #[inline]
                fn try_from(e: #target) -> ::core::result::Result<Self, Self::Error> {
                    ::core::result::Result::Ok(match e {
                        #(#target::#variant_ids => Self::#variant_ids,)*
                        _ => return ::core::result::Result::Err(::ffi_enum::Error::UnknownVariant),
                    })
                }
            }

            impl ::core::convert::From<#origin> for #target {
                #[inline]
                fn from(e: #origin) -> Self {
                    match e {
                        #(#origin::#variant_ids => Self::#variant_ids,)*
                    }
                }
            }

            impl ::core::convert::From<#repr> for #target {
                #[inline]
                fn from(repr: #repr) -> Self {
                    Self { repr }
                }
            }

            impl ::core::convert::From<#target> for #repr {
                #[inline]
                fn from(e: #target) -> Self {
                    e.repr
                }
            }

            #impls
        };

        #[derive(Clone, Copy, PartialEq, Eq, ::ffi_enum::__private::NoDerive)]
        #(#[#args])*
        #[ffi_enum_origin(#input)]
        #[repr(transparent)]
        #vis struct #ident {
            repr: #repr,
        }

        #deferred_error
    })
}

fn extract_repr(attrs: &[Attribute]) -> Result<Option<Type>> {
    let mut iter = extract_meta_from_lists(attrs, "repr").filter_map(|meta| {
        let Meta::Path(path) = meta else { return None };
        let s = path.get_ident()?.to_string();
        let [b'i' | b'u', rest @ ..] = s.as_str().as_bytes() else {
            return None;
        };
        if !matches!(rest, b"8" | b"16" | b"32" | b"64" | b"128" | b"size") {
            return None;
        }
        Some(Type::Path(TypePath { qself: None, path }))
    });

    let Some(result) = iter.next() else {
        return Ok(None);
    };

    if let Some(dup) = iter.next() {
        return Err(Error::new_spanned(dup, "duplicate repr type found"));
    }

    Ok(Some(result))
}
