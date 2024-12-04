pub mod calibration;
pub mod command_handler;
pub mod events;
pub mod operation;

use core::fmt::Display;

use defmt::{debug, info};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use calibration::{CalibrationCommand, CalibrationStage, CALIBRATION_STATE, START_CALIBRATION};
use events::Events;
use operation::OperationCommand;
use operation::START_OPERATION;

use crate::emg::EmgSensorsState;

#[derive(Copy, Clone)]
pub enum ProgramStage {
    Calibration,
    Operation,
    Error,
}

impl ProgramStage {
    pub async fn transition(&mut self, new_state: ProgramStage) {
        let mut state = PROGRAM_STATE.lock().await;
        *state = new_state;
    }
}

pub type ProgramStateMutex = Mutex<CriticalSectionRawMutex, ProgramStage>;
pub static PROGRAM_STATE: ProgramStateMutex = Mutex::new(ProgramStage::Calibration);

#[embassy_executor::task]
pub async fn orchestrator() {
    info!("Starting orchestrator");
    let event_receiver = events::EVENT_CHANNEL.receiver();
    START_CALIBRATION.signal(CalibrationCommand);

    loop {
        let event = event_receiver.receive().await;
        debug!("Received event");

        {
            let mut state = PROGRAM_STATE.lock().await;

            match event {
                // TODO: Add calibrated data to event
                Events::CalibrationFinished => {
                    info!("Calibration finished, transitioning to Operation state");
                    *state = ProgramStage::Operation;
                    START_OPERATION.signal(OperationCommand {});
                }
            }
        }
    }
}
