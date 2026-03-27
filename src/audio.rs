use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

const RIFF_TAG: &[u8; 4] = b"RIFF";
const WAVE_TAG: &[u8; 4] = b"WAVE";
const FMT_TAG: &[u8; 4] = b"fmt ";
const DATA_TAG: &[u8; 4] = b"data";

const PCM_FORMAT: u16 = 0x0001;
const FLOAT_FORMAT: u16 = 0x0003;
const EXTENSIBLE_FORMAT: u16 = 0xFFFE;

const RAW_SCAN_THRESHOLD: usize = 2_048;
const BASE_CHUNK_SIZE: usize = 32;

#[derive(Clone, Copy, Debug, Default)]
pub struct Peak {
    pub min: f32,
    pub max: f32,
    pub sum_abs: f32,
    pub sample_count: u32,
}

impl Peak {
    fn from_sample(sample: f32) -> Self {
        Self {
            min: sample,
            max: sample,
            sum_abs: sample.abs(),
            sample_count: 1,
        }
    }

    fn merge(self, other: Self) -> Self {
        if self.sample_count == 0 {
            return other;
        }
        if other.sample_count == 0 {
            return self;
        }

        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
            sum_abs: self.sum_abs + other.sum_abs,
            sample_count: self.sample_count + other.sample_count,
        }
    }

    pub fn average_abs(self) -> f32 {
        if self.sample_count == 0 {
            0.0
        } else {
            self.sum_abs / self.sample_count as f32
        }
    }
}

pub struct PeakLevel {
    pub chunk_size: usize,
    pub peaks: Vec<Peak>,
}

pub struct Channel {
    pub samples: Vec<f32>,
    pub levels: Vec<PeakLevel>,
    pub peak_abs: f32,
    pub rms: f32,
}

impl Channel {
    pub fn summarize(&self, range: Range<usize>) -> Peak {
        if self.samples.is_empty() {
            return Peak::default();
        }

        let start = range.start.min(self.samples.len().saturating_sub(1));
        let end = range.end.max(start + 1).min(self.samples.len());
        let span = end - start;

        if span <= RAW_SCAN_THRESHOLD || self.levels.is_empty() {
            return summarize_samples(&self.samples[start..end]);
        }

        let target_chunk = (span / 4).max(BASE_CHUNK_SIZE);
        let level = self
            .levels
            .iter()
            .rev()
            .find(|level| level.chunk_size <= target_chunk)
            .unwrap_or(&self.levels[0]);

        let start_bucket = start / level.chunk_size;
        let end_bucket = (end.saturating_sub(1) / level.chunk_size) + 1;
        level.peaks[start_bucket..end_bucket]
            .iter()
            .copied()
            .fold(Peak::default(), Peak::merge)
    }
}

pub struct Waveform {
    pub path: PathBuf,
    pub channels: Vec<Channel>,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub frame_count: usize,
}

impl Waveform {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        parse_waveform(bytes, path)
    }

    pub fn duration_seconds(&self) -> f64 {
        self.frame_count as f64 / self.sample_rate.max(1) as f64
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

#[derive(Clone, Copy)]
struct FmtChunk {
    audio_format: u16,
    channel_count: usize,
    sample_rate: u32,
    block_align: usize,
    bits_per_sample: u16,
}

fn parse_waveform(bytes: Vec<u8>, path: &Path) -> Result<Waveform, String> {
    if bytes.len() < 12 {
        return Err("file is too small to be a WAV".to_string());
    }
    if &bytes[0..4] != RIFF_TAG || &bytes[8..12] != WAVE_TAG {
        return Err("only RIFF/WAVE files are supported".to_string());
    }

    let mut fmt = None;
    let mut data_range = None;
    let mut cursor = 12usize;

    while cursor + 8 <= bytes.len() {
        let chunk_size = read_u32(&bytes, cursor + 4)? as usize;
        let body_start = cursor + 8;
        let body_end = body_start
            .checked_add(chunk_size)
            .ok_or("WAV chunk length overflowed")?;
        if body_end > bytes.len() {
            return Err("encountered a truncated WAV chunk".to_string());
        }

        let chunk_tag = &bytes[cursor..cursor + 4];
        if chunk_tag == FMT_TAG {
            fmt = Some(parse_fmt_chunk(&bytes[body_start..body_end])?);
        } else if chunk_tag == DATA_TAG {
            data_range = Some(body_start..body_end);
        }

        cursor = body_end + (chunk_size & 1);
    }

    let fmt = fmt.ok_or("missing fmt chunk")?;
    let data_range = data_range.ok_or("missing data chunk")?;
    let frame_count = data_range.len() / fmt.block_align;
    let channels = decode_channels(&bytes[data_range.clone()], fmt)?;

    Ok(Waveform {
        path: path.to_path_buf(),
        channels,
        sample_rate: fmt.sample_rate,
        bits_per_sample: fmt.bits_per_sample,
        frame_count,
    })
}

fn parse_fmt_chunk(chunk: &[u8]) -> Result<FmtChunk, String> {
    if chunk.len() < 16 {
        return Err("fmt chunk is truncated".to_string());
    }

    let mut audio_format = read_u16(chunk, 0)?;
    let channel_count = read_u16(chunk, 2)? as usize;
    let sample_rate = read_u32(chunk, 4)?;
    let block_align = read_u16(chunk, 12)? as usize;
    let bits_per_sample = read_u16(chunk, 14)?;

    if channel_count == 0 {
        return Err("WAV declares zero channels".to_string());
    }

    if audio_format == EXTENSIBLE_FORMAT {
        if chunk.len() < 40 {
            return Err("WAVE_FORMAT_EXTENSIBLE chunk is too small".to_string());
        }
        audio_format = read_u16(chunk, 24)?;
    }

    if audio_format != PCM_FORMAT && audio_format != FLOAT_FORMAT {
        return Err(format!("unsupported WAV format tag: 0x{audio_format:04X}"));
    }

    Ok(FmtChunk {
        audio_format,
        channel_count,
        sample_rate,
        block_align,
        bits_per_sample,
    })
}

fn decode_channels(data: &[u8], fmt: FmtChunk) -> Result<Vec<Channel>, String> {
    let bytes_per_sample = match (fmt.audio_format, fmt.bits_per_sample) {
        (PCM_FORMAT, 8) => 1,
        (PCM_FORMAT, 16) => 2,
        (PCM_FORMAT, 24) => 3,
        (PCM_FORMAT, 32) => 4,
        (FLOAT_FORMAT, 32) => 4,
        (FLOAT_FORMAT, 64) => 8,
        _ => {
            return Err(format!(
                "unsupported WAV encoding: format={} bits={}",
                fmt.audio_format, fmt.bits_per_sample
            ));
        }
    };

    if fmt.block_align != bytes_per_sample * fmt.channel_count {
        return Err("unsupported block alignment for the declared sample format".to_string());
    }
    if data.len() % fmt.block_align != 0 {
        return Err("data chunk is not aligned to whole audio frames".to_string());
    }

    let frame_count = data.len() / fmt.block_align;
    let mut channels = (0..fmt.channel_count)
        .map(|_| Vec::with_capacity(frame_count))
        .collect::<Vec<_>>();

    for frame in 0..frame_count {
        let frame_offset = frame * fmt.block_align;
        for (channel_index, channel) in channels.iter_mut().enumerate() {
            let sample_offset = frame_offset + (channel_index * bytes_per_sample);
            let sample = decode_sample(
                &data[sample_offset..sample_offset + bytes_per_sample],
                fmt.audio_format,
                fmt.bits_per_sample,
            )?;
            channel.push(sample);
        }
    }

    Ok(channels
        .into_iter()
        .map(build_channel)
        .collect::<Vec<_>>())
}

fn build_channel(samples: Vec<f32>) -> Channel {
    let peak_abs = samples
        .iter()
        .fold(0.0f32, |current, sample| current.max(sample.abs()));
    let sum_squares = samples.iter().map(|sample| sample * sample).sum::<f32>();
    let rms = if samples.is_empty() {
        0.0
    } else {
        (sum_squares / samples.len() as f32).sqrt()
    };

    Channel {
        levels: build_peak_levels(&samples),
        samples,
        peak_abs,
        rms,
    }
}

fn build_peak_levels(samples: &[f32]) -> Vec<PeakLevel> {
    if samples.len() <= BASE_CHUNK_SIZE {
        return Vec::new();
    }

    let mut levels = Vec::new();
    let mut chunk_size = BASE_CHUNK_SIZE;
    let mut peaks = chunk_samples(samples, chunk_size);

    loop {
        levels.push(PeakLevel {
            chunk_size,
            peaks: peaks.clone(),
        });
        if peaks.len() <= 1 {
            break;
        }
        peaks = combine_peaks(&peaks);
        chunk_size *= 2;
    }

    levels
}

fn chunk_samples(samples: &[f32], chunk_size: usize) -> Vec<Peak> {
    samples
        .chunks(chunk_size)
        .map(summarize_samples)
        .collect::<Vec<_>>()
}

fn combine_peaks(peaks: &[Peak]) -> Vec<Peak> {
    peaks
        .chunks(2)
        .map(|pair| {
            if pair.len() == 2 {
                pair[0].merge(pair[1])
            } else {
                pair[0]
            }
        })
        .collect::<Vec<_>>()
}

fn summarize_samples(samples: &[f32]) -> Peak {
    samples
        .iter()
        .copied()
        .map(Peak::from_sample)
        .fold(Peak::default(), Peak::merge)
}

fn decode_sample(bytes: &[u8], audio_format: u16, bits_per_sample: u16) -> Result<f32, String> {
    let sample = match (audio_format, bits_per_sample) {
        (PCM_FORMAT, 8) => (bytes[0] as f32 - 128.0) / 128.0,
        (PCM_FORMAT, 16) => i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 32_768.0,
        (PCM_FORMAT, 24) => {
            let sign = if bytes[2] & 0x80 == 0 { 0 } else { 0xFF };
            let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], sign]);
            value as f32 / 8_388_608.0
        }
        (PCM_FORMAT, 32) => i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32
            / 2_147_483_648.0,
        (FLOAT_FORMAT, 32) => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        (FLOAT_FORMAT, 64) => {
            f64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]) as f32
        }
        _ => return Err("unsupported sample format".to_string()),
    };

    Ok(sample.clamp(-1.0, 1.0))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or("unexpected end of file while reading u16")?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or("unexpected end of file while reading u32")?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

#[cfg(test)]
mod tests {
    use super::{parse_waveform, Peak};
    use std::path::Path;

    #[test]
    fn parses_pcm16_wave() {
        let bytes = wav_bytes_pcm16(&[0, 8_192, -8_192, 16_384]);
        let waveform = parse_waveform(bytes, Path::new("fixture.wav")).expect("valid WAV");

        assert_eq!(waveform.channels.len(), 1);
        assert_eq!(waveform.sample_rate, 48_000);
        assert_eq!(waveform.frame_count, 4);
        assert!(waveform.channels[0].peak_abs > 0.49);
    }

    #[test]
    fn summarizer_uses_peak_levels() {
        let bytes = wav_bytes_pcm16(&[0, 1_000, -2_000, 3_000, -4_000, 5_000, -6_000, 7_000]);
        let waveform = parse_waveform(bytes, Path::new("fixture.wav")).expect("valid WAV");
        let peak = waveform.channels[0].summarize(0..waveform.frame_count);

        assert!(peak.min < -0.18);
        assert!(peak.max > 0.21);
    }

    fn wav_bytes_pcm16(samples: &[i16]) -> Vec<u8> {
        let data_size = (samples.len() * 2) as u32;
        let riff_size = 36 + data_size;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_size.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&(48_000u32 * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    #[allow(dead_code)]
    fn _merge(left: Peak, right: Peak) -> Peak {
        left.merge(right)
    }
}
