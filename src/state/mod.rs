pub mod calibration;
pub mod events;
pub mod operation;
pub mod state_sender;

use core::fmt::Display;

use defmt::{debug, info};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use calibration::{CalibrationCommand, CalibrationStage, CALIBRATION_STATE, START_CALIBRATION};
use events::Events;
use operation::OperationCommand;
use operation::START_OPERATION;

use crate::emg::EmgSensorsState;

pub struct ApplicationState {
    emg: EmgSensorsState,
    stage: ProgramStage,
    calibration_data: Option<CalibrationStage>,
}

impl ApplicationState {
    pub async fn gather() -> Self {
        let emg = EmgSensorsState::gather().await;
        let stage = *PROGRAM_STATE.lock().await;

        let calibration = match stage {
            ProgramStage::Calibration => Some(*CALIBRATION_STATE.lock().await),
            ProgramStage::Operation => None,
            ProgramStage::Error => None,
        };

        ApplicationState {
            emg,
            stage: *PROGRAM_STATE.lock().await,
            calibration_data: calibration,
        }
    }
}

impl Display for ApplicationState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "EMG1: {}, EMG2: {},Stage: {}",
            self.emg.emg1_value, self.emg.emg2_value, self.stage
        )?;
        if let Some(calibration) = self.calibration_data {
            write!(f, ", Cal: {}", calibration)
        } else {
            write!(f, ", Cal: None")
        }
    }
}

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

impl Display for ProgramStage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ProgramStage::Calibration => "Calibration",
                ProgramStage::Operation => "Operation",
                ProgramStage::Error => "Error",
            }
        )
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

// #[derive(Default)]
// struct Calibration {
//     stage: CalibrationStage,
//     last_zero_time: Option<Instant>,
// }

// pub struct Program {
//     state: ProgramState,

//     calibration_data: Calibration,
// }

// impl<'a> Program<'a> {
//     pub fn new(adc: Adc<'a, AdcAsync>, uart: Uart<'a, UART0, UartAsync>) -> Self {
//         info!("Creating new Program instance");
//         Self {
//             state: ProgramState::Calibration,

//             calibration_data: Calibration::default(),
//         }
//     }

//     fn transiton_to_state(&mut self, state: ProgramState) {
//         match state {
//             ProgramState::Calibration => {
//                 info!("Transitioning to Calibration state");
//                 self.calibration_data = Calibration::default();
//                 self.state = ProgramState::Calibration;
//             }
//             ProgramState::Operation => {
//                 info!("Transitioning to Operation state");
//                 self.state = ProgramState::Operation;
//             }
//             ProgramState::Error => {
//                 error!("Transitioning to Error state");
//                 self.state = ProgramState::Error;
//             }
//         }
//     }

//     pub async fn tick(&mut self) {
//         match &mut self.state {
//             ProgramState::Calibration => self.handle_calibration().await,
//             ProgramState::Operation => self.handle_operation().await,
//             ProgramState::Error => self.handle_error().await,
//         };
//     }

//     async fn handle_calibration(&mut self) {
//         match &mut self.calibration_data.stage {
//             CalibrationStage::WaitForZero => {
//                 debug!("WaitForZero - EMG1: {}, EMG2: {}", emg1_value, emg2_value);

//                 if emg1_value == 0 && emg2_value == 0 {
//                     match self.calibration_data.last_zero_time {
//                         None => {
//                             info!("Starting zero wait period");
//                             self.calibration_data.last_zero_time = Some(Instant::now());
//                         }
//                         Some(zero_time) => {
//                             if zero_time.elapsed().as_millis() > 1000 {
//                                 info!("Zero wait period complete, moving to peak calibration");
//                                 self.calibration_data.stage = CalibrationStage::PeakCalibration;
//                             }
//                         }
//                     }
//                 } else {
//                     debug!("Resetting zero wait period");
//                     self.calibration_data.last_zero_time = None;
//                 }
//             }
//             CalibrationStage::PeakCalibration => {
//                 if let Some(start_time) = self.calibration_data.last_zero_time {
//                     if start_time.elapsed().as_millis() > 5000 {
//                         info!("Peak calibration complete");
//                         self.transiton_to_state(ProgramState::Operation);
//                     } else {
//                         if emg1_value > self.emg1.max_value {
//                             self.emg1.max_value = emg1_value;
//                             debug!("New max EMG1: {}", self.emg1.max_value);
//                         }
//                         if emg2_value > self.emg2.max_value {
//                             self.emg2.max_value = emg2_value;
//                             debug!("New max EMG2: {}", self.emg2.max_value);
//                         }
//                     }
//                 } else {
//                     self.calibration_data.last_zero_time = Some(Instant::now());
//                 }
//             }
//         }
//     }

//     async fn handle_operation(&mut self) {
//         let emg1_value = self.emg1.read(&mut self.adc).await.unwrap();
//         let emg2_value = self.emg2.read(&mut self.adc).await.unwrap();

//         self.uart.write(&i32_to_bytes(90000)).await.unwrap();
//         self.uart.write(",".as_bytes()).await.unwrap();
//         self.uart.write(&i32_to_bytes(-1000)).await.unwrap();
//         self.uart.write(",".as_bytes()).await.unwrap();

//         self.uart
//             .write(&i32_to_bytes(self.emg1.max_value))
//             .await
//             .unwrap();
//         self.uart.write(",".as_bytes()).await.unwrap();
//         self.uart
//             .write(&i32_to_bytes(self.emg2.max_value))
//             .await
//             .unwrap();
//         self.uart.write(",".as_bytes()).await.unwrap();

//         self.uart.write(&i32_to_bytes(emg1_value)).await.unwrap();
//         self.uart.write(",".as_bytes()).await.unwrap();
//         self.uart.write(&i32_to_bytes(emg2_value)).await.unwrap();
//         self.uart.write("\r\n".as_bytes()).await.unwrap();
//     }

//     async fn handle_error(&mut self) {}
// }
