pub struct MovingAvg<const N: usize> {
    interval: usize,
    avoid_div_by_zero: bool,
    nbr_readings: usize,
    sum: i64,
    next: usize,
    readings: [i32; N],
}

impl<const N: usize> MovingAvg<N> {
    pub fn new(avoid_div_by_zero: bool) -> Self {
        MovingAvg {
            interval: N,
            avoid_div_by_zero,
            nbr_readings: 0,
            sum: 0,
            next: 0,
            readings: [0; N],
        }
    }

    pub fn reading(&mut self, new_reading: i32) -> i32 {
        // add each new data point to the sum until the readings array is filled
        if self.nbr_readings < self.interval {
            self.nbr_readings += 1;
            self.sum += new_reading as i64;
        } else {
            // once the array is filled, subtract the oldest data point and add the new one
            self.sum = self.sum - self.readings[self.next] as i64 + new_reading as i64;
        }

        self.readings[self.next] = new_reading;
        self.next = (self.next + 1) % self.interval;

        ((self.sum + (self.nbr_readings as i64 / 2)) / self.nbr_readings as i64) as i32
    }

    pub fn get_avg(&self) -> i32 {
        if self.nbr_readings > 0 || !self.avoid_div_by_zero {
            ((self.sum + (self.nbr_readings as i64 / 2)) / self.nbr_readings as i64) as i32
        } else {
            0
        }
    }

    pub fn get_avg_n(&self, n_points: usize) -> i32 {
        if n_points < 1 || n_points > self.interval || n_points > self.nbr_readings {
            return 0;
        }

        let mut sum: i64 = 0;
        let mut i = self.next;

        for _ in 0..n_points {
            i = if i == 0 { self.interval - 1 } else { i - 1 };
            sum += self.readings[i] as i64;
        }

        ((sum + (n_points as i64 / 2)) / n_points as i64) as i32
    }

    pub fn reset(&mut self) {
        self.nbr_readings = 0;
        self.sum = 0;
        self.next = 0;
    }

    pub fn get_count(&self) -> usize {
        self.nbr_readings
    }

    pub fn get_readings(&self) -> &[i32] {
        &self.readings
    }
}
