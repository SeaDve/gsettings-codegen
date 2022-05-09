mod enumeration;
mod string;

use heck::ToSnakeCase;
use proc_macro2::Span;
use proc_macro_error::abort_call_site;
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use syn::Ident;

use crate::schema::{Enum as SchemaEnum, Key as SchemaKey, KeySignature as SchemaKeySignature};

pub enum Override {
    Define { arg_type: String, ret_type: String },
    Skip,
}

pub enum GetResult<'a> {
    Some(KeyGenerator<'a>),
    Skip,
    Unknown,
}

pub struct KeyGenerators {
    signatures: HashMap<SchemaKeySignature, Context>,
    key_names: HashMap<String, Context>,
    enums: HashMap<String, SchemaEnum>,
    signature_skips: HashSet<SchemaKeySignature>,
    key_name_skips: HashSet<String>,
}

impl KeyGenerators {
    pub fn with_defaults(enums: HashMap<String, SchemaEnum>) -> Self {
        let mut this = Self {
            signatures: HashMap::new(),
            key_names: HashMap::new(),
            enums,
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
    pub fn add_signature_overrides(&mut self, overrides: HashMap<SchemaKeySignature, Override>) {
        for (signature, item) in overrides {
            match item {
                Override::Define { arg_type, ret_type } => {
                    self.signatures
                        .insert(signature, Context::new_dissimilar(&arg_type, &ret_type));
                }
                Override::Skip => {
                    self.signature_skips.insert(signature);
                }
            }
        }
    }

    /// Add contexts that has higher priority than both default and signature overrides.
    pub fn add_key_name_overrides(&mut self, overrides: HashMap<String, Override>) {
        for (key_name, item) in overrides {
            match item {
                Override::Define { arg_type, ret_type } => {
                    self.key_names
                        .insert(key_name, Context::new_dissimilar(&arg_type, &ret_type));
                }
                Override::Skip => {
                    self.key_name_skips.insert(key_name);
                }
            }
        }
    }

    pub fn get<'a>(&'a self, key: &'a SchemaKey) -> GetResult<'a> {
        let key_signature = key.signature();

        if self.key_name_skips.contains(&key.name) {
            return GetResult::Skip;
        }

        if self.signature_skips.contains(&key_signature) {
            return GetResult::Skip;
        }

        if let Some(context) = self.key_names.get(&key.name) {
            return GetResult::Some(KeyGenerator::new(key, context.clone()));
        }

        if let Some(context) = self.signatures.get(&key_signature) {
            return GetResult::Some(KeyGenerator::new(key, context.clone()));
        }

        match key_signature {
            SchemaKeySignature::Type(type_) => match type_.as_str() {
                "s" => GetResult::Some(string::key_generator(key)),
                _ => GetResult::Unknown,
            },
            SchemaKeySignature::Enum(ref enum_name) => GetResult::Some(enumeration::key_generator(
                key,
                self.enums.get(enum_name).unwrap_or_else(|| {
                    abort_call_site!("expected an enum definition for `{}`", enum_name)
                }),
            )),
        }
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

    fn docs(&self) -> String {
        let mut buf = String::new();
        if let Some(ref summary) = self.key.summary {
            if !summary.is_empty() {
                buf.push_str(summary);
                buf.push('\n');
            }
        }

        buf.push('\n');
        buf.push_str(&format!("default: {}", self.key.default));

        // only needed for numerical types
        if let Some(ref range) = self.key.range {
            let min_is_some = range.min.as_ref().map_or(false, |min| !min.is_empty());
            let max_is_some = range.max.as_ref().map_or(false, |max| !max.is_empty());

            if min_is_some || max_is_some {
                buf.push('\n');
                buf.push('\n');
            }
            if min_is_some {
                buf.push_str(&format!("min: {}", range.min.as_ref().unwrap()));
            }
            if min_is_some && max_is_some {
                buf.push(';');
                buf.push(' ');
            }
            if max_is_some {
                buf.push_str(&format!("max: {}", range.max.as_ref().unwrap()));
            }
        }

        buf
    }
}

impl quote::ToTokens for KeyGenerator<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let docs = self.docs();
        let key_name = self.key.name.as_str();
        let key_name_snake_case = key_name.to_snake_case();
        let getter_func_ident = Ident::new(&key_name_snake_case, Span::call_site());

        let connect_changed_func_ident = format_ident!("connect_{}_changed", getter_func_ident);
        let bind_func_ident = format_ident!("bind_{}", getter_func_ident);
        let create_action_func_ident = format_ident!("create_{}_action", getter_func_ident);

        tokens.extend(quote! {
            #[doc = #docs]
            pub fn #connect_changed_func_ident(&self, f: impl Fn(&gio::Settings) + 'static) -> gio::glib::SignalHandlerId {
                gio::prelude::SettingsExt::connect_changed(&self.0, Some(#key_name), move |settings, _| {
                    f(settings)
                })
            }

            #[doc = #docs]
            pub fn #bind_func_ident<'a>(&'a self, object: &'a impl gio::glib::object::IsA<gio::glib::Object>, property: &'a str) -> gio::BindingBuilder<'a> {
                gio::prelude::SettingsExtManual::bind(&self.0, #key_name, object, property)
            }

            #[doc = #docs]
            pub fn #create_action_func_ident(&self) -> gio::Action {
                gio::prelude::SettingsExt::create_action(&self.0, #key_name)
            }
        });

        let setter_func_ident = format_ident!("set_{}", getter_func_ident);
        let try_setter_func_ident = format_ident!("try_set_{}", getter_func_ident);

        let get_type = syn::parse_str::<syn::Type>(&self.context.ret_type)
            .unwrap_or_else(|_| panic!("Invalid type `{}`", &self.context.ret_type));
        let set_type = syn::parse_str::<syn::Type>(&self.context.arg_type)
            .unwrap_or_else(|_| panic!("Invalid type `{}`", &self.context.arg_type));

        tokens.extend(quote! {
            #[doc = #docs]
            pub fn #setter_func_ident(&self, value: #set_type) {
                self.#try_setter_func_ident(value).unwrap_or_else(|err| panic!("failed to set value for key `{}`: {:?}", #key_name, err))
            }

            #[doc = #docs]
            pub fn #try_setter_func_ident(&self, value: #set_type) -> std::result::Result<(), gio::glib::BoolError> {
                gio::prelude::SettingsExtManual::set(&self.0, #key_name, &value)
            }

            #[doc = #docs]
            pub fn #getter_func_ident(&self) -> #get_type {
                gio::prelude::SettingsExtManual::get(&self.0, #key_name)
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
