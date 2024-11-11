use embassy_rp::{
    adc::{Adc, Async, Config, InterruptHandler},
    bind_interrupts,
    peripherals::ADC,
};

bind_interrupts!(pub struct AdcIrqs {
    ADC_IRQ_FIFO => InterruptHandler;
});

pub fn init_adc<'a>(adc: &'a mut ADC) -> Adc<'_, Async> {
    let adc = Adc::new(adc, AdcIrqs, Config::default());

    adc
}
