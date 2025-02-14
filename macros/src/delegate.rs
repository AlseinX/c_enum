use proc_macro2::{Span, TokenStream};
use quote::quote_spanned;
use syn::{
    Attribute, Expr, ExprLit, Ident, Lit, Meta, MetaNameValue, Path, PathArguments, PathSegment,
    Token, Type,
};

use crate::utils::extract_meta_from_lists;

struct Context<'a, 'b> {
    name: &'a str,
    span: Span,
    target: &'a Type,
    origin: &'a Type,
    attrs: &'a [Attribute],
    impls: &'b mut TokenStream,
}

impl Context<'_, '_> {
    fn debug(self) {
        let Self {
            span,
            target,
            origin,
            impls,
            ..
        } = self;

        impls.extend(quote_spanned! { span =>
            impl ::core::fmt::Debug for #target {
                #[inline(always)]
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    if let Ok(origin) = <#origin as ::core::convert::TryFrom<#target>>::try_from(*self) {
                        <#origin as ::core::fmt::Debug>::fmt(&origin, f)?;
                        write!(f, "({})", self.repr)
                    } else {
                        write!(f, "<Unknown>({})", self.repr)
                    }
                }
            }
        })
    }

    fn display(self) {
        let Self {
            span,
            target,
            origin,
            impls,
            ..
        } = self;

        impls.extend(quote_spanned! { span =>
            impl ::core::fmt::Display for #target {
                #[inline(always)]
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    if let Ok(origin) = <#origin as ::core::convert::TryFrom<#target>>::try_from(*self) {
                        <#origin as ::core::fmt::Display>::fmt(&origin, f)
                    } else {
                        write!(f, "<Unknown>")
                    }
                }
            }
        })
    }

    fn error(mut self) {
        let Self {
            span,
            target,
            ref mut impls,
            ..
        } = self;

        impls.extend(quote_spanned! { span =>
            impl ::core::error::Error for #target { }
        });

        self.display();
    }

    fn hash(self) {
        let Self {
            span,
            target,
            origin,
            impls,
            ..
        } = self;

        impls.extend(quote_spanned! { span =>
            impl ::core::hash::Hash for #target {
                #[inline(always)]
                fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                    if let Ok(origin) = <#origin as ::core::convert::TryFrom<#target>>::try_from(*self) {
                        <#origin as ::core::hash::Hash>::hash(&origin, state)
                    } else {
                        ::core::hash::Hash::hash(&<#target as ::ffi_enum::FfiEnum>::UNKNOWN.repr, state)
                    }
                }
            }
        })
    }

    #[allow(clippy::wrong_self_convention)]
    fn from_str(self) {
        let Self {
            span,
            target,
            origin,
            impls,
            ..
        } = self;

        impls.extend(quote_spanned! { span =>
            impl ::core::str::FromStr for #target {
                type Err = ::core::convert::Infallible;

                #[inline(always)]
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    Ok(
                        <#origin as ::core::str::FromStr>::from_str(s)
                            .map_or_else(|_| <Self as ::ffi_enum::FfiEnum>::UNKNOWN, |v| v.into()),
                    )
                }
            }
        })
    }

    fn serialize(self) {
        let Self {
            name,
            target,
            origin,
            span,
            attrs,
            impls,
            ..
        } = self;

        let serde = locate_serde(attrs);

        impls.extend(quote_spanned! { span =>
            impl #serde::Serialize for #target {
                #[inline(always)]
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: #serde::Serializer,
                {
                    if let Ok(origin) = <#origin as ::core::convert::TryFrom<#target>>::try_from(*self) {
                        <#origin as #serde::Serialize>::serialize(&origin, serializer)
                    } else {
                        <S as #serde::Serializer>::serialize_unit_variant(serializer, #name, <#target as ::ffi_enum::FfiEnum>::UNKNOWN.repr as _, "<Unknown>")
                    }
                }
            }
        });
    }

    fn deserialize(self) {
        let Self {
            target,
            origin,
            span,
            attrs,
            impls,
            ..
        } = self;
        let serde = locate_serde(attrs);

        impls.extend(quote_spanned! { span =>
            impl<'de> #serde::Deserialize<'de> for #target {
                #[inline(always)]
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: #serde::Deserializer<'de>,
                {
                    Ok(<#origin as #serde::Deserialize>::deserialize(deserializer).map(Into::into).unwrap_or_else(|_|<#target as ::ffi_enum::FfiEnum>::UNKNOWN))
                }
            }
        });
    }
}

fn locate_serde(attrs: &[Attribute]) -> Path {
    extract_meta_from_lists(attrs, "serde")
        .find_map(|meta| {
            let MetaNameValue { path, value, .. } = meta.require_name_value().ok()?;
            if !path.is_ident("crate") {
                return None;
            }
            let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = value
            else {
                return None;
            };
            s.parse::<Path>().ok()
        })
        .unwrap_or_else(|| Path {
            leading_colon: Some(Token![::](Span::mixed_site())),
            segments: [PathSegment {
                ident: Ident::new("serde", Span::mixed_site()),
                arguments: PathArguments::None,
            }]
            .into_iter()
            .collect(),
        })
}

pub fn delegate<'a, 'b>(
    name: &'a str,
    target: &'a Type,
    origin: &'a Type,
    attrs: &'a [Attribute],
    impls: &'b mut TokenStream,
) -> impl FnMut(Meta) + use<'a, 'b> {
    |meta| {
        let Meta::Path(path) = meta else {
            return;
        };

        let Some(ident) = path.get_ident() else {
            return;
        };

        let context = Context {
            name,
            span: ident.span().resolved_at(Span::mixed_site()),
            target,
            origin,
            attrs,
            impls,
        };

        match ident.to_string().as_str() {
            "Debug" => context.debug(),
            "Display" => context.display(),
            "Error" => context.error(),
            "Hash" => context.hash(),
            "FromStr" => context.from_str(),
            "Serialize" => context.serialize(),
            "Deserialize" => context.deserialize(),
            _ => (),
        };
    }
}
