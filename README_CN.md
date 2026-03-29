# CycBox

[CycBox](https://cycbox.io/) 是一款功能强大且高度可扩展的物联网调试工具包，具有高性能，跨平台，灵活扩展等优点。

本仓库包含 CycBox 的开源核心组件，采用 [MPL-2.0](./LICENSE) 协议。

https://github.com/user-attachments/assets/51087a43-ac56-45aa-8bcc-d1713417741e

## 主要功能

- 集成串口与网络调试功能，并支持两者间的数据透传
- 拥有 1ms 的高精度定时器与 1ms 的低响应延迟
- 可通过 Lua 脚本实现消息的处理与响应
- 数据可视化面板，支持图表绘制、快照截取与数据导出
- 搜索功能支持关键词高亮显示以及筛选只显示匹配的消息
- 支持AT指令、Modbus RTU等多种标准协议，并可自定义帧结构
- 内置多种校验算法，能自动为发送数据添加校验位，并对接收数据进行验证
- 支持定时发送与指令序列配置，可一键执行复杂的调试流程
- 跨平台，支持 Windows，Linux，Android（试验功能）
- 提供 MCP 接口，方便 AI Agent 接入调试（试验功能）

官方网站：https://cycbox.io

## 开源组件

| 组件 | 说明 |
|------|------|
| [cycbox-sdk](./crates/cycbox-sdk/) | 插件开发 SDK — 特征定义、消息类型、表单清单 |
| [cycbox-engine](./crates/cycbox-engine/) | 核心异步引擎 — 管道编排、传输管理、Lua 脚本 |
| [cycbox-runtime](./crates/cycbox-runtime/) | 编解码器（COBS、SLIP、行模式）、转换器（CSV、JSON）及 Lua 扩展 |
| [cycbox-serialport](./crates/cycbox-serialport/) | 跨平台串口驱动，支持 Tokio 异步 |
| [serialport-transport](./crates/serialport-transport/) | 串口传输层，支持清单驱动配置 |

## 下载和安装

- Windows: 从 Microsoft Store 直接安装 https://apps.microsoft.com/detail/9n9d7d1mv4sf
- Linux: 依赖 GTK库，提供 deb 安装包，支持 x64 与 arm64， https://github.com/cycbox/cycbox/releases

## 许可证

本项目采用 [Mozilla Public License 2.0](./LICENSE) 协议。

## 应用截图

![](/assets/zh01.png)

![](/assets/zh02.png)

![](/assets/zh03.png)

![](/assets/zh04.png)

![](/assets/zh05.png)

![](/assets/zh06.png)

![](/assets/zh07.png)

![](/assets/zh08.png)

![](/assets/zh09.png)

![](/assets/zh10.png)
