use std::{fs::read_to_string, net::Ipv4Addr, path::Path};

use serde::Deserialize;
use tokio::net::UdpSocket;

use coe::{COEValue, Packet, Payload};

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    PdoZero,
}
impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        Self::Toml(value)
    }
}
impl From<PdoZeroError> for ConfigError {
    fn from(_: PdoZeroError) -> Self {
        Self::PdoZero
    }
}
impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Io(x) => {
                write!(f, "Error reading from file /etc/ta-agi-doorbell/config.toml: {x}")
            }
            Self::Toml(x) => {
                write!(f, "Error parsing config file as toml: {x}")
            }
            Self::PdoZero => {
                write!(f, "One of the PDO indices is zero. They need to be one-based.")
            }
        }
    }
}
impl std::error::Error for ConfigError {}

#[derive(Debug)]
pub enum DoorOpenError {
    CannotBindSocket,
    CannotSendCoe(std::io::Error),
}
impl From<std::io::Error> for DoorOpenError {
    fn from(value: std::io::Error) -> Self {
        Self::CannotSendCoe(value)
    }
}
impl core::fmt::Display for DoorOpenError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::CannotBindSocket => {
                write!(f, "Cannot bind to a udp socket to send packets from")
            }
            Self::CannotSendCoe(x) => {
                write!(f, "Cannot send the complete coe packet: {x}")
            }
        }
    }
}
impl std::error::Error for DoorOpenError {}

/// The entire configuration for ta-agi-doorbell
#[derive(Debug)]
pub struct Config {
    agi: AgiConfig,
    pub cmi: CmiConfig,
}
impl TryFrom<ConfigData> for Config {
    type Error = PdoZeroError;

    fn try_from(value: ConfigData) -> Result<Self, Self::Error> {
        Ok(Self {
            agi: value.agi.into(),
            cmi: value.cmi.try_into()?,
        })
    }
}
impl Config {
    pub fn create() -> Result<Self, ConfigError> {
        Ok(ConfigData::create()?.try_into()?)
    }

    pub fn agi_listen_string(&self) -> String {
        format!("{}:{}", self.agi.listen_address, self.agi.listen_port)
    }

    pub fn agi_digest_secret(&self) -> String {
        self.agi.digest_secret.to_owned()
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct ConfigData {
    agi: AgiConfigData,
    cmi: CmiConfigData,
}
impl ConfigData {
    fn create() -> Result<Self, ConfigError> {
        let path = Path::new("/etc/ta-agi-doorbell/config.toml");
        let content = read_to_string(path)?;
        Ok(toml::de::from_str(&content)?)
    }
}

/// Variables for listening to AGI requests
#[derive(Debug, PartialEq, Eq, Deserialize)]
struct AgiConfigData {
    listen_address: Ipv4Addr,
    listen_port: Option<u16>,
    digest_secret: String,
}
#[derive(Debug)]
pub struct AgiConfig {
    pub listen_address: Ipv4Addr,
    pub listen_port: u16,
    pub digest_secret: String,
}
impl From<AgiConfigData> for AgiConfig {
    fn from(value: AgiConfigData) -> Self {
        Self {
            listen_address: value.listen_address,
            listen_port: value.listen_port.unwrap_or(4573),
            digest_secret: value.digest_secret,
        }
    }
}

/// Mappings for all doors
#[derive(Debug, PartialEq, Eq)]
pub struct CmiConfig {
    door_mappings: Vec<DoorMapping>,
}
impl TryFrom<CmiConfigData> for CmiConfig {
    type Error = PdoZeroError;

    fn try_from(value: CmiConfigData) -> Result<Self, Self::Error> {
        Ok(Self {
            door_mappings: value.door_mappings.into_iter().map(|x| <DoorMappingData as TryInto<DoorMapping>>::try_into(x)).collect::<Result<Vec<_>, _>>()?,
        })
    }
}
impl CmiConfig {
    pub fn get_cmi_for_door(&self, name: &str) -> Option<&DoorMapping> {
        for map in &self.door_mappings {
            if map.door_name == name {
                return Some(&map);
            }
        };
        None
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct CmiConfigData {
    door_mappings: Vec<DoorMappingData>,
}

/// Mapping a single door to a destination in TA
#[derive(Debug, PartialEq, Eq)]
pub struct DoorMapping {
    pub door_name: String,
    cmi_address: Ipv4Addr,
    cmi_port: u16,
    virtual_node: u8,
    pdo: u8,
}
impl DoorMapping {
    pub async fn open_door(&self) -> Result<(), DoorOpenError> {
        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|_| DoorOpenError::CannotBindSocket)?;
        let value = COEValue::Digital(coe::DigitalCOEValue::OnOff(true));
        let payload = Payload::new(self.virtual_node, self.pdo, value);
        let packet = Packet::try_from_payloads(&[payload]).expect("known good sequence");
        let mut buf = [0_u8; 12];
        packet.try_serialize_into(&mut buf).expect("known packet length");
        socket.send_to(&buf, format!("{}:{}", self.cmi_address, self.cmi_port)).await?;
        Ok(())
    }
}


#[derive(Debug)]
struct PdoZeroError {}
impl core::fmt::Display for PdoZeroError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "PDO is zero, but has to be entered one-based.")
    }
}
impl std::error::Error for PdoZeroError {}

impl TryFrom<DoorMappingData> for DoorMapping {
    type Error = PdoZeroError;

    fn try_from(value: DoorMappingData) -> Result<Self, Self::Error> {
        Ok(Self {
            door_name: value.door_name,
            cmi_address: value.cmi_address,
            cmi_port: value.cmi_port.unwrap_or(5422),
            virtual_node: value.virtual_node,
            pdo: value.pdo.checked_sub(1).ok_or(PdoZeroError {})?,
        })
    }
}

/// Data for [DoorMapping] on disk.
#[derive(Debug, PartialEq, Eq, Deserialize)]
struct DoorMappingData {
    door_name: String,
    cmi_address: Ipv4Addr,
    cmi_port: Option<u16>,
    virtual_node: u8,
    pdo: u8,
}
