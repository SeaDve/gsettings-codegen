use super::{Context, KeyGenerator, SchemaFlag, SchemaKey};

pub fn key_generator<'a>(
    key: &'a SchemaKey,
    flag: &SchemaFlag,
    _aux_visibility: syn::Visibility,
) -> KeyGenerator<'a> {
    let flag_type = super::type_name_from_id(&flag.id);
    KeyGenerator::new(key, Context::new_dissimilar(&flag_type, &flag_type))
}
