use embassy_sync::{channel::Channel, signal::Signal};
use embassy_time::Duration;

use crate::state::command_handler::{COMMAND_CHANNEL, PENDING_REQUESTS};

use super::{CommandType, Packet, RequestId};

pub struct CommandDispatcher;

impl CommandDispatcher {
    async fn send_with_response<T, F>(
        mut packet: Packet,
        timeout: Duration,
        process_response: F,
    ) -> Option<T>
    where
        F: FnOnce(&Packet) -> Option<T>,
    {
        let channel = Channel::new();
        let sender = channel.sender();
        let receiver = channel.receiver();

        {
            let mut pending = PENDING_REQUESTS.lock().await;
            pending.requests.insert(packet.request_id, sender).ok()?;
        }

        COMMAND_CHANNEL.send(packet).await;

        let response = embassy_time::with_timeout(timeout, receiver.receive())
            .await
            .ok()?;

        process_response(&response)
    }

    pub async fn get_sensors() -> Option<[i16; 9]> {
        let mut packet = Packet::with_payload(CommandType::GetSensors, &[0xFF])?;
        packet.request_id = RequestId::new();

        Self::send_with_response(packet, Duration::from_millis(100), |response| {
            let payload = &response.payload[..response.length as usize];
            let mut sensors = [0i16; 9];

            for i in 0..9 {
                let bytes = [payload[i * 2], payload[i * 2 + 1]];
                sensors[i] = i16::from_le_bytes(bytes);
            }

            Some(sensors)
        })
        .await
    }
}
