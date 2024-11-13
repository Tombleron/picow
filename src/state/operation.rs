use defmt::info;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

pub struct OperationCommand;
pub static START_OPERATION: Signal<CriticalSectionRawMutex, OperationCommand> = Signal::new();

#[embassy_executor::task]
pub async fn operation() {
    loop {
        info!("Waiting for operation start signal");
        START_OPERATION.wait().await;
        info!("Operation signal received");
    }
}
