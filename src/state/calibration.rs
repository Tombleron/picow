use core::fmt::Display;

use defmt::info;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, signal::Signal};
use embassy_time::{Instant, Timer};

use super::events::{Events, EVENT_CHANNEL};

#[derive(Clone, Copy)]
pub enum CalibrationStage {
    WaitForZero(Option<Instant>),
    PeakCalibration(i32, i32),
}

impl Display for CalibrationStage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CalibrationStage::WaitForZero(None) => write!(f, "None"),
            CalibrationStage::WaitForZero(Some(instant)) => write!(f, "{instant}"),
            CalibrationStage::PeakCalibration(min, max) => {
                write!(f, "Peak({min} {max})")
            }
        }
    }
}

type CalibrationStateMutex = Mutex<CriticalSectionRawMutex, CalibrationStage>;
pub static CALIBRATION_STATE: CalibrationStateMutex =
    Mutex::new(CalibrationStage::WaitForZero(None));

pub struct CalibrationCommand;
pub static START_CALIBRATION: Signal<CriticalSectionRawMutex, CalibrationCommand> = Signal::new();

async fn calibration() {
    info!("Starting calibration");

    let now = Instant::now();
    *CALIBRATION_STATE.lock().await = CalibrationStage::WaitForZero(Some(now));

    Timer::after_secs(3).await;

    *CALIBRATION_STATE.lock().await = CalibrationStage::PeakCalibration(0, 0);

    Timer::after_secs(5).await;

    info!("Calibration finished");
    EVENT_CHANNEL.send(Events::CalibrationFinished).await;
}

#[embassy_executor::task]
pub async fn calibration_task() {
    loop {
        info!("Waiting for calibration start signal");
        START_CALIBRATION.wait().await;
        calibration().await;
    }
}
