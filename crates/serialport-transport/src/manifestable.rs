use crate::SERIAL_PORT_TRANSPORT_ID;
use crate::utils::{get_available_serial_ports, get_default_port};
use cycbox_sdk::prelude::*;

pub(crate) fn serial_manifest(locale: &str) -> Manifest {
    let l10n = crate::l10n::get_l10n();
    let available_serial_ports = get_available_serial_ports();

    // Convert SerialPortInfo to FormFieldOption
    let port_options: Vec<FormFieldOption> = available_serial_ports
        .iter()
        .map(|port_info| {
            FormFieldOption::new(
                port_info.display_label.clone(),
                FormValue::Text(port_info.port_name.clone()),
            )
        })
        .collect();

    Manifest {
        id: SERIAL_PORT_TRANSPORT_ID.to_string(),
        name: l10n.get(locale, "transport-serial"),
        description: l10n.get(locale, "transport-serial-description"),
        category: PluginCategory::Transport,
        config_schema: vec![FormGroup {
            key: SERIAL_PORT_TRANSPORT_ID.to_string(),
            label: l10n.get(locale, "transport-serial"),
            description: Some(serial_config_description(locale)),
            fields: vec![
                // Hidden field to indicate this transport requires codec
                FormField {
                    key: format!("{}_requires_codec", SERIAL_PORT_TRANSPORT_ID),
                    field_type: FieldType::BooleanInput,
                    label: "Requires Codec".to_string(),
                    description: None,
                    values: Some(vec![FormValue::Boolean(true)]),
                    options: None,
                    is_required: false,
                    condition: Some(FormCondition {
                        field_key: "__hidden__".to_string(),
                        operator: ConditionOperator::Equal,
                        value: FormValue::Boolean(true),
                    }),
                    span: 6,
                },
                FormField {
                    key: format!("{}_port", SERIAL_PORT_TRANSPORT_ID),
                    field_type: FieldType::TextInputDropdown,
                    label: l10n.get(locale, "serial-port-label"),
                    description: None,
                    values: {
                        let default_port = get_default_port(&available_serial_ports);
                        if let Some(default) = default_port {
                            Some(vec![FormValue::Text(default)])
                        } else {
                            #[cfg(unix)]
                            let default = FormValue::Text("/dev/ttyACM0".to_string());
                            #[cfg(windows)]
                            let default = FormValue::Text("COM1".to_string());
                            Some(vec![default])
                        }
                    },
                    options: Some(port_options),
                    is_required: true,
                    condition: None,
                    span: 8,
                },
                FormField {
                    key: format!("{}_baud_rate", SERIAL_PORT_TRANSPORT_ID),
                    field_type: FieldType::IntegerInputDropdown,
                    label: l10n.get(locale, "serial-baudrate-label"),
                    description: None,
                    values: Some(vec![FormValue::Integer(9600)]),
                    options: Some(vec![
                        FormFieldOption::new("1200".to_string(), FormValue::Integer(1200)),
                        FormFieldOption::new("2400".to_string(), FormValue::Integer(2400)),
                        FormFieldOption::new("4800".to_string(), FormValue::Integer(4800)),
                        FormFieldOption::new("9600".to_string(), FormValue::Integer(9600)),
                        FormFieldOption::new("19200".to_string(), FormValue::Integer(19200)),
                        FormFieldOption::new("38400".to_string(), FormValue::Integer(38400)),
                        FormFieldOption::new("57600".to_string(), FormValue::Integer(57600)),
                        FormFieldOption::new("115200".to_string(), FormValue::Integer(115200)),
                        FormFieldOption::new("230400".to_string(), FormValue::Integer(230400)),
                        FormFieldOption::new("460800".to_string(), FormValue::Integer(460800)),
                    ]),
                    is_required: true,
                    condition: None,
                    span: 4,
                },
                FormField {
                    key: format!("{}_data_bits", SERIAL_PORT_TRANSPORT_ID),
                    field_type: FieldType::IntegerDropdown,
                    label: l10n.get(locale, "serial-databits-label"),
                    description: None,
                    values: Some(vec![FormValue::Integer(8)]),
                    options: Some(vec![
                        FormFieldOption::new("5".to_string(), FormValue::Integer(5)),
                        FormFieldOption::new("6".to_string(), FormValue::Integer(6)),
                        FormFieldOption::new("7".to_string(), FormValue::Integer(7)),
                        FormFieldOption::new("8".to_string(), FormValue::Integer(8)),
                    ]),
                    is_required: true,
                    condition: None,
                    span: 3,
                },
                FormField {
                    key: format!("{}_parity", SERIAL_PORT_TRANSPORT_ID),
                    field_type: FieldType::TextDropdown,
                    label: l10n.get(locale, "serial-parity-label"),
                    description: None,
                    values: Some(vec![FormValue::Text("none".to_string())]),
                    options: Some(vec![
                        FormFieldOption::new(
                            l10n.get(locale, "serial-parity-none"),
                            FormValue::Text("none".to_string()),
                        ),
                        FormFieldOption::new(
                            l10n.get(locale, "serial-parity-even"),
                            FormValue::Text("even".to_string()),
                        ),
                        FormFieldOption::new(
                            l10n.get(locale, "serial-parity-odd"),
                            FormValue::Text("odd".to_string()),
                        ),
                    ]),
                    is_required: true,
                    condition: None,
                    span: 3,
                },
                FormField {
                    key: format!("{}_stop_bits", SERIAL_PORT_TRANSPORT_ID),
                    field_type: FieldType::TextDropdown,
                    label: l10n.get(locale, "serial-stopbits-label"),
                    description: None,
                    values: Some(vec![FormValue::Text("1".to_string())]),
                    options: Some(vec![
                        FormFieldOption::new(
                            l10n.get(locale, "serial-stopbits-one"),
                            FormValue::Text("1".to_string()),
                        ),
                        FormFieldOption::new(
                            l10n.get(locale, "serial-stopbits-two"),
                            FormValue::Text("2".to_string()),
                        ),
                    ]),
                    is_required: true,
                    condition: None,
                    span: 3,
                },
                FormField {
                    key: format!("{}_flow_control", SERIAL_PORT_TRANSPORT_ID),
                    field_type: FieldType::TextDropdown,
                    label: l10n.get(locale, "serial-flowcontrol-label"),
                    description: None,
                    values: Some(vec![FormValue::Text("none".to_string())]),
                    options: Some(vec![
                        FormFieldOption::new(
                            l10n.get(locale, "serial-flowcontrol-none"),
                            FormValue::Text("none".to_string()),
                        ),
                        FormFieldOption::new(
                            l10n.get(locale, "serial-flowcontrol-software"),
                            FormValue::Text("software".to_string()),
                        ),
                        FormFieldOption::new(
                            l10n.get(locale, "serial-flowcontrol-hardware"),
                            FormValue::Text("hardware".to_string()),
                        ),
                    ]),
                    is_required: true,
                    condition: None,
                    span: 3,
                },
            ],
            condition: None,
        }],
        ..Default::default()
    }
}

fn serial_config_description(locale: &str) -> String {
    if locale.starts_with("zh") {
        r#"
**基本参数**:
- **端口**: 选择要连接的串口设备 (如 /dev/ttyUSB0, COM1 等)
- **波特率**: 通信速度，必须与设备端设置一致 (常用: 9600, 115200)
- **数据位**: 每个字符包含的数据位数 (通常为 8 位)
- **校验位**: 错误检测方式 (无、奇校验、偶校验)
- **停止位**: 每个字符后的停止位数量 (通常为 1 位)
- **流控制**: 数据流控制方式 (无、软件流控、硬件流控)

"#
        .to_string()
    } else {
        r#"
**Basic Parameters**:
- **Port**: Select the serial device to connect to (e.g., /dev/ttyUSB0, COM1)
- **Baud Rate**: Communication speed, must match the device setting (common: 9600, 115200)
- **Data Bits**: Number of data bits per character (typically 8 bits)
- **Parity**: Error detection method (None, Odd, Even)
- **Stop Bits**: Number of stop bits after each character (typically 1 bit)
- **Flow Control**: Data flow control method (None, Software, Hardware)

"#
        .to_string()
    }
}
