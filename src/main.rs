#![no_std]
#![no_main]
use {defmt_rtt as _, panic_probe as _};

mod adc;
mod emg;
mod filters;
mod pwm;
mod resources;
mod serial;
mod state;
mod wifi;

use adc::init_adc;
use embassy_executor::Spawner;
use embassy_rp::{adc::Channel, gpio::Pull, uart};

use embassy_time::Duration;
use emg::{emg_reading_task, EMGSensor};

use defmt::*;
use state::{calibration::calibration_task, orchestrator, state_sender::state_sender_task};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting up...");
    let p = embassy_rp::init(Default::default());

    let config = uart::Config::default();
    let uart = uart::Uart::new(
        p.UART0,
        p.PIN_0,
        p.PIN_1,
        serial::SerialInterrupts,
        p.DMA_CH0,
        p.DMA_CH1,
        config,
    );

    let adc = init_adc(p.ADC);

    info!("Initializing EMG filters...");
    let emg1 = EMGSensor::new(Channel::new_pin(p.PIN_27, Pull::None));
    let emg2 = EMGSensor::new(Channel::new_pin(p.PIN_26, Pull::None));
    info!("EMG filters initialized!");

    info!("Spawning EMG reading task...");
    unwrap!(spawner.spawn(emg_reading_task(adc, emg1, emg2)));
    info!("EMG reading task spawned!");

    info!("Spawning StateSender task...");
    unwrap!(spawner.spawn(state_sender_task(Duration::from_millis(100), uart)));
    info!("StateSender task spawned!");

    info!("Starting calibration...");
    unwrap!(spawner.spawn(calibration_task()));
    info!("Calibration task spawned!");

    info!("Starting orchestrator...");
    unwrap!(spawner.spawn(orchestrator()));
    info!("Orchestrator task spawned!");
}
