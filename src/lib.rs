#![warn(rust_2018_idioms)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]

mod generators;
mod schema;

use deluxe::SpannedValue;
use generators::{bitflag_token_stream, enum_token_stream, type_name_from_id};
use proc_macro_error::{abort, emit_call_site_error, emit_error, proc_macro_error};
use quote::{quote, ToTokens};

use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Token,
};

use std::{collections::HashMap, fs::File, io::BufReader};

use crate::{
    generators::{GetResult, KeyGenerators, OverrideType},
    schema::{KeySignature as SchemaKeySignature, SchemaList},
};

// TODO:
// * Replace proc-macro-error dep with syn::Result
// * Use `quote_spanned` where applicable for better error propagation on generated code
// * Remove serde and deluxe dependencies (consider using quick-xml directly or xmlserde)
// * Improve enum generation (create enum based on its definition, instead of by key; also add doc alias for its id)
// * Add way to map setter and getters value
// * Add `bind_#key writable`, `user_#key_value`, `connect_#key_writable_changed` variants
// * Add trybuild tests

#[derive(deluxe::ParseMetaItem)]
struct GenSettings {
    file: SpannedValue<String>,
    id: Option<SpannedValue<String>>,
    #[deluxe(default = true)]
    default: bool,
    #[deluxe(default = true)]
    globals: bool,
}

#[derive(deluxe::ParseAttributes)]
struct GenSettingsDefine {
    signature: Option<SpannedValue<String>>,
    key_name: Option<SpannedValue<String>>,
    arg_type: SpannedValue<String>,
    ret_type: SpannedValue<String>,
}

#[derive(deluxe::ParseAttributes)]
struct GenSettingsSkip {
    signature: Option<SpannedValue<String>>,
    key_name: Option<SpannedValue<String>>,
}

struct SettingsStruct {
    attrs: Vec<syn::Attribute>,
    vis: syn::Visibility,
    struct_token: Token![struct],
    ident: syn::Ident,
    semi_token: Token![;],
}

impl Parse for SettingsStruct {
    fn parse(input: ParseStream<'_>) -> syn::parse::Result<Self> {
        Ok(Self {
            attrs: input.call(syn::Attribute::parse_outer)?,
            vis: input.parse()?,
            struct_token: input.parse()?,
            ident: input.parse()?,
            semi_token: input.parse()?,
        })
    }
}

impl ToTokens for SettingsStruct {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.vis.to_tokens(tokens);
        self.struct_token.to_tokens(tokens);
        self.ident.to_tokens(tokens);

        let field: syn::FieldsUnnamed = syn::parse_quote!((gio::Settings));
        field.to_tokens(tokens);

        self.semi_token.to_tokens(tokens);
    }
}

/// Macro for typesafe [`gio::Settings`] key access.
///
/// The macro's main purpose is to reduce the risk of mistyping a key,
/// using the wrong method to access values, inputting incorrect values,
/// and to reduce boilerplate. Additionally, the summary, description,
/// and default value are included in the documentation of each generated
/// method. This would be beneficial if you use tools like
/// [`rust-analyzer`](https://rust-analyzer.github.io/).
///
/// **⚠️ IMPORTANT ⚠️**
///
/// Both `gio` and `glib` need to be in scope, so unless they are direct crate
/// dependencies, you need to import them because `gen_settings` is using
/// them internally. For example:
///
/// ```ignore
/// use gtk::{gio, glib};
/// ```
///
/// ### Example
///
/// ```ignore
/// use gsettings_macro::gen_settings;
///
/// #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
/// pub struct ApplicationSettings;
///
/// let settings = ApplicationSettings::new("io.github.seadve.test");
///
/// // `i` D-Bus type
/// settings.set_window_width(100);
/// assert_eq!(settings.window_width(), 100);
///
/// // enums
/// settings.set_alert_sound(AlertSound::Glass);
/// assert_eq!(settings.alert_sound(), AlertSound::Glass);
///
/// // bitflags
/// settings.set_space_style(SpaceStyle::BEFORE_COLON | SpaceStyle::BEFORE_COMMA);
/// assert_eq!(
///     settings.space_style(),
///     SpaceStyle::BEFORE_COLON | SpaceStyle::BEFORE_COMMA
/// );
/// ```
///
/// Note: The file path is relative to the project root or where the
/// `Cargo.toml` file is located.
///
/// ### Generated methods
///
/// The procedural macro generates the following [`gio::Settings`] methods
/// for each key in the schema:
///
/// * `set` -> `set_${key}`, which panics when writing in a readonly
/// key, and `try_set_${key}`, which behaves the same as the original method.
/// * `get` -> `${key}`
/// * `connect_changed` -> `connect_${key}_changed`
/// * `bind` -> `bind_${key}`
/// * `create_action` -> `create_${key}_action`
/// * `default_value` -> `${key}_default_value`
/// * `reset` -> `reset_${key}`
///
/// ### Known D-Bus type signatures
///
/// The setter and getter methods has the following parameter and
/// return type, depending on the key's type signature.
///
/// | Type Signature | Parameter Type | Return Type   |
/// | -------------- | -------------- | ------------- |
/// | b              | `bool`         | `bool`        |
/// | i              | `i32`          | `i32`         |
/// | u              | `u32`          | `u32`         |
/// | x              | `i64`          | `i64`         |
/// | t              | `u64`          | `u64`         |
/// | d              | `f64`          | `f64`         |
/// | (ii)           | `(i32, i32`)   | `(i32, i32`)  |
/// | as             | `&[&str]`      | `Vec<String>` |
/// | s *            | `&str`         | `String`      |
///
/// \* If the key of type signature `s` has no `choice` attribute
/// specified in the GSchema, the parameter and return types stated
/// in the table would be applied. Otherwise, it will generate an
/// enum, like described in the next section, and use it as the parameter
/// and return types, instead of `&str` and `String` respectively.
///
/// It will not compile if the type signature is not defined above.
/// However, it is possible to explicitly skip generating methods
/// for a specific key or type signature using the attribute
/// `#[gen_settings_skip]`, or define a custom parameter and return
/// types using `#[gen_settings_define]` attribute. The usage of
/// the latter will be further explained in the following sections.
///
/// ### Enums and Flags
///
/// The macro will also automatically generate enums or flags. If it is
/// an enum, it would generated a normal Rust enum with each nick
/// specified in the GSchema converted to pascal case as an enum variant.
/// The enum would implement both [`ToVariant`] and [`FromVariant`], [`Clone`],
/// [`Hash`], [`PartialEq`], [`Eq`], [`PartialOrd`], and [`Ord`]. On
/// the other hand, if it is a flag, it would generate bitflags
/// same as the bitflags generated by the [`bitflags`] macro with each
/// nick specified in the GSchema converted to screaming snake case as
/// a const flag.
///
/// The generated types, enum or bitflags, would have the same
/// visibility and scope with the generated struct.
///
/// ### Skipping methods generation
///
/// This would be helpful if you want to have full control
/// with the key without the macro intervening. For example:
///
/// ```ignore
/// use gsettings_macro::gen_settings;
///
/// #[gen_settings(
///     file = "./tests/io.github.seadve.test.gschema.xml",
///     id = "io.github.seadve.test"
/// )]
/// // Skip generating methods for keys with type signature `(ss)`
/// #[gen_settings_skip(signature = "(ss)")]
/// // Skip generating methods for the key of name `some-key-name`
/// #[gen_settings_skip(key_name = "some-key-name")]
/// pub struct Settings;
///
/// impl Settings {
///     pub fn set_some_key_name(value: &std::path::Path) {
///         ...
///     }
/// }
/// ```
///
/// ### Defining custom types
///
/// ```ignore
/// use gsettings_macro::gen_settings;
///
/// use std::path::{Path, PathBuf};
///
/// #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
/// // Define custom parameter and return types for keys with type `(ss)`
/// #[gen_settings_define(
///     signature = "(ss)",
///     arg_type = "(&str, &str)",
///     ret_type = "(String, String)"
/// )]
/// // Define custom parameter and return types for key with name `cache-dir`
/// #[gen_settings_define(key_name = "cache-dir", arg_type = "&Path", ret_type = "PathBuf")]
/// pub struct SomeAppSettings;
///
/// let settings = SomeAppSettings::new("io.github.seadve.test");
///
/// settings.set_cache_dir(Path::new("/some_dir"));
/// assert_eq!(settings.cache_dir(), PathBuf::from("/some_dir"));
///
/// settings.set_string_tuple(("hi", "hi2"));
/// assert_eq!(settings.string_tuple(), ("hi".into(), "hi2".into()));
/// ```
///
/// The type specified in `arg_type` and `ret_type` has to be on scope or
/// you can specify the full path.
///
/// If you somehow do not want an enum parameter and return types for `s`
/// type signature with choices. You can also use this to override that behavior.
///
/// Note: The type has to implement both [`ToVariant`] and [`FromVariant`] or it
/// would fail to compile.
///
/// ### Default trait
///
/// The schema id can be specified as an attribute, making it implement
/// [`Default`] and create a `new` constructor without parameters.
/// Otherwise, it will not implement [`Default`] and would require the
/// schema id as an parameter in the the constructor or the `new` method.
///
/// The following is an example of defining the `id` attribute in the macro:
///
/// ```ignore
/// use gsettings_macro::gen_settings;
///
/// #[gen_settings(
///     file = "./tests/io.github.seadve.test.gschema.xml",
///     id = "io.github.seadve.test"
/// )]
/// pub struct ApplicationSettings;
///
/// // The id is specified above so it is not needed
/// // to specify it in the constructor.
/// let settings = ApplicationSettings::new();
/// let another_instance = ApplicationSettings::default();
/// ```
///
/// [`gio::Settings`]: https://docs.rs/gio/latest/gio/struct.Settings.html
/// [`ToVariant`]: https://docs.rs/glib/latest/glib/variant/trait.ToVariant.html
/// [`FromVariant`]: https://docs.rs/glib/latest/glib/variant/trait.FromVariant.html
/// [`bitflags`]: https://docs.rs/bitflags/latest/bitflags/macro.bitflags.html
#[proc_macro_attribute]
#[proc_macro_error]
pub fn gen_settings(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let GenSettings {
        file: file_attr,
        id: id_attr,
        default: impl_default,
        globals,
    } = match deluxe::parse2(attr.into()) {
        Ok(gen_settings) => gen_settings,
        Err(err) => return err.to_compile_error().into(),
    };
    let file_attr_span = file_attr.span();
    let schema_file_path = SpannedValue::into_inner(file_attr);

    // Parse schema list
    let schema_file = File::open(schema_file_path).unwrap_or_else(|err| {
        abort!(file_attr_span, "failed to open schema file: {}", err);
    });
    let schema_list: SchemaList = quick_xml::de::from_reader(BufReader::new(schema_file))
        .unwrap_or_else(|err| abort!(file_attr_span, "failed to parse schema file: {}", err));

    let schemas = schema_list.schemas;

    // Get the schema
    let schema = match &schemas[..] {
        [] => {
            abort!(file_attr_span, "schema file must have a single schema");
        }
        [schema] => schema,
        _schemas if id_attr.is_none() => {
            abort!(
                file_attr_span,
                "schema file contains multiple schemas, specify one with `id`"
            );
        }
        schemas => {
            let id_attr = id_attr.as_ref().unwrap_or_else(|| {
                abort!(
                    file_attr_span,
                    "schema file contains multiple schemas, specify one with `id`"
                )
            });
            let id_attr = SpannedValue::into_inner(id_attr.clone());
            schemas
                .iter()
                .find(|schema| schema.id == id_attr)
                .unwrap_or_else(|| abort!(file_attr_span, "schema with id `{}` not found", id_attr))
        }
    };

    // Get schema id
    let schema_id = if let Some(id_attr) = id_attr {
        let id_attr_span = id_attr.span();
        let schema_id = SpannedValue::into_inner(id_attr);

        if schema.id != schema_id {
            emit_error!(
                id_attr_span,
                "id does not match the one specified in the schema file"
            );
        }

        Some(schema_id)
    } else {
        None
    };

    let settings_struct = syn::parse_macro_input!(item as SettingsStruct);

    // Parse overrides
    let known_signatures = schema
        .keys
        .iter()
        .map(|key| {
            key.signature().unwrap_or_else(|| {
                abort!(file_attr_span, "expected one of `type`, `enum` or `flags` specified attribute on key `{}` in the schema", key.name);
            })
        })
        .collect::<Vec<_>>();
    let known_key_names = schema
        .keys
        .iter()
        .map(|key| key.name.as_str())
        .collect::<Vec<_>>();
    let mut signature_overrides = HashMap::new();
    let mut key_name_overrides = HashMap::new();
    for attr in &settings_struct.attrs {
        let (signature, key_name, override_type) = if attr.path().is_ident("gen_settings_define") {
            let GenSettingsDefine {
                signature,
                key_name,
                arg_type,
                ret_type,
            } = match deluxe::parse_attributes::<_, GenSettingsDefine>(attr) {
                Ok(gen_settings) => gen_settings,
                Err(err) => {
                    emit_error!(attr.span(), err);
                    continue;
                }
            };

            (
                signature,
                key_name,
                OverrideType::Define {
                    arg_type: SpannedValue::into_inner(arg_type),
                    ret_type: SpannedValue::into_inner(ret_type),
                },
            )
        } else if attr.path().is_ident("gen_settings_skip") {
            let GenSettingsSkip {
                signature,
                key_name,
            } = match deluxe::parse_attributes::<_, GenSettingsSkip>(attr) {
                Ok(gen_settings) => gen_settings,
                Err(err) => {
                    emit_error!(attr.span(), err);
                    continue;
                }
            };

            (signature, key_name, OverrideType::Skip)
        } else {
            emit_error!(
                attr.span(),
                "expected `#[gen_settings_define( .. )]` or `#[gen_settings_skip( .. )]`"
            );
            continue;
        };

        match (signature, key_name) {
            (Some(_), Some(_)) => {
                emit_error!(
                    attr.span(),
                    "cannot specify both `signature` and `key_name`"
                )
            }
            (None, None) => {
                emit_error!(attr.span(), "must specify either `signature` or `key_name`")
            }
            (Some(signature), None) => {
                let signature_span = signature.span();
                let signature_str = SpannedValue::into_inner(signature);
                let signature_type = SchemaKeySignature::Type(signature_str);

                if !known_signatures.contains(&signature_type) {
                    emit_error!(signature_span, "useless define for this signature");
                }

                if signature_overrides.contains_key(&signature_type) {
                    emit_error!(signature_span, "duplicate override");
                }

                signature_overrides.insert(signature_type, override_type);
            }
            (None, Some(key_name)) => {
                let key_name_span = key_name.span();
                let key_name_str = SpannedValue::into_inner(key_name);

                if !known_key_names.contains(&key_name_str.as_str()) {
                    emit_error!(key_name_span, "key_name not found in the schema");
                }

                if key_name_overrides.contains_key(&key_name_str) {
                    emit_error!(key_name_span, "duplicate override");
                }

                key_name_overrides.insert(key_name_str, override_type);
            }
        }
    }

    // Generate keys
    let enums = schema_list
        .enums
        .iter()
        .map(|enum_| (enum_.id.to_string(), enum_))
        .collect::<HashMap<_, _>>();
    let flags = schema_list
        .flags
        .iter()
        .map(|flag| (flag.id.to_string(), flag))
        .collect::<HashMap<_, _>>();

    let mut key_generators = KeyGenerators::with_defaults(enums.clone(), flags.clone());
    key_generators.add_signature_overrides(signature_overrides);
    key_generators.add_key_name_overrides(key_name_overrides);

    // Generate code
    let mut aux_token_stream = proc_macro2::TokenStream::new();
    let mut keys_token_stream = proc_macro2::TokenStream::new();

    if globals {
        for (id, enumeration) in enums {
            let enum_type = type_name_from_id(&id);
            aux_token_stream.extend(enum_token_stream(
                &enum_type,
                &enumeration
                    .values
                    .iter()
                    .map(|value| (value.nick.as_str(), Some(value.value)))
                    .collect::<Vec<_>>(),
                settings_struct.vis.clone(),
            ));
        }

        for (id, flag) in flags {
            let flag_type = type_name_from_id(&id);
            aux_token_stream.extend(bitflag_token_stream(
                &flag_type,
                flag,
                settings_struct.vis.clone(),
            ));
        }
    }

    for key in &schema.keys {
        match key_generators
            .get(key, settings_struct.vis.clone())
            .unwrap()
        {
            GetResult::Skip => (),
            GetResult::Some(generator) => {
                keys_token_stream.extend(generator.to_token_stream());

                if let Some(aux) = generator.auxiliary() {
                    aux_token_stream.extend(aux);
                }
            }
            GetResult::Unknown => {
                emit_call_site_error!(
                    "unsupported {} signature used by key `{}`; consider using `#[gen_settings_define( .. )]` or skip it with `#[gen_settings_skip( .. )]`",
                    &key.signature().unwrap(),
                    &key.name,
                )
            }
        }
    }

    let constructor_token_stream = if schema_id.is_some() && impl_default {
        let schema_id = schema_id.as_ref().unwrap();
        quote! {
            pub fn new() -> Self {
                Self(gio::Settings::new(#schema_id))
            }
        }
    } else {
        quote! {
            pub fn new(schema_id: &str) -> Self {
                Self(gio::Settings::new(schema_id))
            }
        }
    };

    let struct_ident = &settings_struct.ident;

    let mut expanded = quote! {
        #aux_token_stream

        #[derive(Clone, Hash, PartialEq, Eq, gio::glib::ValueDelegate)]
        #[value_delegate(nullable)]
        #settings_struct

        impl #struct_ident {
            #constructor_token_stream

            #keys_token_stream
        }

        impl std::ops::Deref for #struct_ident {
            type Target = gio::Settings;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for #struct_ident {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl std::fmt::Debug for #struct_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.0, f)
            }
        }
    };

    if schema_id.is_some() && impl_default {
        expanded.extend(quote! {
            impl Default for #struct_ident {
                fn default() -> Self {
                    Self::new()
                }
            }
        });
    }

    expanded.into()
}
