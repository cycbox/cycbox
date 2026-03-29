# Application
app-name = CycBox
app-description = Professional IoT Debugging Toolkit
app-basic-config = Basic Configuration

# Engine-specific keys (SDK keys are loaded via add_provider)
app-transport-config = Transport Configuration
app-codec-config = Codec Configuration
app-transformer-config = Transformer Configuration
app-encoding-config = Encoding Configuration


cobs-codec = COBS Codec
cobs-codec-description = Consistent Overhead Byte Stuffing - eliminates a sentinel byte from data stream for framing

slip-codec = SLIP Codec
slip-codec-description = Serial Line Internet Protocol - simple framing protocol using special delimiter bytes (END=0xC0, ESC=0xDB) for packet separation
slip-push-leading-end-label = Push Leading END
slip-push-leading-end-description = Send an END byte before the frame to flush accumulated noise (recommended for noisy connections)

passthrough-codec = Passthrough
passthrough-codec-description = Pass buffered data as messages immediately

line-codec = Line Codec
codec-line-description = Decode based on line separator (LF or CRLF)
line-codec-end-label = Line Ending
line-codec-packet-end-crlf = CRLF (\r\n)
line-codec-packet-end-lf = LF (\n)

timeout-codec = Timeout Codec
timeout-codec-description = Decode based on timeout (treat buffered data as a frame after timeout)
timeout-codec-timeout-label = Timeout (ms)
timeout-codec-timeout-description = If there is no new data after this time, the buffered data will be treated as a complete frame

data-transformer-disable = Disable
data-transformer-disable-description = No data transformation applied

csv-transformer-name = CSV Transformer
csv-transformer-description = Parses space/tab/comma-separated values with auto type detection (Int64, Float64, Boolean, String). Values are named csv_0, csv_1, etc.

json-transformer-name = JSON Transformer
json-transformer-description = Parses JSON key-value pairs.
