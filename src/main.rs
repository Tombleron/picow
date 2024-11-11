#![no_std]
#![no_main]
use {defmt_rtt as _, panic_probe as _};

mod adc;
mod filters;
mod pwm;
mod resources;
mod serial;
mod wifi;

use adc::init_adc;
use embassy_executor::Spawner;
use embassy_rp::{
    adc::{Adc, Channel},
    gpio::{AnyPin, Level, Output, Pull},
    pio::Pio,
    uart,
};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Ticker, Timer};
use filters::{
    mean::MovingAvg,
    EMG::{EMGFilters, NotchFrequency, SampleFrequency},
};
use pwm::{PwmIrqs, PwmPio};

use defmt::*;

use resources::{AssignedResources, WiFi};
use wifi::{initialize_wifi, start_ap_wpa2};

const THRESHOLD: u16 = 50;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting up...");
    let mut p = embassy_rp::init(Default::default());

    let config = uart::Config::default();
    let mut uart =
        uart::Uart::new_with_rtscts_blocking(p.UART0, p.PIN_0, p.PIN_1, p.PIN_3, p.PIN_2, config);

    info!("Initializing EMG filter...");
    let mut filter = EMGFilters::new();
    filter.init(
        SampleFrequency::Freq500Hz,
        NotchFrequency::Freq50Hz,
        true,
        true,
        true,
    );
    info!("EMG filter initialized!");

    info!("Creating moving average filter...");
    let mut avg: MovingAvg<300> = MovingAvg::new(true);
    info!("Moving average filter created!");

    info!("Setting up ADC...");
    let mut adc = init_adc(&mut p.ADC);
    let mut p27 = Channel::new_pin(p.PIN_27, Pull::None);
    info!("ADC setup complete!");

    let mut ticker = Ticker::every(Duration::from_micros(2000));
    info!("Starting main loop with 500Hz sampling...");

    let mut counter = 0;

    loop {
        ticker.next().await;
        let adc_value = adc.read(&mut p27).await.unwrap();

        let filtered_value = if adc_value > THRESHOLD {
            debug!("ADC value {} above threshold", adc_value);
            filter.update(adc_value as i32)
        } else {
            debug!("ADC value {} below threshold", adc_value);
            filter.update(0)
        };
        let filtered_value = filtered_value * filtered_value;
        let avg_value = avg.reading(filtered_value);

        if counter == 50 {
            info!(
                "ADC: {} Filtered: {} Average: {}",
                adc_value, filtered_value, avg_value
            );
            uart.blocking_write(&i32_to_bytes(90000)).unwrap();
            uart.blocking_write(" ".as_bytes()).unwrap();
            uart.blocking_write(&i32_to_bytes(-1000)).unwrap();
            uart.blocking_write(" ".as_bytes()).unwrap();
            uart.blocking_write(&i32_to_bytes(avg_value)).unwrap();
            uart.blocking_write("\r\n".as_bytes()).unwrap();
            counter = 0;
        }

        counter += 1;
    }
}
const BUFFER_SIZE: usize = 10;
const ZERO_ASCII: u8 = b'0';
const MINUS_ASCII: u8 = b'-';
const SPACE_ASCII: u8 = b' ';
const BASE: i32 = 10;
const LAST_INDEX: usize = BUFFER_SIZE - 1;
fn i32_to_bytes(num: i32) -> [u8; BUFFER_SIZE] {
    let mut bytes = [0u8; BUFFER_SIZE];
    let mut i = LAST_INDEX;
    let mut n = if num < 0 { -num } else { num };

    if n == 0 {
        bytes[LAST_INDEX] = ZERO_ASCII;
        return bytes;
    }

    while n > 0 {
        bytes[i] = ZERO_ASCII + (n % BASE) as u8;
        n /= BASE;
        if i > 0 {
            i -= 1;
        }
    }

    if num < 0 {
        bytes[i] = MINUS_ASCII;
        i -= 1;
    }
    for j in 0..=i {
        bytes[j] = SPACE_ASCII;
    }
    bytes
}
