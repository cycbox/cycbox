# Application
app-name = CycBox
app-description = 专业物联网调试工具
app-basic-config = 基础配置

# 引擎专用键（SDK键通过 add_provider 加载）
app-transport-config = 传输层配置
app-codec-config = 编解码配置
app-transformer-config = 数据转换配置
app-encoding-config = 编码配置


cobs-codec = COBS编解码
cobs-codec-description = 一致性开销字节填充 - 从数据流中消除哨兵字节用于帧定界

slip-codec = SLIP编解码
slip-codec-description = 串行线路互联网协议 - 使用特殊定界字节(END=0xC0, ESC=0xDB)的简单成帧协议
slip-push-leading-end-label = 推送前导END字节
slip-push-leading-end-description = 在帧之前发送END字节以清除累积的噪声(建议在噪声较大的连接上使用)

passthrough-codec = 透传
passthrough-codec-description = 立即将缓冲数据作为消息传递

line-codec = 行编解码
codec-line-description = 基于换行符（LF或CRLF）
line-codec-end-label = 换行符
line-codec-packet-end-crlf = CRLF (\r\n)
line-codec-packet-end-lf = LF (\n)

timeout-codec = 超时编解码
timeout-codec-description = 超时后将缓冲区数据视为一帧
timeout-codec-timeout-label = 超时时间（ms）
timeout-codec-timeout-description = 如果超过此时间无新数据，则缓冲区数据将被视为完整的一帧

data-transformer-disable = 禁用
data-transformer-disable-description = 不应用数据转换

csv-transformer-name = CSV转换器
csv-transformer-description = 解析空格/制表符/逗号分隔的值，自动检测类型（Int64、Float64、Boolean、String）。值命名为 csv_0, csv_1 等。

json-transformer-name = JSON转换器
json-transformer-description = 解析JSON键值对。
