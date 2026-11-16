//! Linux audio output through PulseAudio's simple API (PipeWire serves it
//! too), loaded at run time: no PulseAudio, no sound, the video still plays.
//! Same interface as `wasapi::Output`. The clock is what was written minus
//! what the server still holds (its latency), which drives A/V sync.

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void};
use std::sync::OnceLock;

pub type Result<T> = std::result::Result<T, String>;

#[repr(C)]
struct SampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}

#[repr(C)]
struct BufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}

const PA_STREAM_PLAYBACK: c_int = 1;
const PA_SAMPLE_FLOAT32LE: c_int = 5;

type New = unsafe extern "C" fn(*const c_char, *const c_char, c_int, *const c_char, *const c_char, *const SampleSpec, *const c_void, *const BufferAttr, *mut c_int) -> *mut c_void;
type Write = unsafe extern "C" fn(*mut c_void, *const c_void, usize, *mut c_int) -> c_int;
type Latency = unsafe extern "C" fn(*mut c_void, *mut c_int) -> u64;
type Flush = unsafe extern "C" fn(*mut c_void, *mut c_int) -> c_int;
type Free = unsafe extern "C" fn(*mut c_void);

struct Api {
    new: New,
    write: Write,
    latency: Latency,
    flush: Flush,
    free: Free,
    _lib: Library,
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let lib = Library::new("libpulse-simple.so.0").ok()?;
        let new: Symbol<New> = lib.get(b"pa_simple_new\0").ok()?;
        let write: Symbol<Write> = lib.get(b"pa_simple_write\0").ok()?;
        let latency: Symbol<Latency> = lib.get(b"pa_simple_get_latency\0").ok()?;
        let flush: Symbol<Flush> = lib.get(b"pa_simple_flush\0").ok()?;
        let free: Symbol<Free> = lib.get(b"pa_simple_free\0").ok()?;
        let (new, write, latency, flush, free) = (*new, *write, *latency, *flush, *free);
        Some(Api { new, write, latency, flush, free, _lib: lib })
    })
    .as_ref()
}

/// How far ahead we keep the server's buffer.
const AHEAD_MS: u32 = 100;

pub struct Output {
    api: &'static Api,
    s: *mut c_void,
    rate: u32,
    pub channels: u16,
    /// Frames written since the last reset.
    written: u64,
    /// Last clock reported, so it never goes back.
    last: f64,
    volume: f32,
    scratch: Vec<f32>,
    /// Stopped (paused, or not started yet): written samples wait here, like
    /// in a stopped WASAPI stream. PulseAudio would play them right away.
    running: bool,
    held: Vec<f32>,
}

impl Output {
    pub fn new(rate: u32, channels: u16) -> Result<Self> {
        let api = api().ok_or("PulseAudio isn't available (libpulse-simple)")?;
        let spec = SampleSpec { format: PA_SAMPLE_FLOAT32LE, rate, channels: channels as u8 };
        let bytes = |ms: u32| (rate as u64 * channels as u64 * 4 * ms as u64 / 1000) as u32;
        let attr = BufferAttr { maxlength: u32::MAX, tlength: bytes(AHEAD_MS), prebuf: bytes(AHEAD_MS / 4), minreq: u32::MAX, fragsize: u32::MAX };
        let mut err = 0;
        let s = unsafe { (api.new)(std::ptr::null(), c"Aurora".as_ptr(), PA_STREAM_PLAYBACK, std::ptr::null(), c"Trailer".as_ptr(), &spec, std::ptr::null(), &attr, &mut err) };
        if s.is_null() {
            return Err(format!("couldn't open the audio output (PulseAudio error {err})"));
        }
        Ok(Self { api, s, rate: rate.max(1), channels, written: 0, last: 0.0, volume: 1.0, scratch: Vec::new(), running: false, held: Vec::new() })
    }

    fn latency_frames(&self) -> Result<u64> {
        let mut err = 0;
        let us = unsafe { (self.api.latency)(self.s, &mut err) };
        if us == u64::MAX {
            return Err(format!("audio output lost (PulseAudio error {err})"));
        }
        Ok(us * self.rate as u64 / 1_000_000)
    }

    /// Frames that can be written now without getting further ahead than
    /// [`AHEAD_MS`].
    pub fn free(&self) -> Result<u32> {
        let ahead = self.rate as u64 * AHEAD_MS as u64 / 1000;
        Ok(ahead.saturating_sub(self.queued()? as u64) as u32)
    }

    /// Frames written and not played yet (held ones too).
    pub fn queued(&self) -> Result<u32> {
        let held = self.held.len() / self.channels.max(1) as usize;
        Ok((self.latency_frames()?.min(self.written) + held as u64) as u32)
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<()> {
        if !self.running {
            self.held.extend(samples.iter().map(|s| s * self.volume));
            return Ok(());
        }
        self.scratch.clear();
        self.scratch.extend(samples.iter().map(|s| s * self.volume));
        self.send()
    }

    fn send(&mut self) -> Result<()> {
        let samples = std::mem::take(&mut self.scratch);
        if samples.is_empty() {
            return Ok(());
        }
        let mut err = 0;
        let r = unsafe { (self.api.write)(self.s, samples.as_ptr().cast(), samples.len() * 4, &mut err) };
        if r < 0 {
            return Err(format!("audio output lost (PulseAudio error {err})"));
        }
        self.written += (samples.len() / self.channels.max(1) as usize) as u64;
        self.scratch = samples;
        Ok(())
    }

    /// Seconds heard since the last reset.
    pub fn played(&mut self) -> Result<f64> {
        let played = self.written.saturating_sub(self.latency_frames()?) as f64 / self.rate as f64;
        self.last = self.last.max(played);
        Ok(self.last)
    }

    /// Send what was held; the server plays once enough is buffered.
    pub fn start(&mut self) -> Result<()> {
        self.running = true;
        self.scratch = std::mem::take(&mut self.held);
        let r = self.send();
        self.held = std::mem::take(&mut self.scratch);
        self.held.clear();
        r
    }

    /// Later writes wait for [`start`](Self::start); what the server already
    /// has (≤ 100 ms) plays out and counts as heard.
    pub fn stop(&mut self) -> Result<()> {
        self.running = false;
        Ok(())
    }

    /// Drop what's buffered (a seek).
    pub fn reset(&mut self) -> Result<()> {
        let mut err = 0;
        unsafe { (self.api.flush)(self.s, &mut err) };
        self.held.clear();
        self.running = false;
        self.written = 0;
        self.last = 0.0;
        Ok(())
    }

    pub fn set_volume(&mut self, v: f32) -> Result<()> {
        self.volume = v.clamp(0.0, 1.0);
        Ok(())
    }

    pub fn wait(&self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        unsafe { (self.api.free)(self.s) };
    }
}
