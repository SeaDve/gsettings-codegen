use heck::ToPascalCase;

use super::{Context, KeyGenerator, SchemaKey};

pub fn key_generator(key: &SchemaKey, aux_visibility: syn::Visibility) -> KeyGenerator<'_> {
    if let Some(ref choices) = key.choices {
        let choice_enum_name = key.name.to_pascal_case();
        let choice_enum_token_stream = super::enum_token_stream(
            &choice_enum_name,
            &choices
                .choices
                .iter()
                .map(|choice| (choice.value.as_str(), None))
                .collect::<Vec<_>>(),
            aux_visibility,
        );
        KeyGenerator::new(
            key,
            Context::new_with_aux(&choice_enum_name, choice_enum_token_stream),
        )
    } else {
        KeyGenerator::new(key, Context::new_dissimilar("&str", "String"))
    }
}
