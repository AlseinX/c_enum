use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Comma, Enum},
    Attribute, Error, Ident, ItemEnum, Meta, Path, PathArguments, PathSegment, Result, Token, Type,
    TypePath, Variant,
};

use crate::utils::{extract_meta_from_lists, is_doc};

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
    let (repr, repr_size) = parse_repr(attrs, &mut deferred_error, variants, enum_token)?;

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

    let docs = attrs.iter().filter(is_doc);

    let variant_ids = variants
        .iter()
        .map(|v| {
            let mut ident = v.ident.clone();
            ident.set_span(Span::mixed_site());
            ident
        })
        .collect::<Vec<_>>();

    let variant_docs = variants.iter().map(|v| {
        let docs = v.attrs.iter().filter(is_doc);
        quote!(#(#docs)*)
    });

    let args = args.iter();

    let name = ident.to_string();
    let mut impls = Default::default();

    extract_meta_from_lists(attrs, "derive").for_each(super::delegate::delegate(
        &name, &target, &origin, attrs, &mut impls,
    ));

    let types = [
        ("8", Some(size_of::<u8>())),
        ("16", Some(size_of::<u16>())),
        ("32", Some(size_of::<u32>())),
        ("64", Some(size_of::<u64>())),
        ("128", Some(size_of::<u128>())),
        ("size", None),
    ];

    let from_impls = types.iter().map(|&(name, size)| {
        let iname = Type::Path(TypePath {
            qself: None,
            path: Ident::new(&(String::from("i") + name), Span::call_site()).into(),
        });
        let uname = Type::Path(TypePath {
            qself: None,
            path: Ident::new(&(String::from("u") + name), Span::call_site()).into(),
        });

        let mut impls = TokenStream::new();

        if infallible(repr_size, size) {
            from_repr(&mut impls, &target, &iname, &uname)
        } else {
            try_from_repr(&mut impls, &target, &repr, &iname, &uname)
        };

        if infallible(size, repr_size) {
            repr_from(&mut impls, &target, &repr, &iname, &uname)
        } else {
            repr_try_from(&mut impls, &target, &repr, &iname, &uname)
        };

        impls
    });

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
                #(
                    #variant_docs
                    pub const #variant_ids: Self = Self { repr: #origin::#variant_ids as _ };
                )*
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

            #(#from_impls)*

            #impls
        };

        #(#docs)*
        #[derive(Clone, Copy, PartialEq, Eq, ::ffi_enum::__private::NoDerive)]
        #(#[#args])*
        #[ffi_enum_origin(#input)]
        #[repr(transparent)]
        #vis struct #ident {
            pub repr: #repr,
        }

        #deferred_error
    })
}

fn parse_repr(
    attrs: &[Attribute],
    deferred_error: &mut Option<TokenStream>,
    variants: &Punctuated<Variant, Comma>,
    enum_token: &Enum,
) -> Result<(Type, Option<usize>)> {
    Ok(extract_repr(attrs)?.map(|t| {
        let st=t.to_token_stream().to_string();
        let [_,rest@..] = st.as_bytes() else {
            panic!("not supported repr type: {}", st);
        };
        let size = match rest {
            b"8" => Some(1),
            b"16" => Some(2),
            b"32" => Some(4),
            b"64" => Some(8),
            b"size" => None,
            _ => panic!("not supported repr type: {}", st),
        };
        (t, size)
    })
        .unwrap_or_else(|| {
            const MAX_8: u64 = (u8::MAX as u64) + 1;
            const MAX_16: u64 = (u16::MAX as u64) + 1;
            const MAX_32: u64 = (u32::MAX as u64) + 1;
            let (name,size) = if variants.iter().all(|v| v.discriminant.is_none()) {
                #[allow(clippy::match_overlapping_arm)]
                match variants.len() as _ {
                    ..=MAX_8 => ("u8", Some(1)),
                    ..=MAX_16 => ("u16", Some(2)),
                    ..=MAX_32 => ("u32", Some(4)),
                    _ => ("u64", Some(8)),
                }
            } else {
                let span = variants
                    .iter()
                    .find_map(|v| v.discriminant.as_ref())
                    .unwrap()
                    .1
                    .span();
                *deferred_error = Some(
                    Error::new(
                        span,
                        "Cannot infer repr type for enum with manually specified discriminants. Try manually specifiying the repr type using the `repr` attribute.",
                    )
                    .into_compile_error()
                );
                ("u64", Some(8))
            };
            (Type::Path(TypePath {
                qself: None,
                path: Ident::new(name, enum_token.span).into(),
            }), size)
        }))
}

fn extract_repr(attrs: &[Attribute]) -> Result<Option<Type>> {
    let mut iter = extract_meta_from_lists(attrs, "repr").filter_map(|meta| {
        let Meta::Path(path) = meta else { return None };
        let s = path.get_ident()?.to_string();
        let [b'i' | b'u', rest @ ..] = s.as_bytes() else {
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

fn infallible(from_width: Option<usize>, to_width: Option<usize>) -> bool {
    match (from_width, to_width) {
        (Some(from), Some(to)) => from <= to,
        (Some(from), None) => from <= 2,
        (None, Some(to)) => to >= 8,
        (None, None) => true,
    }
}

fn try_from_repr(impls: &mut TokenStream, from: &Type, repr: &Type, to_i: &Type, to_u: &Type) {
    impls.extend(
    quote! {
        impl ::core::convert::TryFrom<#from> for #to_i {
            type Error = ::ffi_enum::Error;

            #[inline]
            fn try_from(e: #from) -> ::core::result::Result<Self, Self::Error> {
                ::core::result::Result::Ok(<#to_i as ::core::convert::TryFrom<#repr>>::try_from(e.repr)?)
            }
        }

        impl ::core::convert::TryFrom<#from> for #to_u {
            type Error = ::ffi_enum::Error;

            #[inline]
            fn try_from(e: #from) -> ::core::result::Result<Self, Self::Error> {
                ::core::result::Result::Ok(<#to_u as ::core::convert::TryFrom<#repr>>::try_from(e.repr)?)
            }
        }
    });
}

fn from_repr(impls: &mut TokenStream, from: &Type, to_i: &Type, to_u: &Type) {
    impls.extend(quote! {
        impl ::core::convert::From<#from> for #to_i {
            #[inline]
            fn from(e: #from) -> Self {
                e.repr as #to_i
            }
        }

        impl ::core::convert::From<#from> for #to_u {
            #[inline]
            fn from(e: #from) -> Self {
                e.repr as #to_u
            }
        }
    });
}

fn repr_try_from(impls: &mut TokenStream, to: &Type, repr: &Type, from_i: &Type, from_u: &Type) {
    impls.extend(
    quote! {
        impl ::core::convert::TryFrom<#from_i> for #to {
            type Error = ::ffi_enum::Error;

            #[inline]
            fn try_from(repr: #from_i) -> ::core::result::Result<Self, Self::Error> {
                ::core::result::Result::Ok(Self{repr: <#repr as ::core::convert::TryFrom<#from_i>>::try_from(repr)?})
            }
        }

        impl ::core::convert::TryFrom<#from_u> for #to {
            type Error = ::ffi_enum::Error;

            #[inline]
            fn try_from(repr: #from_u) -> ::core::result::Result<Self, Self::Error> {
                ::core::result::Result::Ok(Self{repr: <#repr as ::core::convert::TryFrom<#from_u>>::try_from(repr)?})
            }
        }
    });
}

fn repr_from(impls: &mut TokenStream, to: &Type, repr: &Type, from_i: &Type, from_u: &Type) {
    impls.extend(quote! {
        impl ::core::convert::From<#from_i> for #to {
            #[inline]
            fn from(repr: #from_i) -> Self {
                Self{ repr: repr as #repr}
            }
        }

        impl ::core::convert::From<#from_u> for #to {
            #[inline]
            fn from(repr: #from_u) -> Self {
                Self{ repr: repr as #repr}
            }
        }
    });
}
