use assign_resources::assign_resources;
use embassy_rp::peripherals;

assign_resources! {
    blt: BltResources {
        pwr: PIN_23,
        cs: PIN_25,
        pio: PIO0,
        dio: PIN_24,
        clk: PIN_29,
        dma: DMA_CH0,
    }
    uart: UartResources {
        uart: UART0,
        tx: PIN_0,
        rx: PIN_1,
        dma_0: DMA_CH1,
        dma_1: DMA_CH2,
    }
    adc: AdcResources {
        adc: ADC
    }
}
