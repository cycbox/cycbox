use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use cycbox_sdk::prelude::*;

pub const LINE_CODEC_ID: &str = "line_codec";

#[derive(Debug, Clone, PartialEq)]
pub enum LineEnding {
    Lf,   // \n
    CrLf, // \r\n
}

impl LineEnding {
    pub fn new_from_str(s: &str) -> Self {
        match s {
            "lf" => LineEnding::Lf,
            "crlf" => LineEnding::CrLf,
            _ => LineEnding::Lf, // Default to Lf
        }
    }
}

pub struct LineCodec {
    line_ending: LineEnding,
}

impl LineCodec {
    #[allow(dead_code)]
    pub fn new(line_ending: LineEnding) -> Self {
        Self { line_ending }
    }
}

impl Default for LineCodec {
    fn default() -> Self {
        Self {
            line_ending: LineEnding::Lf,
        }
    }
}

#[async_trait]
impl Codec for LineCodec {
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        let line = src.as_ref();
        match self.line_ending {
            LineEnding::Lf => {
                if let Some(index) = line.iter().position(|&b| b == b'\n') {
                    let frame = src.split_to(index + 1);
                    // Payload is frame without the trailing \n
                    let payload = frame[..frame.len() - 1].to_vec();
                    let builder =
                        MessageBuilder::rx(PayloadType::Binary, payload.clone(), frame.to_vec());
                    return Ok(Some(builder.build()));
                }
            }
            LineEnding::CrLf => {
                // find \r\n in src
                for (i, &b) in line.iter().enumerate() {
                    if b == b'\r' && i + 1 < line.len() && line[i + 1] == b'\n' {
                        let frame = src.split_to(i + 2);
                        // Payload is frame without the trailing \r\n
                        let payload = frame[..frame.len() - 2].to_vec();
                        let builder = MessageBuilder::rx(
                            PayloadType::Binary,
                            payload.clone(),
                            frame.to_vec(),
                        );
                        return Ok(Some(builder.build()));
                    }
                }
            }
        }
        Ok(None)
    }

    fn encode(&mut self, item: &mut Message) -> Result<(), CycBoxError> {
        let mut dst = BytesMut::new();
        dst.extend_from_slice(&item.payload);
        // Add line ending if not already present
        match self.line_ending {
            LineEnding::Lf => {
                if !item.payload.ends_with(b"\n") {
                    dst.put_u8(b'\n');
                }
            }
            LineEnding::CrLf => {
                if !item.payload.ends_with(b"\r\n") {
                    dst.put_u8(b'\r');
                    dst.put_u8(b'\n');
                }
            }
        }
        item.frame = dst.to_vec();
        Ok(())
    }
}

#[async_trait]
impl Manifestable for LineCodec {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: LINE_CODEC_ID.to_string(),
            name: l10n.get(locale, "line-codec"),
            description: l10n.get(locale, "codec-line-description"),
            category: PluginCategory::Codec,
            config_schema: vec![FormGroup {
                key: LINE_CODEC_ID.to_string(),
                label: l10n.get(locale, "line-codec"),
                description: None,
                fields: vec![FormField {
                    key: format!("{LINE_CODEC_ID}_line_ending"),
                    field_type: FieldType::TextChoiceChip,
                    label: l10n.get(locale, "line-codec-end-label"),
                    description: None,
                    values: Some(vec![FormValue::Text("lf".to_string())]),
                    options: Some(vec![
                        FormFieldOption::new(
                            l10n.get(locale, "line-codec-packet-end-lf"),
                            FormValue::Text("lf".to_string()),
                        ),
                        FormFieldOption::new(
                            l10n.get(locale, "line-codec-packet-end-crlf"),
                            FormValue::Text("crlf".to_string()),
                        ),
                    ]),
                    is_required: true,
                    condition: None,
                    span: 6,
                }],
                condition: None,
            }],
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for LineCodec {
    async fn config(&mut self, config: &[FormGroup]) -> Result<(), CycBoxError> {
        let line_ending_key = format!("{LINE_CODEC_ID}_line_ending");
        let line_ending_config =
            FormUtils::get_text_value(config, LINE_CODEC_ID, &line_ending_key).unwrap_or("lf");
        self.line_ending = LineEnding::new_from_str(line_ending_config);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_ending_from_str() {
        assert_eq!(LineEnding::new_from_str("lf"), LineEnding::Lf);
        assert_eq!(LineEnding::new_from_str("crlf"), LineEnding::CrLf);
        // Test default case - defaults to Lf
        assert_eq!(LineEnding::new_from_str("invalid"), LineEnding::Lf);
        assert_eq!(LineEnding::new_from_str(""), LineEnding::Lf);
    }
}
