# CycBox Lua Configuration Examples

This directory contains example Lua configuration files that demonstrate how to extend CycBox functionality with custom
data processing, automatic polling, and device-specific parsers.

## What are Lua Config Files?

Lua config files combine two features:

1. **Lua Script**: Custom code for message processing, automatic polling, and data extraction
2. **Device Configuration**: Transport settings, codec configuration, and message templates

This allows you to create **reusable, portable configurations** that can be shared and loaded into CycBox with a single
file.

## File Format

Lua config files use a special format:

```lua
-- Your Lua script code here
function on_receive()
    -- Process incoming messages
end

--[[
id: "my_device"
version: "1.0.0"
name: "My Device"
configs:
  - # Config 0
    app:
      app_transport: serial
      app_codec: frame_codec
    serial:
      serial_port: /dev/ttyUSB0
      serial_baud_rate: 115200
]]
```

**Structure:**

- **Lua Script** (top): Custom processing logic
- **YAML Configuration** (in `--[[ ]]` block comment): Device and connection settings

## How to Use Lua Config Files

### Load from File

1. Open CycBox application
2. Go to the **Options** tab
3. Click the **Load Config** button
4. Select a `.lua` file
5. The configuration will be automatically applied

### Export Your Current Configuration

1. Configure your device settings in the **Settings** tab
2. Write custom Lua code in the **Script** tab
3. Click the **Export as Lua** button in the **Terminal** tab to save as a `.lua` file
4. Share this file with others or use it as a template

