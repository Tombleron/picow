#![no_std]
#![no_main]
use cyw43::Control;
use cyw43_pio::PioSpi;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_rp::gpio::Level;
use embassy_rp::peripherals::{DMA_CH0, PIO0, PWM_SLICE4};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::pwm::Pwm;
use embassy_rp::{bind_interrupts, gpio::Output};
use embassy_rp::{pwm, Peripherals};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

// Async task for managing the CYW43 Wi-Fi chip
#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await // Runs the Wi-Fi task indefinitely
}

async fn initialize(spawner: Spawner, p: Peripherals) -> Control<'static> {
    let fw = include_bytes!("firmware/43439A0.bin");
    let clm = include_bytes!("firmware/43439A0_clm.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);

    // Set up PIO (Programmable I/O) and SPI communication for the Wi-Fi chip
    let mut pio = Pio::new(p.PIO0, Irqs);
    // SPI uses specific GPIO pins (PIN_24 for data in, PIN_29 for data out)
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    // Create a static state object for managing the CYW43 chip
    static STATE: StaticCell<cyw43::State> = StaticCell::new(); // Static means it stays in memory for the lifetime of the program
    let state = STATE.init(cyw43::State::new()); // Initialize the state

    // Initialize the Wi-Fi chip using the provided firmware, power control, and SPI interface
    let (_net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

    // Spawn the Wi-Fi task that will continuously manage the chip's internal state in the background
    unwrap!(spawner.spawn(cyw43_task(runner)));

    // Initialize the Wi-Fi chip with the CLM data (important for country-specific wireless regulations)
    control.init(clm).await;

    // Set the Wi-Fi chip to power-saving mode to conserve energy when it's not active
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    control
}

// Initialize pwm on the Raspberry Pi Pico
async fn pwm_init(peripherals: Peripherals) -> Pwm<'static> {
    let mut config = pwm::Config::default();
    config.top = 0x8000;
    config.compare_b = 0x4000;
    Pwm::new_output_b(peripherals.PWM_SLICE1, peripherals.PIN_19, config)
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initializes the Raspberry Pi Pico's hardware peripherals (like GPIO, PIO, etc.)
    let peripherals = embassy_rp::init(Default::default());

    // let mut control = initialize(spawner, peripherals).await;

    let delay = Duration::from_secs(5);
    let mut led_status = false;

    let mut pwm = pwm_init(peripherals).await;

    loop {
        info!("anime{}", pwm.wrapped());
        // control.gpio_set(0, led_status).await;
        // Timer::after(delay).await;
        // led_status = !led_status;
    }
}
