use defmt::info;
use embassy_futures::select::{select, Either};
use embassy_rp::uart::BufferedUartTx;
use embassy_rp::{peripherals::UART0, uart::BufferedUartRx};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embedded_io_async::{Read, Write};

use crate::commands::Packet;

pub static COMMAND_CHANNEL: Channel<CriticalSectionRawMutex, Packet, 10> = Channel::new();
pub static RESPONSE_CHANNEL: Channel<CriticalSectionRawMutex, Packet, 10> = Channel::new();

pub struct CommandSender {
    uart: BufferedUartTx<'static, UART0>,
}

impl CommandSender {
    pub fn new(uart: BufferedUartTx<'static, UART0>) -> Self {
        Self { uart }
    }

    async fn send_request(&mut self, packet: Packet) {
        info!("Sending request: {}", packet);
        let serialized = packet.serialize();
        info!("Serialized: {}", serialized);
        self.uart.write_all(&serialized).await.unwrap();
    }

    async fn handle_response(&mut self, packet: Packet) {
        info!("Handling response: {:?}", packet);
        match packet.command {
            _ => {}
        }
    }
}

#[embassy_executor::task]
pub async fn command_handler_task(uart: BufferedUartTx<'static, UART0>) {
    info!("Starting command handler task");
    let mut handler = CommandSender::new(uart);
    let command_receiver = COMMAND_CHANNEL.receiver();
    let response_receiver = RESPONSE_CHANNEL.receiver();

    loop {
        match select(command_receiver.receive(), response_receiver.receive()).await {
            Either::First(packet) => {
                info!("Received command packet");
                handler.send_request(packet).await;
            }
            Either::Second(packet) => {
                info!("Received response packet");
                handler.handle_response(packet).await;
            }
        }
    }
}

#[embassy_executor::task]
pub async fn response_reader_task(mut uart: BufferedUartRx<'static, UART0>) {
    info!("Starting response reader task");
    let mut buffer = [0u8; 38];

    loop {
        if uart.read_exact(&mut buffer).await.is_ok() {
            if let Some(packet) = Packet::deserialize(&buffer) {
                info!("Successfully parsed response packet");
                RESPONSE_CHANNEL.send(packet).await;
            } else {
                info!("Failed to parse response packet");
            }
        } else {
            info!("Failed to read from UART");
        }
    }
}
