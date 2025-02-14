use std::borrow::Borrow;

use syn::{
    parse::Parser as _, punctuated::Punctuated, Attribute, Ident, MacroDelimiter, Meta, MetaList,
    MetaNameValue, Token,
};

pub fn extract_meta_from_lists<'a, I>(
    attrs: &'a [Attribute],
    ident: &'a I,
) -> impl Iterator<Item = Meta> + use<'a, I>
where
    I: ?Sized,
    Ident: PartialEq<I>,
{
    attrs
        .iter()
        .filter_map(|attr| {
            let Meta::List(MetaList {
                path,
                delimiter: MacroDelimiter::Paren(..),
                tokens,
            }) = &attr.meta
            else {
                return None;
            };

            if !path.is_ident(ident) {
                return None;
            }

            Punctuated::<Meta, Token![,]>::parse_terminated
                .parse2(tokens.clone())
                .ok()
        })
        .flatten()
}

pub fn is_doc(attr: &impl Borrow<Attribute>) -> bool {
    let Ok(MetaNameValue { path, .. }) = attr.borrow().meta.require_name_value() else {
        return false;
    };
    path.is_ident("doc")
}
