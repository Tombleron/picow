#![no_std]
#![no_main]
use {defmt_rtt as _, panic_probe as _};

mod adc;
// mod bluetooth;
mod commands;
mod emg;
mod filters;
mod resources;
mod serial;
mod state;

use adc::init_adc;
use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{adc::Channel, gpio::Pull};

use emg::{emg_reading_task, EMGSensor};
use resources::*;
use state::{
    calibration::calibration_task,
    command_handler::{command_handler_task, response_reader_task},
    operation::operation_task,
    orchestrator,
};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting up...");
    let p = embassy_rp::init(Default::default());

    let r = split_resources!(p);

    let uart = serial::init_buffered_uart(r.uart);
    let (tx, rx) = uart.split();

    let adc = init_adc(r.adc.adc);

    info!("Initializing EMG filters...");
    let emg1 = EMGSensor::new(Channel::new_pin(p.PIN_27, Pull::None));
    let emg2 = EMGSensor::new(Channel::new_pin(p.PIN_26, Pull::None));
    info!("EMG filters initialized!");

    info!("Spawning EMG reading task...");
    unwrap!(spawner.spawn(emg_reading_task(adc, emg1, emg2)));
    info!("EMG reading task spawned!");

    info!("Starting calibration task...");
    unwrap!(spawner.spawn(calibration_task()));
    info!("Calibration task spawned!");

    info!("Starting operation task...");
    unwrap!(spawner.spawn(operation_task()));
    info!("Operation task spawned!");

    info!("Starting orchestrator...");
    unwrap!(spawner.spawn(orchestrator()));
    info!("Orchestrator task spawned!");

    info!("Starting command handler...");
    unwrap!(spawner.spawn(command_handler_task(tx)));
    info!("Command handler started!");

    info!("Starting response reader...");
    unwrap!(spawner.spawn(response_reader_task(rx)));
    info!("Response reader started!");
    // info!("Starting bluetooth...");
    // unwrap!(spawner.spawn(bluetooth::initialize_bluetooth(spawner, r.blt)));
    // info!("Bluetooth initialized!");
}
