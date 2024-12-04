use embassy_futures::select::{select, Either};
use embassy_rp::uart::BufferedUartTx;
use embassy_rp::{peripherals::UART0, uart::BufferedUartRx};
use embassy_sync::channel::Sender;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, mutex::Mutex, signal::Signal,
};
use embedded_io_async::{Read, Write};
use heapless::FnvIndexMap;

use crate::commands::{Packet, RequestId};

pub static COMMAND_CHANNEL: Channel<CriticalSectionRawMutex, Packet, 10> = Channel::new();
pub static RESPONSE_CHANNEL: Channel<CriticalSectionRawMutex, Packet, 10> = Channel::new();

pub struct PendingRequests<'a> {
    pub requests: FnvIndexMap<RequestId, Sender<'a, CriticalSectionRawMutex, Packet, 1>, 16>,
}

impl<'a> PendingRequests<'a> {
    pub const fn new() -> Self {
        Self {
            requests: FnvIndexMap::new(),
        }
    }
}

pub static PENDING_REQUESTS: Mutex<CriticalSectionRawMutex, PendingRequests> =
    Mutex::new(PendingRequests::new());

pub struct CommandSender {
    uart: BufferedUartTx<'static, UART0>,
}

impl CommandSender {
    pub fn new(uart: BufferedUartTx<'static, UART0>) -> Self {
        Self { uart }
    }

    async fn send_request(&mut self, packet: Packet) {
        self.uart.write_all(&packet.serialize()).await.unwrap();
    }

    async fn handle_response(&mut self, packet: Packet) {
        let mut pending = PENDING_REQUESTS.lock().await;
        if let Some(sender) = pending.requests.remove(&packet.request_id()) {
            sender.send(packet).await
        }
    }
}

#[embassy_executor::task]
pub async fn command_handler_task(uart: BufferedUartTx<'static, UART0>) {
    let mut handler = CommandSender::new(uart);
    let command_receiver = COMMAND_CHANNEL.receiver();
    let response_receiver = RESPONSE_CHANNEL.receiver();

    loop {
        match select(command_receiver.receive(), response_receiver.receive()).await {
            Either::First(packet) => {
                handler.send_request(packet).await;
            }
            Either::Second(packet) => {
                handler.handle_response(packet).await;
            }
        }
    }
}

#[embassy_executor::task]
pub async fn response_reader_task(mut uart: BufferedUartRx<'static, UART0>) {
    let mut buffer = [0u8; 38];

    loop {
        if uart.read_exact(&mut buffer).await.is_ok() {
            if let Some(packet) = Packet::deserialize(&buffer) {
                RESPONSE_CHANNEL.send(packet).await;
            }
        }
    }
}
