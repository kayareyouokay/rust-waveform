use crate::audio::{Peak, Waveform};

const MIN_PLOT_HEIGHT: usize = 6;
const MIN_PLOT_WIDTH: usize = 32;
const PALETTE: &[u8] = b".:-=+*#%@";

#[derive(Clone, Copy)]
pub struct ViewState {
    pub start_frame: usize,
    pub frame_span: usize,
    pub gain: f32,
    pub focus_channel: Option<usize>,
    pub show_grid: bool,
    pub show_help: bool,
}

impl ViewState {
    pub fn new(total_frames: usize) -> Self {
        Self {
            start_frame: 0,
            frame_span: total_frames.max(1),
            gain: 1.0,
            focus_channel: None,
            show_grid: true,
            show_help: false,
        }
    }

    pub fn clamp_to(&mut self, total_frames: usize) {
        let total_frames = total_frames.max(1);
        self.frame_span = self.frame_span.clamp(1, total_frames);
        self.start_frame = self
            .start_frame
            .min(total_frames.saturating_sub(self.frame_span));
    }
}

pub fn render_frame(
    waveform: &Waveform,
    state: &ViewState,
    width: usize,
    height: usize,
    color: bool,
) -> String {
    let width = width.max(MIN_PLOT_WIDTH + 4);
    let height = height.max(MIN_PLOT_HEIGHT + 8);
    let inner_width = width.saturating_sub(2);
    let reserved_rows = if state.show_help { 8 } else { 6 };
    let plot_rows = height.saturating_sub(reserved_rows).max(MIN_PLOT_HEIGHT);

    let channels = visible_channels(waveform, state.focus_channel, plot_rows);
    let panel_height = (plot_rows / channels.len()).max(MIN_PLOT_HEIGHT);

    let mut lines = Vec::new();
    lines.push(border(width, '='));
    lines.push(colorize(
        &boxed(
            inner_width,
            &format!(
                " waveform :: rust viewer :: {} :: {:.2}s ",
                waveform.file_name(),
                waveform.duration_seconds()
            ),
        ),
        color,
        "1;36",
    ));
    lines.push(boxed(
        inner_width,
        &format!(
            " range {}  zoom {:>8} fr  channels {:>2}  {} Hz  {}-bit ",
            format_range(state.start_frame, state.frame_span, waveform.sample_rate),
            state.frame_span,
            waveform.channels.len(),
            waveform.sample_rate,
            waveform.bits_per_sample
        ),
    ));
    lines.push(border(width, '='));

    for (panel_index, channel_index) in channels.iter().enumerate() {
        let channel = &waveform.channels[*channel_index];
        let label = format!(
            " ch {:>2}  peak {:>5.1}%  rms {:>5.1}% ",
            channel_index + 1,
            channel.peak_abs * 100.0,
            channel.rms * 100.0
        );
        let plot = render_channel(
            waveform,
            state,
            *channel_index,
            inner_width,
            panel_height,
            &label,
        );
        for line in plot {
            lines.push(colorize(
                &line,
                color,
                if panel_index % 2 == 0 { "0;32" } else { "0;36" },
            ));
        }
    }

    lines.push(border(width, '='));
    lines.push(boxed(
        inner_width,
        " h/l pan  +/- zoom  [/] gain  c channel  g grid  ? help  0 fit  q quit ",
    ));

    if state.show_help {
        lines.push(boxed(
            inner_width,
            " Focus cycles across all channels and then each individual channel. ",
        ));
        lines.push(boxed(
            inner_width,
            " The renderer uses a peak pyramid, so large files stay responsive while zooming. ",
        ));
        lines.push(boxed(
            inner_width,
            " Use --snapshot to print a single frame instead of entering the interactive viewer. ",
        ));
    }

    lines.push(border(width, '='));
    format!("{}\n", lines.join("\n"))
}

fn visible_channels(
    waveform: &Waveform,
    focus_channel: Option<usize>,
    plot_rows: usize,
) -> Vec<usize> {
    if let Some(channel) = focus_channel {
        return vec![channel.min(waveform.channels.len().saturating_sub(1))];
    }

    let mut channels = (0..waveform.channels.len()).collect::<Vec<_>>();
    while channels.len() > 1 && (plot_rows / channels.len()) < MIN_PLOT_HEIGHT {
        channels.pop();
    }
    channels
}

fn render_channel(
    waveform: &Waveform,
    state: &ViewState,
    channel_index: usize,
    width: usize,
    height: usize,
    label: &str,
) -> Vec<String> {
    let mut canvas = vec![vec![b' '; width]; height];
    let channel = &waveform.channels[channel_index];
    let center = height / 2;

    if state.show_grid {
        let quarter = height / 4;
        draw_horizontal(&mut canvas, quarter, b'.');
        draw_horizontal(&mut canvas, center, b'-');
        draw_horizontal(&mut canvas, height.saturating_sub(quarter + 1), b'.');
    }

    let start_frame = state.start_frame;
    let end_frame = start_frame
        .saturating_add(state.frame_span)
        .min(waveform.frame_count);

    for column in 0..width {
        let range = frame_range_for_column(start_frame, end_frame, column, width);
        let peak = channel.summarize(range);
        draw_peak(&mut canvas, column, peak, state.gain);
    }

    write_label(&mut canvas[0], label.as_bytes());

    canvas
        .into_iter()
        .map(|row| format!("|{}|", String::from_utf8_lossy(&row)))
        .collect::<Vec<_>>()
}

fn draw_peak(canvas: &mut [Vec<u8>], column: usize, peak: Peak, gain: f32) {
    if canvas.is_empty() {
        return;
    }

    let height = canvas.len();
    let scaled_min = (peak.min * gain).clamp(-1.0, 1.0);
    let scaled_max = (peak.max * gain).clamp(-1.0, 1.0);
    let top = amplitude_to_row(scaled_max, height);
    let bottom = amplitude_to_row(scaled_min, height);
    let intensity = (peak.average_abs() * gain).clamp(0.0, 1.0);
    let glyph = intensity_glyph(intensity, scaled_min <= -0.99 || scaled_max >= 0.99);

    for row in top.min(bottom)..=top.max(bottom) {
        canvas[row][column] = glyph;
    }
}

fn draw_horizontal(canvas: &mut [Vec<u8>], row: usize, glyph: u8) {
    if row >= canvas.len() {
        return;
    }
    for cell in &mut canvas[row] {
        *cell = glyph;
    }
}

fn write_label(row: &mut [u8], label: &[u8]) {
    for (cell, byte) in row.iter_mut().zip(label.iter().copied()) {
        *cell = byte;
    }
}

fn amplitude_to_row(value: f32, height: usize) -> usize {
    let normalized = (1.0 - value) * 0.5;
    (normalized * (height.saturating_sub(1)) as f32)
        .round()
        .clamp(0.0, height.saturating_sub(1) as f32) as usize
}

fn intensity_glyph(intensity: f32, clipped: bool) -> u8 {
    if clipped {
        return b'@';
    }
    let index = (intensity * (PALETTE.len().saturating_sub(1)) as f32).round() as usize;
    PALETTE[index.min(PALETTE.len().saturating_sub(1))]
}

fn frame_range_for_column(
    start: usize,
    end: usize,
    column: usize,
    width: usize,
) -> std::ops::Range<usize> {
    let span = end.saturating_sub(start).max(1);
    let range_start = start + (span * column) / width.max(1);
    let mut range_end = start + (span * (column + 1)) / width.max(1);
    if range_end <= range_start {
        range_end = range_start + 1;
    }
    range_start..range_end
}

fn border(width: usize, fill: char) -> String {
    let inner = fill.to_string().repeat(width.saturating_sub(2));
    format!("+{inner}+")
}

fn boxed(width: usize, content: &str) -> String {
    let mut trimmed = content.to_string();
    if trimmed.len() > width {
        trimmed.truncate(width);
    }
    let padding = width.saturating_sub(trimmed.len());
    format!("|{trimmed}{}|", " ".repeat(padding))
}

fn colorize(line: &str, enabled: bool, code: &str) -> String {
    if enabled {
        format!("\x1b[{code}m{line}\x1b[0m")
    } else {
        line.to_string()
    }
}

fn format_range(start_frame: usize, frame_span: usize, sample_rate: u32) -> String {
    let start_seconds = start_frame as f64 / sample_rate.max(1) as f64;
    let end_seconds = start_seconds + frame_span as f64 / sample_rate.max(1) as f64;
    format!(
        "{} -> {}",
        format_seconds(start_seconds),
        format_seconds(end_seconds)
    )
}

fn format_seconds(seconds: f64) -> String {
    let total_millis = (seconds * 1_000.0).round() as u64;
    let minutes = total_millis / 60_000;
    let seconds = (total_millis % 60_000) / 1_000;
    let millis = total_millis % 1_000;
    format!("{minutes:02}:{seconds:02}.{millis:03}")
}
