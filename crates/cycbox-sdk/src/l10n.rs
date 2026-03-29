use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use std::collections::HashMap;
use unic_langid::{LanguageIdentifier, langid};

/// Trait for types that can provide locale files
/// This allows plugins to embed their locales using rust-embed or other mechanisms
pub trait LocaleProvider: Send + Sync {
    /// Get the content of a locale file by name (e.g., "en.ftl")
    fn get_locale(&self, locale: &str) -> Option<&'static [u8]>;

    /// Get a list of available locale names (e.g., ["en", "zh"])
    fn available_locales(&self) -> Vec<&'static str>;
}

/// Localization manager for SDK and plugins
///
/// This is designed to be WASM-friendly and work with embedded locale files.
/// Use the `embed_locales!` macro to create instances easily.
///
/// Supports loading multiple locales at once and switching between them without reinitialization.
pub struct L10n {
    bundles: HashMap<String, FluentBundle<FluentResource>>,
    fallback_locale: String,
}

impl L10n {
    /// Create a new L10n instance loading all available locales from the provider
    ///
    /// # Arguments
    /// * `locale` - The initial locale to use
    /// * `provider` - The locale provider that supplies locale files
    ///
    /// The fallback locale is always "en". If the requested locale is not found,
    /// it will use "en" as the current locale. All available locales from the provider
    /// are loaded during initialization.
    pub fn new(provider: &dyn LocaleProvider) -> Self {
        let available_locales = provider.available_locales();
        let mut bundles = HashMap::new();
        // Load all available locales
        for locale_name in available_locales {
            let locale_file = format!("{locale_name}.ftl");
            if let Some(content) = provider.get_locale(&locale_file) {
                let lang_id: LanguageIdentifier =
                    locale_name.parse().unwrap_or_else(|_| langid!("en"));

                let mut bundle = FluentBundle::new_concurrent(vec![lang_id]);
                let source = String::from_utf8_lossy(content);

                match FluentResource::try_new(source.to_string()) {
                    Ok(resource) => {
                        if let Err(errors) = bundle.add_resource(resource) {
                            eprintln!(
                                "Failed to add resource for {}: {} error(s)",
                                locale_name,
                                errors.len()
                            );
                        } else {
                            bundles.insert(locale_name.to_string(), bundle);
                        }
                    }
                    Err((_resource, parse_errors)) => {
                        eprintln!(
                            "Failed to parse fluent resource for {}: {} error(s)",
                            locale_name,
                            parse_errors.len()
                        );
                    }
                }
            }
        }
        let fallback_locale = if bundles.contains_key("en") {
            "en".to_string()
        } else if let Some(first_locale) = bundles.keys().next() {
            first_locale.to_string()
        } else {
            println!("WARNING: No locale bundles loaded successfully! Using empty fallback.");
            "en".to_string() // fallback even if no bundles loaded
        };
        Self {
            bundles,
            fallback_locale,
        }
    }

    /// Create a new L10n instance directly from embedded bytes for a single locale
    /// This is useful when you have a single locale or want manual control
    pub fn from_bytes(locale: &str, ftl_content: &[u8]) -> Self {
        let lang_id: LanguageIdentifier = locale.parse().unwrap_or_else(|_| langid!("en"));

        let mut bundle = FluentBundle::new_concurrent(vec![lang_id]);
        let source = String::from_utf8_lossy(ftl_content);

        if let Ok(resource) = FluentResource::try_new(source.to_string()) {
            let _ = bundle.add_resource(resource);
        }

        let mut bundles = HashMap::new();
        bundles.insert(locale.to_string(), bundle);

        Self {
            bundles,
            fallback_locale: "en".to_string(),
        }
    }

    /// Add locale resources from an additional provider
    ///
    /// Merges resources into existing bundles for matching locales, and creates
    /// new bundles for locales not yet loaded. Must be called before the instance
    /// is shared (i.e., before storing in OnceLock/Arc).
    pub fn add_provider(&mut self, provider: &dyn LocaleProvider) {
        for locale_name in provider.available_locales() {
            let locale_file = format!("{locale_name}.ftl");
            if let Some(content) = provider.get_locale(&locale_file) {
                let source = String::from_utf8_lossy(content);
                match FluentResource::try_new(source.to_string()) {
                    Ok(resource) => {
                        if let Some(bundle) = self.bundles.get_mut(locale_name) {
                            bundle.add_resource_overriding(resource);
                        } else {
                            let lang_id: LanguageIdentifier =
                                locale_name.parse().unwrap_or_else(|_| langid!("en"));
                            let mut bundle = FluentBundle::new_concurrent(vec![lang_id]);
                            if let Err(errors) = bundle.add_resource(resource) {
                                eprintln!(
                                    "Failed to add resource for {}: {} error(s)",
                                    locale_name,
                                    errors.len()
                                );
                            } else {
                                self.bundles.insert(locale_name.to_string(), bundle);
                            }
                        }
                    }
                    Err((_resource, parse_errors)) => {
                        eprintln!(
                            "Failed to parse fluent resource for {}: {} error(s)",
                            locale_name,
                            parse_errors.len()
                        );
                    }
                }
            }
        }
    }

    /// Get a localized message for a specific locale
    ///
    /// Returns the key itself if not found in that locale.
    pub fn get(&self, locale: &str, key: &str) -> String {
        self.get_with_args(locale, key, None)
    }

    /// Get a localized message for a specific locale with arguments
    ///
    /// Returns None if the locale is not loaded, or the key itself if not found in that locale.
    pub fn get_with_args(&self, locale: &str, key: &str, args: Option<&FluentArgs>) -> String {
        let bundle = if let Some(b) = self.bundles.get(locale) {
            b
        } else if let Some(b) = self.bundles.get(&self.fallback_locale) {
            b
        } else {
            // No bundles available at all
            return key.to_string();
        };

        if let Some(message) = bundle.get_message(key)
            && let Some(pattern) = message.value()
        {
            let mut errors = vec![];
            let value = bundle.format_pattern(pattern, args, &mut errors);
            return value.to_string();
        }

        // Return the key itself if not found
        key.to_string()
    }
}

/// Create a fluent argument for use in localized strings
#[macro_export]
macro_rules! fluent_args {
    ($($key:expr => $value:expr),* $(,)?) => {
        {
            let mut args = fluent_bundle::FluentArgs::new();
            $(
                args.set($key, $value);
            )*
            args
        }
    };
}

#[macro_export]
macro_rules! embed_locales {
    ($folder:expr) => {
        #[derive(rust_embed::RustEmbed)]
        #[folder = $folder]
        struct EmbeddedLocales;

        struct LocaleProviderImpl;

        impl $crate::l10n::LocaleProvider for LocaleProviderImpl {
            fn get_locale(&self, locale: &str) -> Option<&'static [u8]> {
                use std::borrow::Cow;
                use std::collections::HashMap;
                use std::sync::OnceLock;

                // Cache all locale data on first access to get true 'static lifetime
                static LOCALE_DATA: OnceLock<HashMap<String, &'static [u8]>> = OnceLock::new();

                let data_map = LOCALE_DATA.get_or_init(|| {
                    let mut map = HashMap::new();
                    for file_path in EmbeddedLocales::iter() {
                        if let Some(file) = EmbeddedLocales::get(file_path.as_ref()) {
                            // Extract the 'static reference from the Cow
                            let static_data: &'static [u8] = match file.data {
                                Cow::Borrowed(b) => b,
                                Cow::Owned(v) => Box::leak(v.into_boxed_slice()),
                            };
                            map.insert(file_path.to_string(), static_data);
                        }
                    }
                    map
                });

                data_map.get(locale).copied()
            }

            fn available_locales(&self) -> Vec<&'static str> {
                use std::sync::OnceLock;
                static LOCALES: OnceLock<Vec<&'static str>> = OnceLock::new();

                LOCALES
                    .get_or_init(|| {
                        // Collect all embedded file names at compile time
                        // We need to leak the strings to get 'static lifetime
                        EmbeddedLocales::iter()
                            .filter_map(|path| {
                                let path_str = path.as_ref();
                                if let Some(locale_name) = path_str.strip_suffix(".ftl") {
                                    // This is acceptable because locale names are fixed at compile time
                                    Some(Box::leak(locale_name.to_string().into_boxed_str())
                                        as &'static str)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .clone()
            }
        }

        static LOCALE_PROVIDER: LocaleProviderImpl = LocaleProviderImpl;
    };
}

/// Helper function to create an L10n instance with a default provider
/// This is useful when you want to use the embedded locales
pub fn create_l10n_with_provider(provider: &'static dyn LocaleProvider) -> L10n {
    L10n::new(provider)
}

/// Macro to set up crate-level L10n with embedded locales.
///
/// This combines `embed_locales!`, a `OnceLock<Arc<L10n>>` singleton, and a `get_l10n()`
/// function that initializes from the crate's own locale files.
///
/// # Example
///
/// ```ignore
/// // In your crate's l10n.rs:
/// cycbox_sdk::setup_crate_l10n!("locales/");
/// ```
#[macro_export]
macro_rules! setup_crate_l10n {
    ($folder:expr) => {
        $crate::embed_locales!($folder);

        static L10N: std::sync::OnceLock<std::sync::Arc<$crate::l10n::L10n>> =
            std::sync::OnceLock::new();

        pub fn get_l10n() -> &'static $crate::l10n::L10n {
            L10N.get_or_init(|| std::sync::Arc::new($crate::l10n::L10n::new(&LOCALE_PROVIDER)))
        }
    };
}
