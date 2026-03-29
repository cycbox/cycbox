use async_trait::async_trait;
use cycbox_sdk::prelude::*;

pub const DISABLE_TRANSFORMER_ID: &str = "disable_transformer";

#[derive(Clone, Debug, Default)]
pub struct DisableTransformer;

impl DisableTransformer {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for DisableTransformer {
    fn on_receive(&self, _message: &mut Message) -> Result<(), CycBoxError> {
        Ok(())
    }
}

#[async_trait]
impl Manifestable for DisableTransformer {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: DISABLE_TRANSFORMER_ID.to_string(),
            name: l10n.get(locale, "data-transformer-disable"),
            description: l10n.get(locale, "data-transformer-disable-description"),
            category: PluginCategory::Transformer,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for DisableTransformer {}
