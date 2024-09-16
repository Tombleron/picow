use assign_resources::assign_resources;
use embassy_rp::peripherals;

assign_resources! {
    wifi: WiFi{
        pwr: PIN_23,
        cs: PIN_25,
        pio: PIO0,
        dio: PIN_24,
        clk: PIN_29,
        dma: DMA_CH0,
    }
}
