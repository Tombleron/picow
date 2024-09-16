use cyw43::{Control, NetDriver};
use cyw43_pio::PioSpi;
use defmt::unwrap;
use embassy_executor::Spawner;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    peripherals::{DMA_CH0, PIO0},
    pio::{self, Pio},
};
use static_cell::StaticCell;

use crate::resources::WiFi;

bind_interrupts!(struct WiFiIrqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

// Async task for managing the CYW43 Wi-Fi chip
#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await // Runs the Wi-Fi task indefinitely
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

pub async fn initialize_wifi(spawner: &Spawner, p: WiFi) -> (NetDriver<'static>, Control<'static>) {
    let fw = include_bytes!("firmware/43439A0.bin");
    let clm = include_bytes!("firmware/43439A0_clm.bin");

    let pwr = Output::new(p.pwr, Level::Low);
    let cs = Output::new(p.cs, Level::High);

    // Set up PIO (Programmable I/O) and SPI communication for the Wi-Fi chip
    let mut pio = Pio::new(p.pio, WiFiIrqs);
    // SPI uses specific GPIO pins (PIN_24 for data in, PIN_29 for data out)
    let spi = PioSpi::new(&mut pio.common, pio.sm0, pio.irq0, cs, p.dio, p.clk, p.dma);

    // Create a static state object for managing the CYW43 chip
    static STATE: StaticCell<cyw43::State> = StaticCell::new(); // Static means it stays in memory for the lifetime of the program
    let state = STATE.init(cyw43::State::new()); // Initialize the state

    // Initialize the Wi-Fi chip using the provided firmware, power control, and SPI interface
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

    // Spawn the Wi-Fi task that will continuously manage the chip's internal state in the background
    unwrap!(spawner.spawn(cyw43_task(runner)));

    // Initialize the Wi-Fi chip with the CLM data (important for country-specific wireless regulations)
    control.init(clm).await;

    // Set the Wi-Fi chip to power-saving mode to conserve energy when it's not active
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    (net_device, control)
}

pub async fn start_ap_wpa2(
    spawner: &Spawner,
    control: &mut Control<'static>,
    net_device: NetDriver<'static>,
    ssid: &str,
    password: &str,
    channel: u8,
) {
    let config = Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: embassy_net::Ipv4Cidr::new(embassy_net::Ipv4Address::new(169, 254, 1, 1), 16),
        dns_servers: heapless::Vec::new(),
        gateway: None,
    });

    // Generate random seed
    let seed = 23266246;

    // Init network stack
    static STACK: StaticCell<Stack<cyw43::NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    ));

    unwrap!(spawner.spawn(net_task(stack)));

    //control.start_ap_open("cyw43", 5).await;
    control.start_ap_wpa2(ssid, password, channel).await;
}
