use crate::codec::CbrtCodec;
use async_trait::async_trait;
use cycbox_sdk::prelude::*;

#[async_trait]
impl Configurable for CbrtCodec {}
