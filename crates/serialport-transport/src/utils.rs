use cycbox_serialport::{SerialPortType, available_ports};
use std::sync::Mutex;

/// Serial port information
#[derive(Debug, Clone)]
pub struct SerialPortInfo {
    /// Port device name (e.g., "/dev/ttyUSB0", "COM1")
    pub port_name: String,
    /// Human-readable label (includes port type info)
    pub display_label: String,
}

// Cache for serial port list with 3-second TTL
struct SerialPortCache {
    ports: Vec<SerialPortInfo>,
    timestamp: std::time::Instant,
}

static SERIAL_PORT_CACHE: Mutex<Option<SerialPortCache>> = Mutex::new(None);
const CACHE_TTL_SECS: u64 = 3;

pub fn get_available_serial_ports() -> Vec<SerialPortInfo> {
    // Check cache first
    {
        let cache = SERIAL_PORT_CACHE.lock().unwrap();
        if let Some(cached) = cache.as_ref() {
            let elapsed = cached.timestamp.elapsed();
            if elapsed.as_secs() < CACHE_TTL_SECS {
                return cached.ports.clone();
            }
        }
    }

    // Cache miss or expired, fetch fresh data
    let mut port_infos = Vec::new();

    match available_ports() {
        Ok(mut ports) => {
            // Sort ports by name for consistent ordering
            ports.sort_by(|a, b| a.port_name.cmp(&b.port_name));

            // Filter out virtual consoles and system terminals
            // Keep: ttyS* (hardware serial), ttyUSB*, ttyACM*, ttyAMA* (RPi), etc.
            #[cfg(unix)]
            ports.retain(|port| {
                let name = &port.port_name;

                // Exclude /dev/tty (current terminal) and /dev/console
                if name == "/dev/tty" || name == "/dev/console" {
                    return false;
                }

                // Exclude /dev/tty[0-9]+ (virtual consoles like /dev/tty0, /dev/tty1)
                // but keep /dev/ttyS*, /dev/ttyUSB*, /dev/ttyACM*, /dev/ttyAMA*, etc.
                if name.starts_with("/dev/tty") {
                    // Check if character after "/dev/tty" is a digit
                    if let Some(ch) = name.chars().nth(8) {
                        // "/dev/tty" is 8 chars
                        if ch.is_ascii_digit() {
                            return false;
                        }
                    }
                }

                true
            });

            for port in ports {
                // Add port type info to label if available
                let display_label = match &port.port_type {
                    SerialPortType::UsbPort(usb_info) => {
                        format!(
                            "{} (USB: {})",
                            port.port_name,
                            usb_info.product.as_ref().unwrap_or(&"Unknown".to_string())
                        )
                    }
                    SerialPortType::PciPort => {
                        format!("{} (PCI)", port.port_name)
                    }
                    SerialPortType::BluetoothPort => {
                        format!("{} (Bluetooth)", port.port_name)
                    }
                    SerialPortType::Unknown => port.port_name.clone(),
                };

                port_infos.push(SerialPortInfo {
                    port_name: port.port_name,
                    display_label,
                });
            }
        }
        Err(_) => {
            // Fallback to common port names if detection fails
            #[cfg(unix)]
            let fallback_ports = vec![
                "/dev/ttyACM0",
                "/dev/ttyACM1",
                "/dev/ttyUSB0",
                "/dev/ttyUSB1",
                "/dev/ttyS0",
                "/dev/ttyS1",
            ];

            #[cfg(windows)]
            let fallback_ports = vec!["COM1", "COM2", "COM3", "COM4", "COM5", "COM6"];

            for port in fallback_ports {
                port_infos.push(SerialPortInfo {
                    port_name: port.to_string(),
                    display_label: port.to_string(),
                });
            }
        }
    }

    // If no ports found, add a default option
    if port_infos.is_empty() {
        #[cfg(unix)]
        let default_port = "/dev/ttyACM0";
        #[cfg(windows)]
        let default_port = "COM1";

        port_infos.push(SerialPortInfo {
            port_name: default_port.to_string(),
            display_label: default_port.to_string(),
        });
    }

    // Update cache
    {
        let mut cache = SERIAL_PORT_CACHE.lock().unwrap();
        *cache = Some(SerialPortCache {
            ports: port_infos.clone(),
            timestamp: std::time::Instant::now(),
        });
    }

    port_infos
}

pub(crate) fn get_default_port(available_ports: &[SerialPortInfo]) -> Option<String> {
    if available_ports.is_empty() {
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        // Priority order for Linux: ttyUSB* (USB-serial adapters), ttyACM* (Arduino/CDC), then others

        // First, try to find ttyUSB* devices (most common USB-serial adapters)
        if let Some(port) = available_ports
            .iter()
            .find(|p| p.port_name.contains("ttyUSB"))
        {
            return Some(port.port_name.clone());
        }

        // Second, try ttyACM* devices (Arduino and other CDC ACM devices)
        if let Some(port) = available_ports
            .iter()
            .find(|p| p.port_name.contains("ttyACM"))
        {
            return Some(port.port_name.clone());
        }

        // Third, try ttyAMA* (Raspberry Pi hardware UART)
        if let Some(port) = available_ports
            .iter()
            .find(|p| p.port_name.contains("ttyAMA"))
        {
            return Some(port.port_name.clone());
        }

        // Finally, fall back to any other port
        Some(available_ports[0].port_name.clone())
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, prioritize lower-numbered COM ports (COM1, COM2, etc.)
        let mut ports_with_numbers: Vec<(String, u32)> = available_ports
            .iter()
            .filter_map(|port_info| {
                let port_name = &port_info.port_name;
                // Extract number from "COM1", "COM2", etc.
                if port_name.starts_with("COM") {
                    let num_str = &port_name[3..];
                    if let Ok(num) = num_str.parse::<u32>() {
                        return Some((port_name.clone(), num));
                    }
                }
                None
            })
            .collect();

        if !ports_with_numbers.is_empty() {
            // Sort by COM port number (ascending)
            ports_with_numbers.sort_by_key(|(_, num)| *num);
            return Some(ports_with_numbers[0].0.clone());
        }

        // Fallback to first available port if no COM ports found
        Some(available_ports[0].port_name.clone())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        // For other platforms (macOS, BSD, etc.), just return the first port
        Some(available_ports[0].port_name.clone())
    }
}
