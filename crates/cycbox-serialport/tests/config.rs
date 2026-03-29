// Configuration for integration tests. This crate is about interacting with real serial ports and
// so some tests need actual hardware.

use envconfig::Envconfig;
use rstest::fixture;
use std::time::Duration;

// Test constants
pub const TEST_MESSAGE: &[u8] = b"Hello async serial!";
pub const TEST_TIMEOUT: Duration = Duration::from_millis(1000);

// Configuration for tests requiring acutual hardware.
//
// For conveniently pulling this configuration into a test case as a parameter, you might want to
// use the test fixture [`hw_config`].
#[derive(Clone, Debug, Envconfig, Eq, PartialEq)]
pub struct HardwareConfig {
    #[envconfig(from = "SERIALPORT_TEST_PORT")]
    pub port: String,
}

// Test fixture for conveniently pulling the actual hardware configuration into test cases.
//
// See [`fixture`](rstest::fixture) for details.
#[fixture]
pub fn hw_config() -> HardwareConfig {
    HardwareConfig::init_from_env().unwrap()
}

