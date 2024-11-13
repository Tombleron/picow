use embassy_rp::{
    adc::{Adc, Async, Config, InterruptHandler},
    bind_interrupts,
    peripherals::ADC,
};

bind_interrupts!(pub struct AdcIrqs {
    ADC_IRQ_FIFO => InterruptHandler;
});

pub fn init_adc(adc: ADC) -> Adc<'static, Async> {
    let adc = Adc::new(adc, AdcIrqs, Config::default());

    adc
}
