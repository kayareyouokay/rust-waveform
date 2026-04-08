use crate::audio::Waveform;
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};
use std::time::Duration;

pub struct AudioPlayer {
    stream: OutputStream,
    sink: Sink,
    source: SamplesBuffer,
    duration: Duration,
    volume: f32,
    speed: f32,
}

impl AudioPlayer {
    pub fn new(waveform: &Waveform) -> Result<Self, String> {
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|error| format!("failed to open the default audio output: {error}"))?;
        let source = SamplesBuffer::new(
            waveform.channel_count(),
            waveform.sample_rate,
            waveform.interleaved_samples.clone(),
        );
        let duration = source.total_duration().unwrap_or_default();
        let sink = Sink::connect_new(stream.mixer());
        sink.append(source.clone());
        sink.play();

        Ok(Self {
            stream,
            sink,
            source,
            duration,
            volume: 1.0,
            speed: 1.0,
        })
    }

    pub fn toggle(&self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    pub fn restart(&mut self) {
        self.rebuild(Duration::ZERO, true);
    }

    pub fn seek_ratio(&mut self, ratio: f32) {
        let clamped = ratio.clamp(0.0, 1.0) as f64;
        let target = Duration::from_secs_f64(self.duration.as_secs_f64() * clamped);
        self.seek(target);
    }

    pub fn seek(&mut self, position: Duration) {
        let autoplay = !self.sink.is_paused();

        if self.sink.empty() {
            self.rebuild(position, autoplay);
            return;
        }

        if self.sink.try_seek(position).is_err() {
            self.rebuild(position, autoplay);
        } else if autoplay {
            self.sink.play();
        }
    }

    pub fn skip_seconds(&mut self, delta_seconds: f32) {
        let next = self.position().as_secs_f32() + delta_seconds;
        self.seek(Duration::from_secs_f32(next.max(0.0)));
    }

    pub fn sync(&mut self, looping: bool) {
        if !self.sink.is_paused() && self.sink.empty() {
            if looping {
                self.rebuild(Duration::ZERO, true);
            } else {
                self.rebuild(self.duration, false);
            }
        }
    }

    pub fn position(&self) -> Duration {
        self.sink.get_pos().min(self.duration)
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 2.0);
        self.sink.set_volume(self.volume);
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.25, 3.0);
        self.sink.set_speed(self.speed);
    }

    fn rebuild(&mut self, position: Duration, autoplay: bool) {
        let sink = Sink::connect_new(self.stream.mixer());
        sink.append(self.source.clone());
        sink.set_volume(self.volume);
        sink.set_speed(self.speed);
        let _ = sink.try_seek(position.min(self.duration));
        if autoplay {
            sink.play();
        } else {
            sink.pause();
        }
        self.sink = sink;
    }
}
