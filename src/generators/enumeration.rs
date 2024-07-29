use super::{Context, KeyGenerator, SchemaEnum, SchemaKey};

pub fn key_generator<'a>(
    key: &'a SchemaKey,
    enum_: &SchemaEnum,
    _aux_visibility: syn::Visibility,
) -> KeyGenerator<'a> {
    let enum_type = super::type_name_from_id(&enum_.id);
    KeyGenerator::new(key, Context::new_dissimilar(&enum_type, &enum_type))
}
