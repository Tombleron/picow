use embassy_rp::{bind_interrupts, peripherals::UART0, uart};

bind_interrupts!(
    pub struct SerialInterrupts {
        UART0_IRQ => uart::InterruptHandler<UART0>;
    }
);
