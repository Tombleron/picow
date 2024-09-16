#![no_std]
#![no_main]
use {defmt_rtt as _, panic_probe as _};

mod pwm;
mod resources;
mod wifi;

use embassy_executor::Spawner;
use embassy_rp::pio::Pio;
use embassy_time::{Duration, Timer};
use pwm::{PwmIrqs, PwmPio};

use resources::{AssignedResources, WiFi};
use wifi::{initialize_wifi, start_ap_wpa2};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let Pio {
        mut common, sm0, ..
    } = Pio::new(p.PIO1, PwmIrqs);
    let mut pwm_pio = PwmPio::new(&mut common, sm0, p.PIN_19);

    pwm_pio.set_period(Duration::from_micros(20400));
    pwm_pio.start();
    pwm_pio.write(Duration::from_micros(10200));

    let r = split_resources!(p);

    let (net_device, mut control) = initialize_wifi(&spawner, r.wifi).await;
    // start_ap_wpa2(&spawner, &mut control, net_device, "cyw43", "anime", 5).await;

    let mut duration = 0;

    loop {
        duration += 1;
        pwm_pio.write(Duration::from_micros(duration % 10000));
        Timer::after_millis(1).await;
    }
}
