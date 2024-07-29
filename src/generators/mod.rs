mod bitflag;
mod enumeration;
mod string;

use heck::{ToPascalCase, ToShoutySnakeCase, ToSnakeCase};

use proc_macro2::Span;
use proc_macro_error::abort_call_site;
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use syn::{spanned::Spanned, Ident};

use std::fmt::Write;

use crate::schema::{
    Enum as SchemaEnum, Flag as SchemaFlag, Key as SchemaKey, KeySignature as SchemaKeySignature,
};

pub enum OverrideType {
    Define { arg_type: String, ret_type: String },
    Skip,
}

pub enum GetResult<'a> {
    Some(KeyGenerator<'a>),
    Skip,
    Unknown,
}

pub struct KeyGenerators<'a> {
    signatures: HashMap<SchemaKeySignature, Context>,
    key_names: HashMap<String, Context>,
    enums: HashMap<String, &'a SchemaEnum>,
    flags: HashMap<String, &'a SchemaFlag>,
    signature_skips: HashSet<SchemaKeySignature>,
    key_name_skips: HashSet<String>,
}

impl<'a> KeyGenerators<'a> {
    pub fn with_defaults(
        enums: HashMap<String, &'a SchemaEnum>,
        flags: HashMap<String, &'a SchemaFlag>,
    ) -> Self {
        let mut this = Self {
            signatures: HashMap::new(),
            key_names: HashMap::new(),
            enums,
            flags,
            signature_skips: HashSet::new(),
            key_name_skips: HashSet::new(),
        };

        // Built ins
        this.insert_type("b", Context::new("bool"));
        this.insert_type("i", Context::new("i32"));
        this.insert_type("u", Context::new("u32"));
        this.insert_type("x", Context::new("i64"));
        this.insert_type("t", Context::new("u64"));
        this.insert_type("d", Context::new("f64"));
        this.insert_type("(ii)", Context::new("(i32, i32)"));
        this.insert_type("as", Context::new_dissimilar("&[&str]", "Vec<String>"));

        this
    }

    /// Add contexts that has higher priority than default, but lower than
    /// key_name overrides
    pub fn add_signature_overrides(
        &mut self,
        overrides: HashMap<SchemaKeySignature, OverrideType>,
    ) {
        for (signature, item) in overrides {
            match item {
                OverrideType::Define { arg_type, ret_type } => {
                    self.signatures
                        .insert(signature, Context::new_dissimilar(&arg_type, &ret_type));
                }
                OverrideType::Skip => {
                    self.signature_skips.insert(signature);
                }
            }
        }
    }

    /// Add contexts that has higher priority than both default and signature overrides.
    pub fn add_key_name_overrides(&mut self, overrides: HashMap<String, OverrideType>) {
        for (key_name, item) in overrides {
            match item {
                OverrideType::Define { arg_type, ret_type } => {
                    self.key_names
                        .insert(key_name, Context::new_dissimilar(&arg_type, &ret_type));
                }
                OverrideType::Skip => {
                    self.key_name_skips.insert(key_name);
                }
            }
        }
    }

    pub fn get(
        &'a self,
        key: &'a SchemaKey,
        aux_visibility: syn::Visibility,
    ) -> Option<GetResult<'a>> {
        let key_signature = key.signature()?;

        if self.key_name_skips.contains(&key.name) {
            return Some(GetResult::Skip);
        }

        if self.signature_skips.contains(&key_signature) {
            return Some(GetResult::Skip);
        }

        if let Some(context) = self.key_names.get(&key.name) {
            return Some(GetResult::Some(KeyGenerator::new(key, context.clone())));
        }

        if let Some(context) = self.signatures.get(&key_signature) {
            return Some(GetResult::Some(KeyGenerator::new(key, context.clone())));
        }

        Some(match key_signature {
            SchemaKeySignature::Type(type_) => match type_.as_str() {
                "s" => GetResult::Some(string::key_generator(key, aux_visibility)),
                _ => GetResult::Unknown,
            },
            SchemaKeySignature::Enum(ref enum_name) => GetResult::Some(enumeration::key_generator(
                key,
                self.enums.get(enum_name).unwrap_or_else(|| {
                    abort_call_site!("expected an enum definition for `{}`", enum_name)
                }),
                aux_visibility,
            )),
            SchemaKeySignature::Flag(ref flag_name) => GetResult::Some(bitflag::key_generator(
                key,
                self.flags.get(flag_name).unwrap_or_else(|| {
                    abort_call_site!("expected a flag definition for `{}`", flag_name)
                }),
                aux_visibility,
            )),
        })
    }

    fn insert_type(&mut self, signature: &str, context: Context) {
        self.signatures
            .insert(SchemaKeySignature::Type(signature.to_string()), context);
    }
}

pub struct KeyGenerator<'a> {
    key: &'a SchemaKey,
    context: Context,
}

impl<'a> KeyGenerator<'a> {
    pub fn auxiliary(&self) -> Option<proc_macro2::TokenStream> {
        self.context.auxiliary.clone()
    }

    fn new(key: &'a SchemaKey, context: Context) -> Self {
        Self { key, context }
    }

    fn func_docs(&self) -> proc_macro2::TokenStream {
        let mut stream = proc_macro2::TokenStream::new();

        let has_summary = self
            .key
            .summary
            .as_ref()
            .map_or(false, |summary| !summary.is_empty());

        let has_description = self
            .key
            .description
            .as_ref()
            .map_or(false, |description| !description.is_empty());

        if has_summary {
            let summary = self.key.summary.as_ref().unwrap();
            stream.extend(quote! {
                #[doc = #summary]
            });
        }

        if has_summary && has_description {
            stream.extend(quote! {
                #[doc = ""]
            });
        }

        if has_description {
            let description = self.key.description.as_ref().unwrap();
            stream.extend(quote! {
                #[doc = #description]
            });
        }

        let default_docs = format!("default: {}", self.key.default);
        stream.extend(quote! {
            #[doc = ""]
            #[doc = #default_docs]
        });

        // only needed for numerical types
        if let Some(ref range) = self.key.range {
            let has_min = range.min.as_ref().map_or(false, |min| !min.is_empty());
            let has_max = range.max.as_ref().map_or(false, |max| !max.is_empty());

            if has_min || has_max {
                stream.extend(quote! {
                    #[doc = ""]
                });
            }
            let mut range_docs = String::new();
            if has_min {
                write!(range_docs, "min: {}", range.min.as_ref().unwrap()).unwrap();
            }
            if has_min && has_max {
                range_docs.push(';');
                range_docs.push(' ');
            }
            if has_max {
                write!(range_docs, "max: {}", range.max.as_ref().unwrap()).unwrap();
            }
            stream.extend(quote! {
                #[doc = #range_docs]
            })
        }

        stream
    }
}

impl quote::ToTokens for KeyGenerator<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let key_name = self.key.name.as_str();
        let key_name_snake_case = key_name.to_snake_case();
        let getter_func_ident = Ident::new(&key_name_snake_case, Span::call_site());

        let connect_changed_func_ident = format_ident!("connect_{}_changed", getter_func_ident);
        let bind_func_ident = format_ident!("bind_{}", getter_func_ident);
        let create_action_func_ident = format_ident!("create_{}_action", getter_func_ident);
        let reset_func_ident = format_ident!("reset_{}", getter_func_ident);

        let func_docs = self.func_docs();

        tokens.extend(quote! {
            #func_docs
            pub fn #connect_changed_func_ident(&self, f: impl Fn(&Self) + 'static) -> gio::glib::SignalHandlerId {
                gio::prelude::SettingsExt::connect_changed(&self.0, Some(#key_name), move |settings, _| {
                    f(&Self(gio::Settings::clone(settings)))
                })
            }

            #func_docs
            pub fn #bind_func_ident<'a>(&'a self, object: &'a impl gio::glib::object::IsA<gio::glib::Object>, property: &'a str) -> gio::BindingBuilder<'a> {
                gio::prelude::SettingsExtManual::bind(&self.0, #key_name, object, property)
            }

            #func_docs
            pub fn #create_action_func_ident(&self) -> gio::Action {
                gio::prelude::SettingsExt::create_action(&self.0, #key_name)
            }

            #func_docs
            pub fn #reset_func_ident(&self) {
                gio::prelude::SettingsExt::reset(&self.0, #key_name);
            }
        });

        let setter_func_ident = format_ident!("set_{}", getter_func_ident);
        let try_setter_func_ident = format_ident!("try_set_{}", getter_func_ident);
        let default_value_func_ident = format_ident!("{}_default_value", getter_func_ident);

        let get_type = syn::parse_str::<syn::Type>(&self.context.ret_type)
            .unwrap_or_else(|_| panic!("Invalid type `{}`", &self.context.ret_type));
        let set_type = syn::parse_str::<syn::Type>(&self.context.arg_type)
            .unwrap_or_else(|_| panic!("Invalid type `{}`", &self.context.arg_type));

        tokens.extend(quote! {
            #func_docs
            pub fn #setter_func_ident(&self, value: #set_type) {
                self.#try_setter_func_ident(value).unwrap_or_else(|err| panic!("failed to set value for key `{}`: {:?}", #key_name, err))
            }

            #func_docs
            pub fn #try_setter_func_ident(&self, value: #set_type) -> std::result::Result<(), gio::glib::BoolError> {
                gio::prelude::SettingsExtManual::set(&self.0, #key_name, &value)
            }

            #func_docs
            pub fn #getter_func_ident(&self) -> #get_type {
                gio::prelude::SettingsExtManual::get(&self.0, #key_name)
            }

            #func_docs
            pub fn #default_value_func_ident(&self) -> #get_type {
                gio::glib::Variant::get(&gio::prelude::SettingsExt::default_value(&self.0, #key_name).unwrap()).unwrap()
            }
        });
    }
}

#[derive(Clone)]
pub struct Context {
    arg_type: String,
    ret_type: String,
    auxiliary: Option<proc_macro2::TokenStream>,
}

impl Context {
    pub fn new(type_: &str) -> Self {
        Self::new_dissimilar(type_, type_)
    }

    pub fn new_dissimilar(arg_type: &str, ret_type: &str) -> Self {
        Self {
            arg_type: arg_type.to_string(),
            ret_type: ret_type.to_string(),
            auxiliary: None,
        }
    }

    pub fn new_with_aux(type_: &str, auxiliary: proc_macro2::TokenStream) -> Self {
        Self {
            arg_type: type_.to_string(),
            ret_type: type_.to_string(),
            auxiliary: Some(auxiliary),
        }
    }
}

/// Creates an enum with given name and (variant name, variant value) tuple. It implements
/// [`FromVariant`](gio::glib::variant::FromVariant), [`ToVariant`](gio::glib::variant::ToVariant),
/// and [`StaticVariantType`](gio::glib::variant::StaticVariantType).
///
/// The input names are converted to pascal case
pub(crate) fn enum_token_stream(
    name: &str,
    variants: &[(&str, Option<i32>)],
    visibility: syn::Visibility,
) -> proc_macro2::TokenStream {
    let variant_names = variants
        .iter()
        .map(|(variant_name, _)| variant_name)
        .collect::<Vec<_>>();

    let variant_idents = variant_names
        .iter()
        .map(|variant_name| Ident::new(&variant_name.to_pascal_case(), variant_name.span()))
        .collect::<Vec<_>>();

    let variant_arms =
        variants
            .iter()
            .zip(variant_idents.iter())
            .map(|((_, variant_value), variant_ident)| {
                if let Some(variant_value) = variant_value {
                    quote! {
                        #variant_ident = #variant_value
                    }
                } else {
                    quote! {
                        #variant_ident
                    }
                }
            });

    let from_variant_arms =
        variant_names
            .iter()
            .zip(variant_idents.iter())
            .map(|(variant_name, variant_ident)| {
                quote! {
                    #variant_name => Some(Self::#variant_ident)
                }
            });

    let to_variant_arms =
        variant_names
            .iter()
            .zip(variant_idents.iter())
            .map(|(variant_name, variant_ident)| {
                quote! {
                    Self::#variant_ident => gio::glib::variant::ToVariant::to_variant(#variant_name)
                }
            });

    let name_pascal_case = name.to_pascal_case();
    let ident = Ident::new(&name_pascal_case, name_pascal_case.span());

    quote! {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(i32)]
        #visibility enum #ident {
            #(#variant_arms),*
        }

        impl gio::glib::variant::StaticVariantType for #ident {
            fn static_variant_type() -> std::borrow::Cow<'static, gio::glib::VariantTy> {
                std::borrow::Cow::Borrowed(gio::glib::VariantTy::STRING)
            }
        }

        impl gio::glib::variant::FromVariant for #ident {
            fn from_variant(variant: &gio::glib::Variant) -> Option<Self> {
                match variant.get::<String>()?.as_str() {
                    #(#from_variant_arms),*,
                    _ => None,
                }
            }
        }

        impl gio::glib::variant::ToVariant for #ident {
            fn to_variant(&self) -> gio::glib::Variant {
                match self {
                    #(#to_variant_arms),*
                }
            }
        }

        impl std::convert::From<#ident> for gio::glib::Variant {
            fn from(this: #ident) -> gio::glib::Variant {
                gio::glib::variant::ToVariant::to_variant(&this)
            }
        }
    }
}

pub(crate) fn type_name_from_id(id: &str) -> String {
    id.split('.')
        .last()
        .map(|part| part.to_pascal_case())
        .unwrap_or(id.to_string())
}

pub(crate) fn bitflag_token_stream(
    name: &str,
    flag: &SchemaFlag,
    visibility: syn::Visibility,
) -> proc_macro2::TokenStream {
    let value_idents = flag
        .values
        .iter()
        .map(|value| Ident::new(&value.nick.to_shouty_snake_case(), value.nick.span()))
        .collect::<Vec<_>>();

    let flags_arms = value_idents
        .iter()
        .zip(flag.values.iter())
        .map(|(value_ident, value)| {
            let value = value.value;
            quote! {
                const #value_ident = #value;
            }
        });

    let from_variant_arms =
        value_idents
            .iter()
            .zip(flag.values.iter())
            .map(|(value_ident, value)| {
                let nick = &value.nick;
                quote! {
                    #nick => this.insert(Self::#value_ident)
                }
            });

    let to_variant_arms =
        value_idents
            .iter()
            .zip(flag.values.iter())
            .map(|(value_ident, value)| {
                let nick = &value.nick;
                quote! {
                    if self.contains(Self::#value_ident) {
                        string_array.push(#nick)
                    }
                }
            });

    let name_pascal_case = name.to_pascal_case();
    let ident = Ident::new(&name_pascal_case, name_pascal_case.span());

    quote! {
        gio::glib::bitflags::bitflags! {
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            #visibility struct #ident: u32 {
                #(#flags_arms)*
            }
        }

        impl gio::glib::variant::StaticVariantType for #ident {
            fn static_variant_type() -> std::borrow::Cow<'static, gio::glib::VariantTy> {
                std::borrow::Cow::Borrowed(gio::glib::VariantTy::STRING_ARRAY)
            }
        }

        impl gio::glib::variant::FromVariant for #ident {
            fn from_variant(variant: &gio::glib::Variant) -> Option<Self> {
                let mut this = Self::empty();

                for string in variant.get::<Vec<String>>()? {
                    match string.as_str() {
                        #(#from_variant_arms),*,
                        _ => return None,
                    }
                }

                Some(this)
            }
        }

        impl gio::glib::variant::ToVariant for #ident {
            fn to_variant(&self) -> gio::glib::Variant {
                let mut string_array = Vec::new();

                #(#to_variant_arms)*

                gio::glib::variant::ToVariant::to_variant(&string_array)
            }
        }

        impl std::convert::From<#ident> for gio::glib::Variant {
            fn from(this: #ident) -> gio::glib::Variant {
                gio::glib::variant::ToVariant::to_variant(&this)
            }
        }
    }
}
