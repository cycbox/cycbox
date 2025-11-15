# CycBox

[中文版本](./README_CN.md)

⚠️ **Note: CycBox is a non-open-source project. This repository provides the open-source CycBox WebAssembly plugin development SDK and Lua script examples.**

[CycBox](https://cycbox.io/) is a powerful and highly extensible IoT debugging toolkit, featuring high performance, and cross-platform support.

## Main Features

- Integrated serial port and TCP network debugging with data pass-through between both
- Features 1ms high-precision timer with 1ms low-latency response
- Message processing and response through Lua scripting
- Supports WebAssembly plugins for complex custom functionality
- Data visualization panel with chart plotting, snapshot capture, and data export
- Search function supports keyword highlighting and filtering to show only matched messages
- Supports multiple standard protocols like AT commands and Modbus RTU, with custom frame structure configuration
- Built-in multiple checksum algorithms that automatically add checksum to outgoing data and verify incoming data
- Supports timed sending and command sequence configuration for one-click execution of complex debugging workflows
- Cross-platform support for Windows, Linux, Android (experimental feature)
- Provides MCP interface for AI Agent integration (experimental feature)

Official Website: https://cycbox.io

## Download and Installation

*   **Windows:** Install directly from the Microsoft Store: https://apps.microsoft.com/detail/9n9d7d1mv4sf
*   **Linux:** Depends on the GTK library. A deb package is provided with support for x64 and arm64 architectures: https://github.com/cycbox/cycbox/releases