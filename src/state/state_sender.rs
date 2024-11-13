use core::fmt::Write;
use embassy_rp::{
    peripherals::UART0,
    uart::{Async as UartAsync, Uart},
};
use embassy_time::{Duration, Ticker};

use super::ApplicationState;

pub struct StateSender {
    period: Duration,
    uart: Uart<'static, UART0, UartAsync>,
}

impl StateSender {
    pub fn new(period: Duration, uart: Uart<'static, UART0, UartAsync>) -> Self {
        Self { period, uart }
    }

    pub async fn send_state(&mut self) {
        let state = ApplicationState::gather().await;

        let mut data = heapless::String::<100>::new();
        write!(data, "[{}]", state).unwrap();

        self.uart.write(data.as_bytes()).await.unwrap();
        self.uart.write("\n".as_bytes()).await.unwrap();
    }
}

#[embassy_executor::task]
pub async fn state_sender_task(period: Duration, uart: Uart<'static, UART0, UartAsync>) {
    let mut state_sender = StateSender::new(period, uart);

    let mut ticker = Ticker::every(state_sender.period);

    loop {
        ticker.next().await;

        state_sender.send_state().await;
    }
}
