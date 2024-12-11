use defmt::info;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

use crate::{
    commands::{CommandType, Packet},
    state::command_handler::COMMAND_CHANNEL,
};

pub struct OperationCommand;
pub static START_OPERATION: Signal<CriticalSectionRawMutex, OperationCommand> = Signal::new();

#[embassy_executor::task]
pub async fn operation_task() {
    loop {
        info!("Waiting for operation start signal");
        START_OPERATION.wait().await;
        info!("Operation signal received");

        loop {
            // Packet::new(CommandType::RequestSensors).send().await;

            embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
        }
    }
}
