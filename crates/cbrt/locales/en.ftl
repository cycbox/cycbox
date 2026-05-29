cbrt-codec = CBRT Realtime Codec
cbrt-codec-description = Decode-only codec for the CBRT (CycBox Realtime) streaming sensor protocol. Parses multi-channel, optionally timestamped sample frames. TX is raw passthrough — bytes are sent as-is, with no header added.
cbrt-transformer = CBRT Realtime Transformer
cbrt-transformer-description = Parse a single CBRT frame from message payloads on message-based transports (UDP, MQTT, …). Mirrors the codec's values/metadata/contents. Skips parsing if the codec already ran on the same pipeline, so it's safe to enable both.
