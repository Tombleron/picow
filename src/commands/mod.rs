pub mod command_dispatcher;
mod define_command;
use core::sync::atomic::Ordering;

use define_command::define_commands;
use portable_atomic::AtomicU16;

define_commands! {
    /// Sets minimum and maximum positions for each finger joint
    /// Payload: 24 bytes (2 bytes per axis)
    SetPosition = 0x01, 24,

    /// Retrieves sensor data from all sensors
    /// Payload: 18 bytes (2 bytes per sensor value)
    /// Contains: 3 pressure sensors, 6 position sensors
    GetSensors = 0x02, 18,

    /// Start motion (opening/closing)
    /// Payload: 1 byte (0 for close, 1 for open)
    StartMotion = 0x03, 1,

    /// Stop current motion
    /// Payload: None
    StopMotion = 0x04, 0,

    /// Set motion speed
    /// Payload: 2 bytes (speed value 0-65535)
    SetSpeed = 0x05, 2,

    /// Service and Configuration Commands
    GetDeviceInfo = 0x10, 0,
    EmergencyStop = 0x16, 0,
}

#[derive(Debug)]
pub struct Packet {
    command: CommandType,
    request_id: RequestId,
    length: u8,
    payload: [u8; 32],
    crc: u16,
}

impl Packet {
    pub fn new(command: CommandType) -> Self {
        Self {
            command,
            request_id: RequestId::new(),
            length: 0,
            payload: [0; 32],
            crc: 0,
        }
    }

    pub fn with_payload(command: CommandType, payload: &[u8]) -> Option<Self> {
        if payload.len() > command.max_payload_size() as usize {
            return None;
        }

        let mut packet = Self::new(command);
        packet.length = payload.len() as u8;
        packet.payload[..payload.len()].copy_from_slice(payload);
        packet.crc = packet.calculate_crc();
        Some(packet)
    }

    pub fn request_id(&self) -> RequestId {
        self.request_id
    }

    pub fn serialize(&self) -> [u8; 38] {
        let mut buffer = [0u8; 38];
        buffer[0] = self.command as u8;
        buffer[1] = self.length;
        buffer[2..4].copy_from_slice(&self.request_id.0.to_le_bytes());
        buffer[4..36].copy_from_slice(&self.payload);
        buffer[36..].copy_from_slice(&self.crc.to_le_bytes());
        buffer
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() != 38 {
            return None;
        }

        let command = CommandType::try_from(data[0]).ok()?;
        let length = data[1];
        if length > command.max_payload_size() {
            return None;
        }

        let request_id = RequestId(u16::from_le_bytes([data[2], data[3]]));

        let mut packet = Self::new(command);
        packet.request_id = request_id;
        packet.length = length;
        packet.payload.copy_from_slice(&data[4..36]);

        let received_crc = u16::from_le_bytes([data[36], data[37]]);
        packet.crc = packet.calculate_crc();

        if packet.crc != received_crc {
            return None;
        }

        Some(packet)
    }

    fn calculate_crc(&self) -> u16 {
        let mut data = [0u8; 36];
        data[0] = self.command as u8;
        data[1] = self.length;
        data[2..4].copy_from_slice(&self.request_id.0.to_le_bytes());
        data[4..].copy_from_slice(&self.payload);

        crc16::State::<crc16::CCITT_FALSE>::calculate(&data[..self.length as usize + 4])
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct RequestId(pub u16);

impl RequestId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU16 = AtomicU16::new(0);
        RequestId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
