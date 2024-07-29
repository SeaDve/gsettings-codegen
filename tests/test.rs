use gio::{
    glib::{self, VariantTy},
    prelude::*,
};
use gsettings_macro::gen_settings;

use std::{
    cell::Cell,
    env,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    sync::Once,
};

static INIT: Once = Once::new();

fn setup_schema() {
    INIT.call_once(|| {
        let schema_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");

        let output = Command::new("glib-compile-schemas")
            .arg(schema_dir)
            .output()
            .unwrap();

        if !output.status.success() {
            println!("Failed to generate GSchema!");
            println!(
                "glib-compile-schemas stdout: {}",
                String::from_utf8_lossy(&output.stdout)
            );
            println!(
                "glib-compile-schemas stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            panic!("Can't test without GSchemas!");
        }

        env::set_var("GSETTINGS_SCHEMA_DIR", schema_dir);
        env::set_var("GSETTINGS_BACKEND", "memory");
    });
}

#[test]
#[serial_test::serial]
fn setter_and_getter_func() {
    setup_schema();

    #[gen_settings(file = "tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    let settings = Settings::new("io.github.seadve.test");

    settings.set_is_maximized(true);
    assert!(settings.is_maximized());

    settings.set_theme("dark");
    assert_eq!(settings.theme(), "dark");

    settings.set_invalid_words(&["invalid", "words"]);
    assert_eq!(settings.invalid_words(), vec!["invalid", "words"]);

    settings.set_window_width(30_000);
    assert_eq!(settings.window_width(), 30_000);

    settings.set_window_height(30_000);
    assert_eq!(settings.window_height(), 30_000);

    settings.set_window_width_64(30_000);
    assert_eq!(settings.window_width_64(), 30_000);

    settings.set_window_height_64(30_000);
    assert_eq!(settings.window_height_64(), 30_000);

    settings.set_volume(1.0);
    assert_eq!(settings.volume(), 1.0);

    settings.set_preferred_audio_source(PreferredAudioSource::DesktopAudio);
    assert_eq!(
        settings.preferred_audio_source(),
        PreferredAudioSource::DesktopAudio
    );

    settings.set_dimensions((20, 20));
    assert_eq!(settings.dimensions(), (20, 20));
}

#[test]
#[serial_test::serial]
fn create_action_func() {
    setup_schema();

    #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_define(
        signature = "(ss)",
        arg_type = "(&str, &str)",
        ret_type = "(String, String)"
    )]
    #[gen_settings_define(key_name = "cache-dir", arg_type = "&Path", ret_type = "PathBuf")]
    pub struct SomeAppSettings;

    let settings = SomeAppSettings::new("io.github.seadve.test");

    assert_eq!(settings.create_is_maximized_action().name(), "is-maximized");
    assert_eq!(settings.create_theme_action().name(), "theme");
    assert_eq!(
        settings.create_invalid_words_action().name(),
        "invalid-words"
    );
    assert_eq!(settings.create_window_width_action().name(), "window-width");
    assert_eq!(
        settings.create_window_height_action().name(),
        "window-height"
    );
    assert_eq!(
        settings.create_window_width_64_action().name(),
        "window-width-64"
    );
    assert_eq!(
        settings.create_window_height_64_action().name(),
        "window-height-64"
    );
    assert_eq!(settings.create_volume_action().name(), "volume");
    assert_eq!(
        settings.create_preferred_audio_source_action().name(),
        "preferred-audio-source"
    );
    assert_eq!(settings.create_dimensions_action().name(), "dimensions");
    assert_eq!(settings.create_string_tuple_action().name(), "string-tuple");
    assert_eq!(settings.create_cache_dir_action().name(), "cache-dir");
}

#[test]
#[serial_test::serial]
fn reset_func() {
    setup_schema();

    #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct SomeAppSettings;

    let settings = SomeAppSettings::new("io.github.seadve.test");
    assert_eq!(settings.alert_sound_default_value(), AlertSound::Bark);

    settings.set_alert_sound(AlertSound::Drip);
    assert_eq!(settings.alert_sound(), AlertSound::Drip);

    settings.reset_alert_sound();
    assert_eq!(settings.alert_sound(), AlertSound::Bark);
}

#[test]
#[serial_test::serial]
fn default_value_func() {
    #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_define(
        signature = "(ss)",
        arg_type = "(&str, &str)",
        ret_type = "(String, String)"
    )]
    #[gen_settings_define(key_name = "cache-dir", arg_type = "&Path", ret_type = "PathBuf")]
    pub struct Settings;

    let settings = Settings::new("io.github.seadve.test");
    assert!(!settings.is_maximized_default_value());
    assert_eq!(settings.theme_default_value(), "light");
    assert_eq!(settings.invalid_words_default_value(), Vec::<String>::new());
    assert_eq!(settings.window_width_default_value(), 600);
    assert_eq!(settings.window_height_default_value(), 400);
    assert_eq!(settings.window_width_64_default_value(), 600);
    assert_eq!(settings.window_height_64_default_value(), 400);
    assert_eq!(settings.volume_default_value(), 6.3);
    assert_eq!(
        settings.preferred_audio_source_default_value(),
        PreferredAudioSource::Microphone
    );
    assert_eq!(settings.dimensions_default_value(), (10, 10));
    assert_eq!(
        settings.string_tuple_default_value(),
        ("string".to_string(), "another one".to_string())
    );
    assert_eq!(
        settings.cache_dir_default_value(),
        PathBuf::from("/tmp/cache_dir/")
    );
    assert_eq!(settings.alert_sound_default_value(), AlertSound::Bark);
    assert_eq!(settings.space_style_default_value(), SpaceStyle::empty());
}

#[test]
#[serial_test::serial]
fn other_func() {
    setup_schema();

    #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct SomeAppSettings;

    let settings = SomeAppSettings::new("io.github.seadve.test");

    // Just sample object. You should never want to bind a theme
    // to the application id.
    let object = gio::Application::new(Some("some.initial.id"), gio::ApplicationFlags::FLAGS_NONE);

    settings.set_theme("some.initial.theme");
    settings.bind_theme(&object, "application-id").build();

    let n_theme_changed_calls = Rc::new(Cell::new(0));
    let n_application_id_notify_calls = Rc::new(Cell::new(0));

    let n_theme_changed_calls_clone = Rc::clone(&n_theme_changed_calls);
    settings.connect_theme_changed(move |settings| {
        assert_ne!(settings.theme(), "some.initial.theme");
        n_theme_changed_calls_clone.set(n_theme_changed_calls_clone.get() + 1);
    });

    let n_application_id_notify_calls_clone = Rc::clone(&n_application_id_notify_calls);
    object.connect_application_id_notify(move |_| {
        n_application_id_notify_calls_clone.set(n_application_id_notify_calls_clone.get() + 1);
    });

    settings.set_theme("org.some.testid");
    assert_eq!(settings.theme(), "org.some.testid");
    assert_eq!(Some(settings.theme().into()), object.application_id());

    assert_eq!(n_theme_changed_calls.get(), 1);
    assert_eq!(n_application_id_notify_calls.get(), 1);

    settings.set_theme("org.some.another.id");
    assert_eq!(settings.theme(), "org.some.another.id");
    assert_eq!(Some(settings.theme().into()), object.application_id());

    assert_eq!(n_theme_changed_calls.get(), 2);
    assert_eq!(n_application_id_notify_calls.get(), 2);
}

#[test]
#[serial_test::serial]
fn custom_define_signature() {
    setup_schema();

    #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_define(
        signature = "(ss)",
        arg_type = "(&str, &str)",
        ret_type = "(String, String)"
    )]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    let settings = Settings::new("io.github.seadve.test");

    settings.set_string_tuple(("hi", "hi2"));
    assert_eq!(settings.string_tuple(), ("hi".into(), "hi2".into()));

    settings.set_two_strings(("a string", "another string"));
    assert_eq!(
        settings.two_strings(),
        ("a string".into(), "another string".into())
    );
}

#[test]
#[serial_test::serial]
fn custom_define_key_name() {
    setup_schema();

    #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_define(key_name = "cache-dir", arg_type = "&Path", ret_type = "PathBuf")]
    #[gen_settings_skip(signature = "(ss)")]
    pub struct Settings;

    let settings = Settings::new("io.github.seadve.test");
    settings.set_cache_dir(Path::new("/some_dir"));
    assert_eq!(settings.cache_dir(), PathBuf::from("/some_dir"));
}

#[test]
#[serial_test::serial]
fn overlapping_define() {
    setup_schema();

    #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_define(signature = "ay", arg_type = "&OsStr", ret_type = "OsString")]
    #[gen_settings_define(key_name = "cache-dir", arg_type = "&Path", ret_type = "PathBuf")]
    #[gen_settings_skip(signature = "(ss)")]
    pub struct Settings;

    let settings = Settings::new("io.github.seadve.test");
    settings.set_cache_dir(Path::new("/some_dir"));
    assert_eq!(settings.cache_dir(), PathBuf::from("/some_dir"));
}

#[test]
#[serial_test::serial]
fn string_choice_enum() {
    setup_schema();

    #[gen_settings(file = "./tests/io.github.seadve.test.gschema.xml")]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    assert_eq!(
        PreferredAudioSource::static_variant_type(),
        gio::glib::VariantTy::STRING
    );

    assert_eq!(
        PreferredAudioSource::DesktopAudio
            .to_variant()
            .get::<String>()
            .unwrap(),
        "desktop-audio"
    );
    assert_eq!(
        PreferredAudioSource::Microphone
            .to_variant()
            .get::<String>()
            .unwrap(),
        "microphone"
    );

    assert_eq!(
        PreferredAudioSource::DesktopAudio,
        PreferredAudioSource::from_variant(&"desktop-audio".to_variant()).unwrap()
    );
    assert_eq!(
        PreferredAudioSource::Microphone,
        PreferredAudioSource::from_variant(&"microphone".to_variant()).unwrap()
    );
}

#[test]
#[serial_test::serial]
fn enumeration() {
    setup_schema();

    #[gen_settings(
        file = "./tests/io.github.seadve.test.gschema.xml",
        id = "io.github.seadve.test"
    )]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    let settings = Settings::new();

    assert_eq!(settings.alert_sound(), AlertSound::Bark);

    settings.set_alert_sound(AlertSound::Glass);
    assert_eq!(settings.alert_sound(), AlertSound::Glass);

    settings.set_alert_sound(AlertSound::Drip);
    assert_eq!(settings.alert_sound(), AlertSound::Drip);
}

#[test]
#[serial_test::serial]
fn enumeration_value() {
    setup_schema();

    #[gen_settings(
        file = "./tests/io.github.seadve.test.gschema.xml",
        id = "io.github.seadve.test"
    )]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    assert_eq!(AlertSound::Bark as i32, 0);
    assert_eq!(AlertSound::Glass as i32, 2);
    assert_eq!(AlertSound::Drip as i32, 1);
}

#[test]
#[serial_test::serial]
fn bitflag() {
    setup_schema();

    #[gen_settings(
        file = "./tests/io.github.seadve.test.gschema.xml",
        id = "io.github.seadve.test"
    )]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    let settings = Settings::new();

    assert_eq!(SpaceStyle::static_variant_type(), VariantTy::STRING_ARRAY);

    assert_eq!(settings.space_style(), SpaceStyle::empty());

    settings.set_space_style(SpaceStyle::BEFORE_COLON);
    assert_eq!(settings.space_style(), SpaceStyle::BEFORE_COLON);

    settings.set_space_style(SpaceStyle::BEFORE_COLON | SpaceStyle::BEFORE_COMMA);
    assert_eq!(
        settings.space_style(),
        SpaceStyle::BEFORE_COLON | SpaceStyle::BEFORE_COMMA
    );

    settings.set_space_style(
        SpaceStyle::BEFORE_COLON | SpaceStyle::BEFORE_COMMA | SpaceStyle::BEFORE_SEMICOLON,
    );
    assert_eq!(settings.space_style(), SpaceStyle::all());
}

#[test]
#[serial_test::serial]
fn bitflag_value() {
    setup_schema();

    #[gen_settings(
        file = "./tests/io.github.seadve.test.gschema.xml",
        id = "io.github.seadve.test"
    )]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    assert_eq!(SpaceStyle::BEFORE_COLON.bits(), 1);
    assert_eq!(SpaceStyle::BEFORE_SEMICOLON.bits(), 4);
    assert_eq!(SpaceStyle::BEFORE_COMMA.bits(), 2);

    assert_eq!(SpaceStyle::from_bits(1).unwrap(), SpaceStyle::BEFORE_COLON);
    assert_eq!(
        SpaceStyle::from_bits(4).unwrap(),
        SpaceStyle::BEFORE_SEMICOLON
    );
    assert_eq!(SpaceStyle::from_bits(2).unwrap(), SpaceStyle::BEFORE_COMMA);

    assert_eq!(
        SpaceStyle::from_variant(&["before-comma", "before-colon"].to_variant()),
        Some(SpaceStyle::BEFORE_COLON | SpaceStyle::BEFORE_COMMA)
    );
    assert_eq!(
        SpaceStyle::from_variant(&["before-comma", "invalid"].to_variant()),
        None
    );

    assert_eq!(
        SpaceStyle::BEFORE_COLON.to_variant(),
        ["before-colon"].to_variant()
    );
    assert_eq!(
        (SpaceStyle::BEFORE_COLON | SpaceStyle::BEFORE_SEMICOLON).to_variant(),
        ["before-colon", "before-semicolon"].to_variant()
    );
}

#[test]
#[serial_test::serial]
fn id_defined_in_macro() {
    setup_schema();

    #[gen_settings(
        file = "./tests/io.github.seadve.test.gschema.xml",
        id = "io.github.seadve.test"
    )]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    assert_eq!(Settings::default().schema_id(), Settings::new().schema_id());
    assert_eq!(
        Settings::default().schema_id().as_deref(),
        Some("io.github.seadve.test")
    );
}

#[test]
#[serial_test::serial]
fn private_struct() {
    setup_schema();

    mod inner {
        use gio::glib;
        use gsettings_macro::gen_settings;

        #[gen_settings(
            file = "./tests/io.github.seadve.test.gschema.xml",
            id = "io.github.seadve.test"
        )]
        #[gen_settings_skip(signature = "(ss)")]
        #[gen_settings_skip(signature = "ay")]
        struct Settings;
    }

    // TODO: Use `trybuild` to test if these would cause failed compilation
    //
    // use inner::AlertSound;
    // use inner::PreferredAudioSource;
    // use inner::Settings;
    // use inner::SpaceStyle;
}

#[test]
#[serial_test::serial]
fn multiple_schemas() {
    setup_schema();

    #[gen_settings(
        file = "tests/io.github.seadve.test.multi.gschema.xml",
        id = "io.github.seadve.test.multi"
    )]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    let settings = Settings::default();

    settings.set_theme("dark");
    assert_eq!(settings.theme(), "dark");

    settings.set_invalid_words(&["invalid", "words"]);
    assert_eq!(settings.invalid_words(), vec!["invalid", "words"]);

    settings.set_volume(1.0);
    assert_eq!(settings.volume(), 1.0);

    settings.set_preferred_audio_source(PreferredAudioSource::DesktopAudio);
    assert_eq!(
        settings.preferred_audio_source(),
        PreferredAudioSource::DesktopAudio
    );

    settings.set_dimensions((20, 20));
    assert_eq!(settings.dimensions(), (20, 20));

    #[gen_settings(
        file = "tests/io.github.seadve.test.multi.gschema.xml",
        id = "io.github.seadve.test.multi.window-state",
        globals = false
    )]
    pub struct WindowStateSettings;
    let window_settings = WindowStateSettings::default();

    window_settings.set_is_maximized(true);
    assert!(window_settings.is_maximized());

    window_settings.set_window_width(30_000);
    assert_eq!(window_settings.window_width(), 30_000);

    window_settings.set_window_height(30_000);
    assert_eq!(window_settings.window_height(), 30_000);

    window_settings.set_window_width_64(30_000);
    assert_eq!(window_settings.window_width_64(), 30_000);

    window_settings.set_window_height_64(30_000);
    assert_eq!(window_settings.window_height_64(), 30_000);
}

#[test]
#[serial_test::serial]
fn multiple_schemas_no_default() {
    setup_schema();

    #[gen_settings(
        file = "tests/io.github.seadve.test.multi.gschema.xml",
        id = "io.github.seadve.test.multi",
        default
    )]
    #[gen_settings_skip(signature = "(ss)")]
    #[gen_settings_skip(signature = "ay")]
    pub struct Settings;

    let settings = Settings::default();

    settings.set_theme("dark");
    assert_eq!(settings.theme(), "dark");

    settings.set_invalid_words(&["invalid", "words"]);
    assert_eq!(settings.invalid_words(), vec!["invalid", "words"]);

    settings.set_volume(1.0);
    assert_eq!(settings.volume(), 1.0);

    settings.set_preferred_audio_source(PreferredAudioSource::DesktopAudio);
    assert_eq!(
        settings.preferred_audio_source(),
        PreferredAudioSource::DesktopAudio
    );

    settings.set_dimensions((20, 20));
    assert_eq!(settings.dimensions(), (20, 20));

    #[gen_settings(
        file = "tests/io.github.seadve.test.multi.gschema.xml",
        id = "io.github.seadve.test.multi.window-state",
        default = false,
        globals = false
    )]
    pub struct WindowStateSettings;

    impl Default for WindowStateSettings {
        fn default() -> Self {
            Self::new("io.github.seadve.test.multi.window-state")
        }
    }

    let window_settings = WindowStateSettings::default();

    window_settings.set_is_maximized(true);
    assert!(window_settings.is_maximized());

    window_settings.set_window_width(30_000);
    assert_eq!(window_settings.window_width(), 30_000);

    window_settings.set_window_height(30_000);
    assert_eq!(window_settings.window_height(), 30_000);

    window_settings.set_window_width_64(30_000);
    assert_eq!(window_settings.window_width_64(), 30_000);

    window_settings.set_window_height_64(30_000);
    assert_eq!(window_settings.window_height_64(), 30_000);
}
