//! Allocation-free stereo processing. All delay memory is prepared in constructors.
use std::f32::consts::PI;

pub type Frame = [f32; 2];

pub fn db_gain(db: f32) -> f32 {
    10.0_f32.powf(db.clamp(-90.0, 24.0) / 20.0)
}
pub fn crossfade(x: f32) -> [f32; 2] {
    let angle = x.clamp(0.0, 1.0) * PI * 0.5;
    [angle.cos(), angle.sin()]
}

#[derive(Clone, Copy)]
pub struct Smooth {
    value: f32,
    coefficient: f32,
}
impl Smooth {
    pub fn new(value: f32, sample_rate: f32, milliseconds: f32) -> Self {
        Self {
            value,
            coefficient: 1.0 - (-1.0 / (sample_rate * milliseconds * 0.001).max(1.0)).exp(),
        }
    }
    pub fn next(&mut self, target: f32) -> f32 {
        self.value += (target - self.value) * self.coefficient;
        self.value
    }
    pub fn reset(&mut self, value: f32) {
        self.value = value;
    }
}

#[derive(Default, Clone, Copy)]
pub struct OnePole {
    state: Frame,
}
impl OnePole {
    pub fn process(&mut self, x: Frame, coefficient: f32) -> Frame {
        for (state, sample) in self.state.iter_mut().zip(x) {
            *state += coefficient * (sample - *state);
        }
        self.state
    }
    pub fn clear(&mut self) {
        self.state = [0.0; 2];
    }
}

/// TPT state variable filter, channel states never shared.
pub struct Filter {
    ic1: Frame,
    ic2: Frame,
    g: f32,
    k: f32,
    last: (f32, f32),
    sr: f32,
}
impl Filter {
    pub fn new(sr: f32) -> Self {
        Self {
            ic1: [0.0; 2],
            ic2: [0.0; 2],
            g: 0.0,
            k: 1.0,
            last: (0.0, 0.0),
            sr,
        }
    }
    pub fn process(&mut self, x: Frame, cutoff: f32, resonance: f32, highpass: bool) -> Frame {
        let cutoff = cutoff.clamp(25.0, self.sr * 0.45);
        if (cutoff - self.last.0).abs() > 0.1 || resonance != self.last.1 {
            self.g = (PI * cutoff / self.sr).tan();
            self.k = 2.0 - 1.9 * resonance.clamp(0.0, 1.0);
            self.last = (cutoff, resonance);
        }
        let a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        let mut out = [0.0; 2];
        for c in 0..2 {
            let v3 = x[c] - self.ic2[c];
            let v1 = a1 * (self.ic1[c] + self.g * v3);
            let v2 = self.ic2[c] + self.g * v1;
            self.ic1[c] = 2.0 * v1 - self.ic1[c];
            self.ic2[c] = 2.0 * v2 - self.ic2[c];
            out[c] = if highpass {
                x[c] - self.k * v1 - v2
            } else {
                v2
            };
        }
        out
    }
    pub fn clear(&mut self) {
        self.ic1 = [0.0; 2];
        self.ic2 = [0.0; 2];
    }
}

pub struct Channel {
    low: OnePole,
    high: OnePole,
    filter: Filter,
    cutoff: Smooth,
    gain: Smooth,
    eq: [Smooth; 3],
    filter_mix: Smooth,
    drive: Smooth,
    sr: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct ChannelParams {
    pub gain: f32,
    pub eq: [f32; 3],
    pub cutoff: f32,
    pub resonance: f32,
    pub highpass: bool,
    pub filter_on: bool,
    pub drive: f32,
}
impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            gain: 1.0,
            eq: [1.0; 3],
            cutoff: 20_000.0,
            resonance: 0.15,
            highpass: false,
            filter_on: false,
            drive: 0.0,
        }
    }
}
impl Channel {
    pub fn new(sr: f32) -> Self {
        Self {
            low: OnePole::default(),
            high: OnePole::default(),
            filter: Filter::new(sr),
            cutoff: Smooth::new(20_000.0, sr, 20.0),
            gain: Smooth::new(0.0, sr, 5.0),
            eq: [Smooth::new(1.0, sr, 10.0); 3],
            filter_mix: Smooth::new(0.0, sr, 10.0),
            drive: Smooth::new(0.0, sr, 10.0),
            sr,
        }
    }
    pub fn process(&mut self, x: Frame, p: ChannelParams) -> Frame {
        let gain = self.gain.next(p.gain);
        let low = self
            .low
            .process(x, 1.0 - (-2.0 * PI * 250.0 / self.sr).exp());
        let below_high = self
            .high
            .process(x, 1.0 - (-2.0 * PI * 2500.0 / self.sr).exp());
        let eq = [
            self.eq[0].next(p.eq[0]),
            self.eq[1].next(p.eq[1]),
            self.eq[2].next(p.eq[2]),
        ];
        let mut shaped = [0.0; 2];
        for c in 0..2 {
            shaped[c] = gain
                * (low[c] * eq[0]
                    + (below_high[c] - low[c]) * eq[1]
                    + (x[c] - below_high[c]) * eq[2]);
        }
        let cutoff = self.cutoff.next(p.cutoff);
        let filtered = self.filter.process(shaped, cutoff, p.resonance, p.highpass);
        let mix = self.filter_mix.next(f32::from(p.filter_on));
        let drive = self.drive.next(p.drive.clamp(0.0, 1.0));
        for c in 0..2 {
            shaped[c] += (filtered[c] - shaped[c]) * mix;
            // Soft clipping; high-drive alias rejection is assessed separately in listening tests.
            let g = 1.0 + drive * 8.0;
            let saturated = (shaped[c] * g).tanh() / g.sqrt();
            shaped[c] += (saturated - shaped[c]) * drive;
        }
        shaped
    }
    pub fn clear(&mut self) {
        self.low.clear();
        self.high.clear();
        self.filter.clear();
        self.gain.reset(0.0);
    }
}

pub struct Delay {
    data: Vec<Frame>,
    pos: usize,
    sr: f32,
    time: Smooth,
    damping: OnePole,
    valid: usize,
}
impl Delay {
    pub fn new(sr: f32) -> Self {
        Self {
            data: vec![[0.0; 2]; (sr * 4.0) as usize + 2],
            pos: 0,
            sr,
            time: Smooth::new(sr * 0.42857, sr, 30.0),
            damping: OnePole::default(),
            valid: 0,
        }
    }
    pub fn process(&mut self, input: Frame, seconds: f32, feedback: f32) -> Frame {
        let delay = self
            .time
            .next((seconds * self.sr).clamp(1.0, self.data.len() as f32 - 2.0));
        let r = (self.pos as f64 - delay as f64).rem_euclid(self.data.len() as f64);
        let i = r as usize % self.data.len();
        let f = (r - r.floor()) as f32;
        let out = std::array::from_fn(|c| {
            if self.valid < delay.ceil() as usize + 1 {
                return 0.0;
            }
            self.data[i][c] * (1.0 - f) + self.data[(i + 1) % self.data.len()][c] * f
        });
        let dark = self.damping.process(out, 0.35);
        self.data[self.pos] = [
            input[0] + (dark[1] * feedback.clamp(0.0, 0.9)).tanh(),
            input[1] + (dark[0] * feedback.clamp(0.0, 0.9)).tanh(),
        ];
        self.pos = (self.pos + 1) % self.data.len();
        self.valid = (self.valid + 1).min(self.data.len());
        out
    }
    pub fn clear(&mut self) {
        self.valid = 0;
        self.damping.clear();
    }
}

struct Comb {
    data: Vec<f32>,
    pos: usize,
    damp: f32,
    valid: usize,
}
impl Comb {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0.0; size],
            pos: 0,
            damp: 0.0,
            valid: 0,
        }
    }
    fn process(&mut self, x: f32, feedback: f32) -> f32 {
        let y = if self.valid >= self.data.len() {
            self.data[self.pos]
        } else {
            0.0
        };
        self.damp += 0.3 * (y - self.damp);
        self.data[self.pos] = x + self.damp * feedback;
        self.pos = (self.pos + 1) % self.data.len();
        self.valid = (self.valid + 1).min(self.data.len());
        y
    }
}
pub struct Reverb {
    combs: [[Comb; 4]; 2],
    highpass: OnePole,
}
impl Reverb {
    pub fn clear(&mut self) {
        for channel in &mut self.combs {
            for comb in channel {
                comb.valid = 0;
                comb.damp = 0.0;
            }
        }
        self.highpass.clear();
    }
    pub fn new(sr: f32) -> Self {
        Self {
            combs: std::array::from_fn(|c| {
                std::array::from_fn(|i| {
                    Comb::new(
                        (([1557, 1617, 1491, 1422][i] + c * 23) as f32 * sr / 44100.0) as usize,
                    )
                })
            }),
            highpass: OnePole::default(),
        }
    }
    pub fn process(&mut self, x: Frame, decay: f32) -> Frame {
        let low = self.highpass.process(x, 0.018);
        std::array::from_fn(|c| {
            self.combs[c]
                .iter_mut()
                .map(|comb| comb.process((x[c] - low[c]) * 0.18, decay.clamp(0.0, 0.88)))
                .sum::<f32>()
        })
    }
}

pub struct Modulation {
    data: Vec<Frame>,
    pos: usize,
    phase: f32,
    sr: f32,
    valid: usize,
}
impl Modulation {
    pub fn new(sr: f32) -> Self {
        Self {
            data: vec![[0.0; 2]; (sr * 0.05) as usize + 2],
            pos: 0,
            phase: 0.0,
            sr,
            valid: 0,
        }
    }
    pub fn process(&mut self, x: Frame, amount: f32, flanger: bool) -> Frame {
        let mut out = x;
        for c in 0..2 {
            let offset = if flanger { 0.003 } else { 0.016 };
            let depth = if flanger { 0.002 } else { 0.006 };
            let delay = (offset + depth * (self.phase + c as f32 * PI * 0.5).sin()) * self.sr;
            let r = (self.pos as f64 - delay as f64).rem_euclid(self.data.len() as f64);
            let i = r as usize % self.data.len();
            let f = (r - r.floor()) as f32;
            let wet = if self.valid > delay.ceil() as usize {
                self.data[i][c] * (1.0 - f) + self.data[(i + 1) % self.data.len()][c] * f
            } else {
                0.0
            };
            out[c] = x[c] * (1.0 - amount * 0.4) + wet * amount * 0.4;
            self.data[self.pos][c] = x[c] + wet * if flanger { 0.35 * amount } else { 0.0 };
        }
        self.pos = (self.pos + 1) % self.data.len();
        self.valid = (self.valid + 1).min(self.data.len());
        self.phase = (self.phase + 2.0 * PI * 0.3 / self.sr) % (2.0 * PI);
        out
    }
    pub fn clear(&mut self) {
        self.valid = 0;
    }
}

pub struct Limiter {
    envelope: f32,
    release: f32,
}
impl Limiter {
    pub fn new(sr: f32) -> Self {
        Self {
            envelope: 1.0,
            release: 1.0 - (-1.0 / (sr * 0.08)).exp(),
        }
    }
    pub fn process(&mut self, mut x: Frame) -> Frame {
        for v in &mut x {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
        let peak = x[0].abs().max(x[1].abs());
        let required = if peak > 0.89125 { 0.89125 / peak } else { 1.0 };
        if required < self.envelope {
            self.envelope = required;
        } else {
            self.envelope += (required - self.envelope) * self.release;
        }
        [x[0] * self.envelope, x[1] * self.envelope]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modulation_indices_remain_valid_across_wraparound() {
        for sr in [44100.0, 48000.0] {
            let mut m = Modulation::new(sr);
            for i in 0..2_000_000 {
                let y = m.process([0.01, 0.02], 1.0, i % 2 == 0);
                assert!(y.iter().all(|v| v.is_finite()));
            }
        }
    }
    #[test]
    fn crossfade_is_equal_power_and_isolates_edges() {
        for i in 0..=100 {
            let g = crossfade(i as f32 / 100.0);
            assert!((g[0] * g[0] + g[1] * g[1] - 1.0).abs() < 1e-6);
        }
        assert!(crossfade(0.0)[1].abs() < 1e-6);
        assert!(crossfade(1.0)[0].abs() < 1e-6);
    }
    #[test]
    fn filter_preserves_channel_isolation_and_stays_finite() {
        let mut f = Filter::new(48000.0);
        for i in 0..100_000 {
            let y = f.process(
                [if i == 0 { 1.0 } else { 0.0 }, 0.0],
                if i % 2 == 0 { 25.0 } else { 21000.0 },
                1.0,
                false,
            );
            assert!(y[0].is_finite());
            assert_eq!(y[1], 0.0);
        }
    }
    #[test]
    fn limiter_links_stereo_and_caps_samples() {
        let mut l = Limiter::new(48000.0);
        let y = l.process([4.0, 2.0]);
        assert!(y[0] <= 0.89126);
        assert!((y[0] / y[1] - 2.0).abs() < 1e-6);
        assert_eq!(l.process([f32::NAN, 0.0])[0], 0.0);
    }
    #[test]
    fn effects_have_bounded_finite_tails() {
        let mut d = Delay::new(48000.0);
        let mut r = Reverb::new(48000.0);
        for i in 0..480_000 {
            let x = if i == 0 { [0.5, 0.25] } else { [0.0; 2] };
            for y in [d.process(x, 0.2, 1.5), r.process(x, 2.0)] {
                assert!(y.iter().all(|v| v.is_finite() && v.abs() < 4.0));
            }
        }
    }
}
