use cycbox_sdk::{Color, Content, ContentType, Decoration};

#[test]
fn plain_content() {
    let c = Content::plain(b"hello".to_vec());
    assert_eq!(c.content_type, ContentType::Text);
    assert!(c.decoration.is_empty());
    assert_eq!(c.payload, b"hello");
    assert_eq!(c.label, None);
}

#[test]
fn styled_content() {
    let c = Content::styled(b"err".to_vec(), Color::Error, true, Some("lbl".into()));
    assert_eq!(c.content_type, ContentType::RichText);
    assert!(c.decoration.bold);
    assert_eq!(c.decoration.color, Color::Error);
    assert_eq!(c.label, Some("lbl".into()));
}

#[test]
fn data_content() {
    let c = Content::data(b"AB".to_vec(), Some("data".into()));
    assert_eq!(c.decoration.color, Color::OnSurface);
    assert!(!c.decoration.bold);
}

#[test]
fn address_content() {
    let c = Content::address(b"0x00".to_vec(), None);
    assert_eq!(c.decoration.color, Color::Primary);
}

#[test]
fn separator_content() {
    let c = Content::separator(b"|".to_vec());
    assert_eq!(c.decoration.color, Color::Outline);
    assert_eq!(c.label, None);
}

#[test]
fn error_indicator_content() {
    let c = Content::error_indicator(b"ERR".to_vec(), None);
    assert_eq!(c.decoration.color, Color::Error);
}

#[test]
fn checksum_valid() {
    let c = Content::checksum(b"AB".to_vec(), None);
    assert_eq!(c.decoration.color, Color::Tertiary);
}

#[test]
fn checksum_invalid() {
    let c = Content::checksum_invalid(b"AB".to_vec(), None);
    assert_eq!(c.decoration.color, Color::Error);
}

#[test]
fn highlight_content() {
    let c = Content::highlight(b"match".to_vec(), None);
    assert!(c.decoration.bold);
    assert_eq!(c.decoration.color, Color::Error);
}

// ---- Decoration ----

#[test]
fn decoration_default_is_empty() {
    assert!(Decoration::default().is_empty());
}

#[test]
fn decoration_not_empty_bold() {
    let d = Decoration {
        bold: true,
        ..Default::default()
    };
    assert!(!d.is_empty());
}

#[test]
fn decoration_not_empty_color() {
    let d = Decoration {
        color: Color::Error,
        ..Default::default()
    };
    assert!(!d.is_empty());
}

// ---- Color serde ----

#[test]
fn color_serde_roundtrip() {
    let colors = [
        Color::Primary,
        Color::Error,
        Color::OnSurface,
        Color::Transparent,
        Color::SurfaceContainerHighest,
    ];
    for c in colors {
        let json = serde_json::to_string(&c).unwrap();
        let back: Color = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }
}
