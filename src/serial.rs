use embassy_rp::{
    bind_interrupts,
    peripherals::UART0,
    uart::{self, BufferedUart},
};
use static_cell::StaticCell;

use crate::UartResources;

static TX_BUF: StaticCell<[u8; 256]> = StaticCell::new();
static RX_BUF: StaticCell<[u8; 256]> = StaticCell::new();

bind_interrupts!(
    pub struct SerialInterrupts {
        UART0_IRQ => uart::BufferedInterruptHandler<UART0>;
    }
);

pub fn init_buffered_uart(uart: UartResources) -> BufferedUart<'static, UART0> {
    let tx_buf = &mut TX_BUF.init([0; 256])[..];
    let rx_buf = &mut RX_BUF.init([0; 256])[..];

    let config = uart::Config::default();
    BufferedUart::new(
        uart.uart,
        SerialInterrupts,
        uart.tx,
        uart.rx,
        tx_buf,
        rx_buf,
        config,
    )
}
