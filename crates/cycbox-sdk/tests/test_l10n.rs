use cycbox_sdk::L10n;

#[test]
fn from_bytes_basic() {
    let l10n = L10n::from_bytes("en", b"hello = Hello World");
    assert_eq!(l10n.get("en", "hello"), "Hello World");
}

#[test]
fn get_returns_key_when_not_found() {
    let l10n = L10n::from_bytes("en", b"hello = Hello");
    assert_eq!(l10n.get("en", "nonexistent"), "nonexistent");
}

#[test]
fn fallback_to_en() {
    let l10n = L10n::from_bytes("en", b"hello = Hello World");
    // Requesting "zh" which doesn't exist falls back to "en"
    assert_eq!(l10n.get("zh", "hello"), "Hello World");
}

#[test]
fn no_bundles_returns_key() {
    // Create with an invalid FTL that won't parse to anything useful
    let l10n = L10n::from_bytes("xx", b"");
    assert_eq!(l10n.get("en", "anything"), "anything");
}

#[test]
fn get_with_args_interpolation() {
    let l10n = L10n::from_bytes("en", b"greeting = Hello { $name }");
    let args = cycbox_sdk::fluent_args!("name" => "World");
    let result = l10n.get_with_args("en", "greeting", Some(&args));
    assert!(result.contains("World"));
}

#[test]
fn multiple_keys() {
    let l10n = L10n::from_bytes(
        "en",
        b"key-one = First\nkey-two = Second\nkey-three = Third",
    );
    assert_eq!(l10n.get("en", "key-one"), "First");
    assert_eq!(l10n.get("en", "key-two"), "Second");
    assert_eq!(l10n.get("en", "key-three"), "Third");
}
