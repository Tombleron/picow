#[derive(Clone, Copy)]
pub enum NotchFrequency {
    Freq50Hz = 50,
    Freq60Hz = 60,
}

#[derive(Clone, Copy)]
pub enum SampleFrequency {
    Freq500Hz = 500,
    Freq1000Hz = 1000,
}

#[derive(Clone, Copy)]
enum FilterType {
    Lowpass,
    Highpass,
}

// Constants for filter coefficients
const LPF_NUMERATOR_COEF: [[f32; 3]; 2] = [[0.3913, 0.7827, 0.3913], [0.1311, 0.2622, 0.1311]];

const LPF_DENOMINATOR_COEF: [[f32; 3]; 2] = [[1.0000, 0.3695, 0.1958], [1.0000, -0.7478, 0.2722]];

const HPF_NUMERATOR_COEF: [[f32; 3]; 2] = [[0.8371, -1.6742, 0.8371], [0.9150, -1.8299, 0.9150]];

const HPF_DENOMINATOR_COEF: [[f32; 3]; 2] = [[1.0000, -1.6475, 0.7009], [1.0000, -1.8227, 0.8372]];

// Anti-hum filter coefficients for 50Hz
const AHF_NUMERATOR_COEF_50HZ: [[f32; 6]; 2] = [
    [0.9522, -1.5407, 0.9522, 0.8158, -0.8045, 0.0855],
    [0.5869, -1.1146, 0.5869, 1.0499, -2.0000, 1.0499],
];

const AHF_DENOMINATOR_COEF_50HZ: [[f32; 6]; 2] = [
    [1.0000, -1.5395, 0.9056, 1.0000, -1.1187, 0.3129],
    [1.0000, -1.8844, 0.9893, 1.0000, -1.8991, 0.9892],
];

const AHF_OUTPUT_GAIN_COEF_50HZ: [f32; 2] = [1.3422, 1.4399];

// Anti-hum filter coefficients for 60Hz
const AHF_NUMERATOR_COEF_60HZ: [[f32; 6]; 2] = [
    [0.9528, -1.3891, 0.9528, 0.8272, -0.7225, 0.0264],
    [0.5824, -1.0810, 0.5824, 1.0736, -2.0000, 1.0736],
];

const AHF_DENOMINATOR_COEF_60HZ: [[f32; 6]; 2] = [
    [1.0000, -1.3880, 0.9066, 1.0000, -0.9739, 0.2371],
    [1.0000, -1.8407, 0.9894, 1.0000, -1.8584, 0.9891],
];

const AHF_OUTPUT_GAIN_COEF_60HZ: [f32; 2] = [1.3430, 1.4206];

struct Filter2nd {
    states: [f32; 2],
    num: [f32; 3],
    den: [f32; 3],
}

impl Filter2nd {
    fn new() -> Self {
        Self {
            states: [0.0; 2],
            num: [0.0; 3],
            den: [0.0; 3],
        }
    }

    fn init(&mut self, ftype: FilterType, sample_freq: SampleFrequency) {
        self.states = [0.0; 2];

        let idx = match sample_freq {
            SampleFrequency::Freq500Hz => 0,
            SampleFrequency::Freq1000Hz => 1,
        };

        match ftype {
            FilterType::Lowpass => {
                self.num.copy_from_slice(&LPF_NUMERATOR_COEF[idx]);
                self.den.copy_from_slice(&LPF_DENOMINATOR_COEF[idx]);
            }
            FilterType::Highpass => {
                self.num.copy_from_slice(&HPF_NUMERATOR_COEF[idx]);
                self.den.copy_from_slice(&HPF_DENOMINATOR_COEF[idx]);
            }
        }
    }

    fn update(&mut self, input: f32) -> f32 {
        let tmp =
            (input - self.den[1] * self.states[0] - self.den[2] * self.states[1]) / self.den[0];
        let output =
            self.num[0] * tmp + self.num[1] * self.states[0] + self.num[2] * self.states[1];

        self.states[1] = self.states[0];
        self.states[0] = tmp;

        output
    }
}

struct Filter4th {
    states: [f32; 4],
    num: [f32; 6],
    den: [f32; 6],
    gain: f32,
}

impl Filter4th {
    fn new() -> Self {
        Self {
            states: [0.0; 4],
            num: [0.0; 6],
            den: [0.0; 6],
            gain: 0.0,
        }
    }

    fn init(&mut self, sample_freq: SampleFrequency, hum_freq: NotchFrequency) {
        self.states = [0.0; 4];

        let idx = match sample_freq {
            SampleFrequency::Freq500Hz => 0,
            SampleFrequency::Freq1000Hz => 1,
        };

        match hum_freq {
            NotchFrequency::Freq50Hz => {
                self.num.copy_from_slice(&AHF_NUMERATOR_COEF_50HZ[idx]);
                self.den.copy_from_slice(&AHF_DENOMINATOR_COEF_50HZ[idx]);
                self.gain = AHF_OUTPUT_GAIN_COEF_50HZ[idx];
            }
            NotchFrequency::Freq60Hz => {
                self.num.copy_from_slice(&AHF_NUMERATOR_COEF_60HZ[idx]);
                self.den.copy_from_slice(&AHF_DENOMINATOR_COEF_60HZ[idx]);
                self.gain = AHF_OUTPUT_GAIN_COEF_60HZ[idx];
            }
        }
    }

    fn update(&mut self, input: f32) -> f32 {
        let mut stage_out = self.num[0] * input + self.states[0];
        self.states[0] = (self.num[1] * input + self.states[1]) - self.den[1] * stage_out;
        self.states[1] = self.num[2] * input - self.den[2] * stage_out;
        let stage_in = stage_out;
        stage_out = self.num[3] * stage_out + self.states[2];
        self.states[2] = (self.num[4] * stage_in + self.states[3]) - self.den[4] * stage_out;
        self.states[3] = self.num[5] * stage_in - self.den[5] * stage_out;

        self.gain * stage_out
    }
}

pub struct EMGFilters {
    lpf: Filter2nd,
    hpf: Filter2nd,
    ahf: Filter4th,
    bypass_enabled: bool,
    notch_filter_enabled: bool,
    lowpass_filter_enabled: bool,
    highpass_filter_enabled: bool,
}

impl EMGFilters {
    pub fn new() -> Self {
        Self {
            lpf: Filter2nd::new(),
            hpf: Filter2nd::new(),
            ahf: Filter4th::new(),
            bypass_enabled: true,
            notch_filter_enabled: true,
            lowpass_filter_enabled: true,
            highpass_filter_enabled: true,
        }
    }

    pub fn init(
        &mut self,
        sample_freq: SampleFrequency,
        notch_freq: NotchFrequency,
        enable_notch_filter: bool,
        enable_lowpass_filter: bool,
        enable_highpass_filter: bool,
    ) {
        self.bypass_enabled = !matches!(
            (sample_freq, notch_freq),
            (
                SampleFrequency::Freq500Hz | SampleFrequency::Freq1000Hz,
                NotchFrequency::Freq50Hz | NotchFrequency::Freq60Hz
            )
        );

        self.lpf.init(FilterType::Lowpass, sample_freq);
        self.hpf.init(FilterType::Highpass, sample_freq);
        self.ahf.init(sample_freq, notch_freq);

        self.notch_filter_enabled = enable_notch_filter;
        self.lowpass_filter_enabled = enable_lowpass_filter;
        self.highpass_filter_enabled = enable_highpass_filter;
    }

    pub fn update(&mut self, input_value: i32) -> i32 {
        if self.bypass_enabled {
            return input_value;
        }

        let mut output = input_value as f32;

        if self.notch_filter_enabled {
            output = self.ahf.update(output);
        }

        if self.lowpass_filter_enabled {
            output = self.lpf.update(output);
        }

        if self.highpass_filter_enabled {
            output = self.hpf.update(output);
        }

        output as i32
    }
}
