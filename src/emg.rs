use core::sync::atomic::{AtomicI32, Ordering};

use defmt::*;
use embassy_rp::adc::{Adc, Async, Channel as AdcChannel};
use embassy_time::{Duration, Ticker};

use crate::filters::{
    mean::MovingAvg,
    EMG::{EMGFilters, NotchFrequency, SampleFrequency},
};

pub static EMG1_VALUE: AtomicI32 = AtomicI32::new(0);
pub static EMG2_VALUE: AtomicI32 = AtomicI32::new(0);

#[embassy_executor::task]
pub async fn emg_reading_task(
    mut adc: Adc<'static, Async>,
    mut emg1: EMGSensor<'static>,
    mut emg2: EMGSensor<'static>,
) {
    info!("EMG reading task started!");
    let mut ticker = Ticker::every(Duration::from_micros(2000));

    loop {
        ticker.next().await;

        let emg1_data = emg1.read(&mut adc).await.unwrap();
        let emg2_data = emg2.read(&mut adc).await.unwrap();

        EMG1_VALUE.store(emg1_data, Ordering::Relaxed);
        EMG2_VALUE.store(emg2_data, Ordering::Relaxed);
    }
}

pub struct EMGSensor<'a> {
    filter: EMGFilters,
    avg: MovingAvg<250>,
    pin: AdcChannel<'a>,
}

impl<'a> EMGSensor<'a> {
    pub fn new(pin: AdcChannel<'a>) -> Self {
        let mut filter = EMGFilters::new();
        filter.init(
            SampleFrequency::Freq500Hz,
            NotchFrequency::Freq50Hz,
            true,
            true,
            true,
        );

        Self {
            filter,
            avg: MovingAvg::new(true),
            pin,
        }
    }

    pub async fn read(&mut self, adc: &mut Adc<'_, Async>) -> Option<i32> {
        let adc_value = adc.read(&mut self.pin).await.ok()?;
        let filtered_value = self.filter.update(adc_value as i32);
        let filtered_value = filtered_value * filtered_value;
        let avg_value = self.avg.reading(filtered_value);

        Some(avg_value)
    }
}

pub struct EmgSensorsState {
    pub emg1_value: i32,
    pub emg2_value: i32,
}

impl EmgSensorsState {
    pub async fn gather() -> Self {
        Self {
            emg1_value: EMG1_VALUE.load(Ordering::Relaxed),
            emg2_value: EMG2_VALUE.load(Ordering::Relaxed),
        }
    }
}
