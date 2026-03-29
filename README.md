# CycBox

[中文版本](./README_CN.md)

[CycBox](https://cycbox.io/) is a powerful and highly extensible IoT debugging toolkit, featuring high performance and
cross-platform support.

This repository contains the open-source core of CycBox, licensed under [MPL-2.0](./LICENSE).

https://github.com/user-attachments/assets/51087a43-ac56-45aa-8bcc-d1713417741e

## Main Features

- Integrated serial port and network debugging with data pass-through between both
- Features 1ms high-precision timer with 1ms low-latency response
- Message processing and response through Lua scripting
- Data visualization panel with chart plotting, snapshot capture, and data export
- Search function supports keyword highlighting and filtering to show only matched messages
- Supports multiple standard protocols like AT commands and Modbus RTU, with custom frame structure configuration
- Built-in multiple checksum algorithms that automatically add checksum to outgoing data and verify incoming data
- Supports timed sending and command sequence configuration for one-click execution of complex debugging workflows
- Cross-platform support for Windows, Linux, Android (experimental feature)
- Provides MCP interface for AI Agent integration (experimental feature)

Official Website: https://cycbox.io

## Open Source Crates

| Crate                                                  | Description                                                                     |
|--------------------------------------------------------|---------------------------------------------------------------------------------|
| [cycbox-sdk](./crates/cycbox-sdk/)                     | Plugin development SDK — traits, message types, manifest definitions            |
| [cycbox-engine](./crates/cycbox-engine/)               | Core async engine — pipeline orchestration, transport management, Lua scripting |
| [cycbox-runtime](./crates/cycbox-runtime/)             | Codecs (COBS, SLIP, line), transformers (CSV, JSON), and Lua extensions         |
| [cycbox-serialport](./crates/cycbox-serialport/)       | Cross-platform serial port driver with async Tokio support                      |
| [serialport-transport](./crates/serialport-transport/) | Serial port transport layer with manifest-driven configuration                  |

## Download and Installation

* **Windows:** Install directly from the Microsoft Store: https://apps.microsoft.com/detail/9n9d7d1mv4sf
* **Linux:** Depends on the GTK library. A deb package is provided with support for x64 and arm64
  architectures: https://github.com/cycbox/cycbox/releases

## License

This project is licensed under the [Mozilla Public License 2.0](./LICENSE).

## Screenshots

![](/assets/en01.png)

![](/assets/en02.png)

![](/assets/en03.png)

![](/assets/en04.png)

![](/assets/en05.png)

![](/assets/en06.png)

![](/assets/en07.png)

![](/assets/en08.png)

![](/assets/en09.png)

![](/assets/en10.png)
