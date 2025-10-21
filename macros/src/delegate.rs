use proc_macro2::{Span, TokenStream};
use quote::{quote_spanned, ToTokens};
use syn::{
    parse::Parser as _, punctuated::Punctuated, token::Comma, Attribute, Expr, ExprLit, Ident, Lit,
    LitByteStr, LitStr, Meta, MetaNameValue, Path, PathArguments, PathSegment, Token, Type,
    Variant,
};

use crate::utils::extract_meta_from_lists;

struct Context<'a, 'b> {
    name: &'a str,
    span: Span,
    target: &'a Type,
    origin: &'a Type,
    attrs: &'a [Attribute],
    variants: &'a Punctuated<Variant, Comma>,
    repr: &'a Type,
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
            impls,
            ..
        } = self;

        impls.extend(quote_spanned! { span =>
            impl ::core::hash::Hash for #target {
                #[inline(always)]
                fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                    self.repr.hash(state)
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
            repr,
            ..
        } = self;

        impls.extend(quote_spanned! { span =>
            impl ::core::str::FromStr for #target {
                type Err = <#repr as ::core::str::FromStr>::Err;

                #[inline(always)]
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    Ok(
                        match <#origin as ::core::str::FromStr>::from_str(s) {
                            Ok(value) => Ok(value.into())
                            Err(err) => #repr::from_str(s),
                        }
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
                    if let Ok(val) = <#origin as ::core::convert::TryFrom<#target>>::try_from(*self) {
                        <#origin as #serde::Serialize>::serialize(&val, serializer)
                    } else {
                        serializer.serialize_unit_variant(#name, self.repr as _, "<Unknown>")
                    }
                }
            }
        });
    }

    fn deserialize(self) {
        let Self {
            target,
            span,
            attrs,
            impls,
            repr,
            variants,
            ..
        } = self;
        let serde = locate_serde(attrs);
        let renameall = rename_with(
            attrs
                .iter()
                .find_map(list_attr_filter_map("serde", |path, val| {
                    if !path.is_ident("rename_all") {
                        return None;
                    }
                    Some(val.value())
                }))
                .unwrap_or("PascalCase".to_string()),
        );
        let variants = variants
            .iter()
            .map(|v| {
                (v.ident.clone(), {
                    let mut name = renameall(v.ident.to_string().as_ref());
                    let mut names = v
                        .attrs
                        .iter()
                        .filter_map(list_attr_filter_map("serde", |path, val| {
                            if path.is_ident("rename") {
                                name = rename_with(val.value())(v.ident.to_string().as_ref());
                                None
                            } else if path.is_ident("alias") {
                                Some(val.value())
                            } else {
                                None
                            }
                        }))
                        .collect::<Vec<_>>();
                    names.push(name);
                    names
                })
            })
            .collect::<Vec<_>>();

        let visit_repr = Ident::new(
            &format!("visit_{}", repr.to_token_stream()),
            Span::mixed_site(),
        );

        let names = variants.iter().map(|v| &v.0).collect::<Vec<_>>();
        let aliases = variants.iter().map(|v| &v.1).map(|v| {
            v.iter()
                .map(|v| Lit::Str(LitStr::new(v, Span::call_site())))
                .collect::<Punctuated<Lit, Token![|]>>()
        });

        let baliases = variants.iter().map(|v| &v.1).map(|v| {
            v.iter()
                .map(|v| Lit::ByteStr(LitByteStr::new(v.as_bytes(), Span::call_site())))
                .collect::<Punctuated<Lit, Token![|]>>()
        });

        impls.extend(quote_spanned! { span =>
            impl<'de> #serde::Deserialize<'de> for #target {
                #[inline(always)]
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: #serde::Deserializer<'de>,
                {
                    struct Visitor;
                    impl<'de> #serde::de::Visitor<'de> for Visitor {
                        type Value = #target;

                        fn expecting(
                            &self,
                            formatter: &mut ::core::fmt::Formatter
                        ) -> ::core::fmt::Result {
                            formatter.write_str("variant identifier")
                        }

                        fn #visit_repr<E:#serde::de::Error>(self,v:#repr)->::core::result::Result<Self::Value,E>{
                            Ok(match v{
                                #(v if v==#target::#names.repr => #target::#names,)*
                                v => #target{repr:v}
                            })
                        }

                        fn visit_str<E:#serde::de::Error>(self,v:&str)->::core::result::Result<Self::Value,E>{
                            match v{
                                #(#aliases => Ok(#target::#names),)*
                                v => Err(#serde::de::Error::unknown_variant(v,VARIANTS)),
                            }
                        }

                        fn visit_bytes<E:#serde::de::Error>(self,v:&[u8])->::core::result::Result<Self::Value,E>{
                            match v{
                                #(#baliases => Ok(#target::#names),)*
                                v => Err(#serde::de::Error::unknown_variant(&::std::string::String::from_utf8_lossy(v),VARIANTS)),
                            }
                        }
                    }
                    const VARIANTS:&'static[&'static str]=&[#(stringify!(#names),)*];
                    deserializer.deserialize_identifier(
                        // #name,
                        // VARIANTS,
                        Visitor,
                    )
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

fn list_attr_filter_map<'a, T>(
    name: &'a str,
    mut f: impl FnMut(Path, syn::LitStr) -> Option<T> + 'a,
) -> impl FnMut(&'a Attribute) -> Option<T> + 'a {
    move |attr| {
        let Attribute {
            style: syn::AttrStyle::Outer,
            meta: Meta::List(syn::MetaList { path, tokens, .. }),
            ..
        } = attr
        else {
            return None;
        };
        if !path.is_ident(name) {
            return None;
        };
        Punctuated::<Meta, Token![,]>::parse_terminated
            .parse2(tokens.clone())
            .ok()?
            .into_iter()
            .find_map(|meta| {
                let Meta::NameValue(syn::MetaNameValue {
                    path,
                    value:
                        syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(val),
                            ..
                        }),
                    ..
                }) = meta
                else {
                    return None;
                };
                f(path, val)
            })
    }
}

fn rename_with(fmt: String) -> impl Fn(&str) -> String {
    move |variant| match fmt.as_str() {
        "PascalCase" => variant.to_owned(),
        "lowercase" => variant.to_ascii_lowercase(),
        "UPPERCASE" => variant.to_ascii_uppercase(),
        "camelCase" => variant[..1].to_ascii_lowercase() + &variant[1..],
        "snake_case" => {
            let mut snake = String::new();
            for (i, ch) in variant.char_indices() {
                if i > 0 && ch.is_uppercase() {
                    snake.push('_');
                }
                snake.push(ch.to_ascii_lowercase());
            }
            snake
        }
        "SCREAMING_SNAKE_CASE" => {
            rename_with("snake_case".to_string())(variant).to_ascii_uppercase()
        }
        "kebab-case" => rename_with("snake_case".to_string())(variant).replace('_', "-"),
        "SCREAMING-KEBAB-CASE" => {
            rename_with("SCREAMING_SNAKE_CASE".to_string())(variant).replace('_', "-")
        }
        _ => panic!("error fmt with {}", fmt),
    }
}

pub fn delegate<'a, 'b>(
    name: &'a str,
    target: &'a Type,
    origin: &'a Type,
    attrs: &'a [Attribute],
    repr: &'a Type,
    variants: &'a Punctuated<Variant, Comma>,
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
            repr,
            variants,
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
