use crate::audio::Waveform;
use crate::cli::{self, Config, ParseOutcome};
use crate::gpu::{WaveformActions, WaveformProgram};
use crate::player::AudioPlayer;
use iced::alignment;
use iced::font::{Family, Stretch, Style as FontStyle, Weight};
use iced::keyboard::{self, Key, key};
use iced::theme::Palette;
use iced::widget::{self, button, column, container, horizontal_space, row, slider, text};
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Subscription, Task, Theme,
};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

const SEEK_STEP_SECONDS: f32 = 5.0;
const PAN_STEP_VIEWPORTS: f32 = 0.18;
const ZOOM_STEP_LINES: f32 = 1.0;

const HEADER_HEIGHT: f32 = 52.0;
const FOOTER_HEIGHT: f32 = 24.0;
const SIDEBAR_WIDTH: f32 = 280.0;
const BUTTON_HEIGHT: f32 = 36.0;
const BUTTON_LABEL_SIZE: f32 = 12.0;
const BUTTON_ICON_SIZE: f32 = 15.0;

const WINDOW_BG: Color = Color::from_rgb(13.0 / 255.0, 13.0 / 255.0, 15.0 / 255.0);
const SURFACE_BG: Color = Color::from_rgb(19.0 / 255.0, 19.0 / 255.0, 23.0 / 255.0);
const RAISED_BG: Color = Color::from_rgb(26.0 / 255.0, 26.0 / 255.0, 32.0 / 255.0);
const ACTIVE_BUTTON_BG: Color = Color::from_rgb(30.0 / 255.0, 28.0 / 255.0, 20.0 / 255.0);
const ACCENT_AMBER: Color = Color::from_rgb(200.0 / 255.0, 169.0 / 255.0, 110.0 / 255.0);
const ACCENT_BLUE: Color = Color::from_rgb(74.0 / 255.0, 158.0 / 255.0, 1.0);
const TEXT_PRIMARY: Color = Color::from_rgb(232.0 / 255.0, 228.0 / 255.0, 220.0 / 255.0);
const TEXT_SECONDARY: Color = Color::from_rgb(107.0 / 255.0, 104.0 / 255.0, 96.0 / 255.0);
const TEXT_HOVER: Color = Color::from_rgb(200.0 / 255.0, 196.0 / 255.0, 188.0 / 255.0);
const BORDER_COLOR: Color = Color::from_rgb(34.0 / 255.0, 34.0 / 255.0, 40.0 / 255.0);
const BORDER_HOVER: Color = Color::from_rgb(58.0 / 255.0, 58.0 / 255.0, 66.0 / 255.0);
const STATUS_DANGER: Color = Color::from_rgb(200.0 / 255.0, 75.0 / 255.0, 75.0 / 255.0);
const REMIX_ICON_FAMILY: &str = "remixicon";

static REMIX_ICON_AVAILABLE: LazyLock<bool> = LazyLock::new(|| {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fonts/remixicon.ttf")
        .is_file()
});

pub fn run() -> Result<(), String> {
    match cli::parse()? {
        ParseOutcome::Exit(message) => {
            println!("{message}");
            Ok(())
        }
        ParseOutcome::Run(config) => run_gui(config),
    }
}

fn run_gui(config: Config) -> Result<(), String> {
    let state = WaveformApp::new(config)?;

    let application = iced::application(
        |state: &WaveformApp| format!("waveform :: {}", state.waveform.file_name()),
        update,
        view,
    )
    .theme(|_| custom_theme())
    .default_font(ui_font())
    .subscription(subscription)
    .window_size(iced::Size::new(1320.0, 860.0))
    .antialiasing(true)
    .centered();

    let application = optional_font_bytes()
        .into_iter()
        .fold(application, |application, bytes| application.font(bytes));

    application
        .run_with(move || (state, Task::none()))
        .map_err(|error| format!("failed to launch GUI: {error}"))
}

fn custom_theme() -> Theme {
    Theme::custom(
        "Industrial Waveform".to_string(),
        Palette {
            background: WINDOW_BG,
            text: TEXT_PRIMARY,
            primary: ACCENT_AMBER,
            success: ACCENT_BLUE,
            danger: STATUS_DANGER,
        },
    )
}

fn mono_font() -> Font {
    Font {
        family: Family::Name(preferred_mono_font_family()),
        weight: Weight::Normal,
        stretch: Stretch::Normal,
        style: FontStyle::Normal,
    }
}

fn ui_font() -> Font {
    Font {
        family: Family::Name(preferred_ui_font_family()),
        weight: Weight::Medium,
        stretch: Stretch::Condensed,
        style: FontStyle::Normal,
    }
}

fn icon_font() -> Font {
    Font {
        family: Family::Name(REMIX_ICON_FAMILY),
        weight: Weight::Normal,
        stretch: Stretch::Normal,
        style: FontStyle::Normal,
    }
}

fn italic_ui_font() -> Font {
    Font {
        style: FontStyle::Italic,
        ..ui_font()
    }
}

fn optional_font_bytes() -> Vec<Vec<u8>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    [
        manifest_dir.join("fonts/JetBrainsMono-Regular.ttf"),
        manifest_dir.join("fonts/FiraCode-Regular.ttf"),
        manifest_dir.join("fonts/IBMPlexSansCondensed-Regular.ttf"),
        manifest_dir.join("fonts/BarlowCondensed-Regular.ttf"),
        manifest_dir.join("fonts/remixicon.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/firacode/FiraCode-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/opentype/ibm-plex/IBMPlexSansCondensed-Regular.otf"),
        PathBuf::from("/usr/share/fonts/truetype/ibm-plex/IBMPlexSansCondensed-Regular.ttf"),
    ]
    .into_iter()
    .filter_map(|path| fs::read(path).ok())
    .collect()
}

fn preferred_mono_font_family() -> &'static str {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    if [
        manifest_dir.join("fonts/JetBrainsMono-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf"),
    ]
    .into_iter()
    .any(|path| path.is_file())
    {
        "JetBrains Mono"
    } else {
        "Fira Code"
    }
}

fn preferred_ui_font_family() -> &'static str {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    if [
        manifest_dir.join("fonts/IBMPlexSansCondensed-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/opentype/ibm-plex/IBMPlexSansCondensed-Regular.otf"),
        PathBuf::from("/usr/share/fonts/truetype/ibm-plex/IBMPlexSansCondensed-Regular.ttf"),
    ]
    .into_iter()
    .any(|path| path.is_file())
    {
        "IBM Plex Sans Condensed"
    } else {
        "Barlow Condensed"
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    TogglePlayback,
    Restart,
    GainChanged(f32),
    VolumeChanged(f32),
    SpeedChanged(f32),
    SeekRequested(f32),
    SkipSeconds(f32),
    ToggleLoop,
    ToggleFollow,
    FitViewport,
    PanViewport(f32),
    ZoomViewport(f32, f32),
}

pub struct WaveformApp {
    waveform: Arc<Waveform>,
    player: Option<AudioPlayer>,
    timeline: TimelineView,
    gain: f32,
    volume: f32,
    speed: f32,
    loop_playback: bool,
    follow_playhead: bool,
    playhead_ratio: f32,
    position: Duration,
    status: Option<String>,
}

impl WaveformApp {
    fn new(config: Config) -> Result<Self, String> {
        let waveform = Arc::new(Waveform::load(&config.path)?);

        let (player, status, volume, speed) = match AudioPlayer::new(&waveform) {
            Ok(player) => {
                let volume = player.volume();
                let speed = player.speed();
                (Some(player), None, volume, speed)
            }
            Err(error) => (None, Some(error), 1.0, 1.0),
        };

        let mut app = Self {
            waveform,
            player,
            timeline: TimelineView::default(),
            gain: 1.0,
            volume,
            speed,
            loop_playback: false,
            follow_playhead: true,
            playhead_ratio: 0.0,
            position: Duration::ZERO,
            status,
        };
        app.refresh_transport();
        Ok(app)
    }

    fn refresh_transport(&mut self) {
        let Some(player) = self.player.as_mut() else {
            return;
        };

        player.sync(self.loop_playback);
        self.position = player.position();
        let total = player.duration().as_secs_f32();
        self.playhead_ratio = if total <= f32::EPSILON {
            0.0
        } else {
            (self.position.as_secs_f32() / total).clamp(0.0, 1.0)
        };

        if self.follow_playhead {
            self.timeline
                .keep_in_view(self.playhead_ratio, 0.24, self.min_view_span());
        }
    }

    fn play_label(&self) -> &'static str {
        match self.player.as_ref() {
            Some(player) if !player.is_paused() => "PAUSE",
            _ => "PLAY",
        }
    }

    fn total_duration(&self) -> Duration {
        self.player
            .as_ref()
            .map(AudioPlayer::duration)
            .unwrap_or_else(|| Duration::from_secs_f64(self.waveform.duration_seconds()))
    }

    fn current_position_label(&self) -> String {
        format_duration(self.position)
    }

    fn total_duration_label(&self) -> String {
        format!("/ {}", format_duration(self.total_duration()))
    }

    fn visible_range(&self) -> (Duration, Duration) {
        let total = self.total_duration();
        let start = total.mul_f32(self.timeline.start_ratio);
        let end = total.mul_f32(self.timeline.end_ratio());
        (start, end)
    }

    fn visible_range_label(&self) -> String {
        let (start, end) = self.visible_range();
        format!("{}–{}", format_duration(start), format_duration(end))
    }

    fn min_view_span(&self) -> f32 {
        let bins = self.waveform.peak_bin_count.max(32) as f32;
        (12.0 / bins).clamp(0.001, 1.0)
    }

    fn seek_visible_ratio(&mut self, local_ratio: f32) {
        let target_ratio = self.timeline.absolute_ratio(local_ratio);
        if let Some(player) = self.player.as_mut() {
            player.seek_ratio(target_ratio);
        }
        self.playhead_ratio = target_ratio;
        self.position =
            Duration::from_secs_f64(self.waveform.duration_seconds() * target_ratio as f64);
        if self.follow_playhead {
            self.timeline
                .keep_in_view(self.playhead_ratio, 0.24, self.min_view_span());
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TimelineView {
    start_ratio: f32,
    span_ratio: f32,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            start_ratio: 0.0,
            span_ratio: 1.0,
        }
    }
}

impl TimelineView {
    fn end_ratio(self) -> f32 {
        (self.start_ratio + self.span_ratio).clamp(0.0, 1.0)
    }

    fn fit(&mut self) {
        *self = Self::default();
    }

    fn absolute_ratio(self, local_ratio: f32) -> f32 {
        (self.start_ratio + local_ratio.clamp(0.0, 1.0) * self.span_ratio).clamp(0.0, 1.0)
    }

    fn pan(&mut self, delta_viewports: f32, min_span: f32) {
        self.span_ratio = self.span_ratio.clamp(min_span, 1.0);
        let max_start = (1.0 - self.span_ratio).max(0.0);
        self.start_ratio =
            (self.start_ratio + delta_viewports * self.span_ratio).clamp(0.0, max_start);
    }

    fn zoom_around(&mut self, anchor: f32, factor: f32, min_span: f32) {
        let anchor = anchor.clamp(0.0, 1.0);
        let anchor_ratio = self.absolute_ratio(anchor);
        let next_span = (self.span_ratio * factor).clamp(min_span, 1.0);
        let max_start = (1.0 - next_span).max(0.0);
        self.start_ratio = (anchor_ratio - next_span * anchor).clamp(0.0, max_start);
        self.span_ratio = next_span;
    }

    fn keep_in_view(&mut self, absolute_ratio: f32, padding: f32, min_span: f32) {
        let absolute_ratio = absolute_ratio.clamp(0.0, 1.0);
        let padding = padding.clamp(0.0, 0.49);
        let left = self.start_ratio + self.span_ratio * padding;
        let right = self.end_ratio() - self.span_ratio * padding;

        if absolute_ratio < left {
            self.start_ratio -= left - absolute_ratio;
        } else if absolute_ratio > right {
            self.start_ratio += absolute_ratio - right;
        }

        self.span_ratio = self.span_ratio.clamp(min_span, 1.0);
        let max_start = (1.0 - self.span_ratio).max(0.0);
        self.start_ratio = self.start_ratio.clamp(0.0, max_start);
    }
}

fn update(state: &mut WaveformApp, message: Message) -> Task<Message> {
    match message {
        Message::Tick => state.refresh_transport(),
        Message::TogglePlayback => {
            if let Some(player) = state.player.as_ref() {
                player.toggle();
            }
            state.refresh_transport();
        }
        Message::Restart => {
            if let Some(player) = state.player.as_mut() {
                player.restart();
            }
            state.playhead_ratio = 0.0;
            state.position = Duration::ZERO;
            if state.follow_playhead {
                state
                    .timeline
                    .keep_in_view(0.0, 0.24, state.min_view_span());
            }
        }
        Message::GainChanged(gain) => state.gain = gain,
        Message::VolumeChanged(volume) => {
            state.volume = volume;
            if let Some(player) = state.player.as_mut() {
                player.set_volume(volume);
            }
        }
        Message::SpeedChanged(speed) => {
            state.speed = speed;
            if let Some(player) = state.player.as_mut() {
                player.set_speed(speed);
            }
            state.refresh_transport();
        }
        Message::SeekRequested(local_ratio) => state.seek_visible_ratio(local_ratio),
        Message::SkipSeconds(delta_seconds) => {
            if let Some(player) = state.player.as_mut() {
                player.skip_seconds(delta_seconds);
            }
            state.refresh_transport();
        }
        Message::ToggleLoop => state.loop_playback = !state.loop_playback,
        Message::ToggleFollow => {
            state.follow_playhead = !state.follow_playhead;
            if state.follow_playhead {
                state
                    .timeline
                    .keep_in_view(state.playhead_ratio, 0.24, state.min_view_span());
            }
        }
        Message::FitViewport => state.timeline.fit(),
        Message::PanViewport(delta_viewports) => {
            state.follow_playhead = false;
            state.timeline.pan(delta_viewports, state.min_view_span());
        }
        Message::ZoomViewport(anchor, lines) => {
            let lines = lines.clamp(-8.0, 8.0);
            let factor = 0.82_f32.powf(lines);
            state
                .timeline
                .zoom_around(anchor, factor, state.min_view_span());
        }
    }

    Task::none()
}

fn subscription(_state: &WaveformApp) -> Subscription<Message> {
    Subscription::batch([
        iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick),
        keyboard::on_key_press(shortcut),
    ])
}

fn shortcut(key: Key, modifiers: keyboard::Modifiers) -> Option<Message> {
    match key.as_ref() {
        Key::Named(key::Named::Space) => Some(Message::TogglePlayback),
        Key::Named(key::Named::Home) => Some(Message::Restart),
        Key::Named(key::Named::ArrowLeft) if modifiers.shift() => {
            Some(Message::PanViewport(-PAN_STEP_VIEWPORTS))
        }
        Key::Named(key::Named::ArrowRight) if modifiers.shift() => {
            Some(Message::PanViewport(PAN_STEP_VIEWPORTS))
        }
        Key::Named(key::Named::ArrowLeft) => Some(Message::SkipSeconds(-SEEK_STEP_SECONDS)),
        Key::Named(key::Named::ArrowRight) => Some(Message::SkipSeconds(SEEK_STEP_SECONDS)),
        Key::Named(key::Named::ArrowUp) => Some(Message::ZoomViewport(0.5, ZOOM_STEP_LINES)),
        Key::Named(key::Named::ArrowDown) => Some(Message::ZoomViewport(0.5, -ZOOM_STEP_LINES)),
        Key::Character("=") | Key::Character("+") => {
            Some(Message::ZoomViewport(0.5, ZOOM_STEP_LINES))
        }
        Key::Character("-") | Key::Character("_") => {
            Some(Message::ZoomViewport(0.5, -ZOOM_STEP_LINES))
        }
        Key::Character("[") => Some(Message::PanViewport(-PAN_STEP_VIEWPORTS)),
        Key::Character("]") => Some(Message::PanViewport(PAN_STEP_VIEWPORTS)),
        Key::Character("0") => Some(Message::FitViewport),
        Key::Character("f") | Key::Character("F") => Some(Message::ToggleFollow),
        Key::Character("l") | Key::Character("L") => Some(Message::ToggleLoop),
        Key::Character("j") | Key::Character("J") => Some(Message::SkipSeconds(-SEEK_STEP_SECONDS)),
        Key::Character("k") | Key::Character("K") => Some(Message::SkipSeconds(SEEK_STEP_SECONDS)),
        _ => None,
    }
}

fn view(state: &WaveformApp) -> Element<'_, Message> {
    let peak = state
        .waveform
        .channels
        .iter()
        .map(|channel| channel.peak_abs)
        .fold(0.0f32, f32::max)
        * 100.0;

    let mean_rms = if state.waveform.channels.is_empty() {
        0.0
    } else {
        state
            .waveform
            .channels
            .iter()
            .map(|channel| channel.rms)
            .sum::<f32>()
            / state.waveform.channels.len() as f32
    };

    let rms_label = if mean_rms <= f32::EPSILON {
        "-INF".to_string()
    } else {
        format!("{:.0}dB", 20.0 * mean_rms.log10())
    };
    let file_name = state.waveform.file_name();
    let visible_range_label = state.visible_range_label();
    let sidebar_visible_range_label = visible_range_label.clone();
    let footer_visible_range_label = visible_range_label.clone();

    let header_left = row![
        text("[WV]").font(mono_font()).size(13).color(ACCENT_AMBER),
        text("·").font(mono_font()).size(12).color(TEXT_SECONDARY),
        text(truncate_filename(&file_name, 40))
            .font(mono_font())
            .size(12)
            .color(TEXT_PRIMARY),
        text("·").font(mono_font()).size(12).color(TEXT_SECONDARY),
        text(format!("{}ch", state.waveform.channel_count()))
            .font(mono_font())
            .size(12)
            .color(TEXT_SECONDARY),
        text("·").font(mono_font()).size(12).color(TEXT_SECONDARY),
        text(format!("{} Hz", state.waveform.sample_rate))
            .font(mono_font())
            .size(12)
            .color(TEXT_SECONDARY),
        text("·").font(mono_font()).size(12).color(TEXT_SECONDARY),
        text(format!("{}-bit", state.waveform.bits_per_sample))
            .font(mono_font())
            .size(12)
            .color(TEXT_SECONDARY),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let header_right = row![
        text(format!("PEAK {:.0}%", peak.clamp(0.0, 100.0)))
            .font(mono_font())
            .size(12)
            .color(TEXT_SECONDARY),
        text("·").font(mono_font()).size(12).color(TEXT_SECONDARY),
        text(format!("RMS {rms_label}"))
            .font(mono_font())
            .size(12)
            .color(TEXT_SECONDARY),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let header = column![
        container(row![header_left, horizontal_space(), header_right].align_y(Alignment::Center))
            .padding([0, 14])
            .center_y(Length::Fixed(HEADER_HEIGHT - 1.0))
            .width(Length::Fill),
        horizontal_rule(),
    ]
    .width(Length::Fill)
    .height(Length::Fixed(HEADER_HEIGHT));

    let transport_panel = panel(
        "TRANSPORT",
        column![
            row![
                sidebar_button(
                    state.play_label(),
                    if matches!(state.player.as_ref(), Some(player) if !player.is_paused()) {
                        ButtonIcon::Pause
                    } else {
                        ButtonIcon::Play
                    },
                    Message::TogglePlayback,
                    ButtonVariant::Standard,
                ),
                sidebar_button(
                    "RESTART",
                    ButtonIcon::Restart,
                    Message::Restart,
                    ButtonVariant::Standard,
                ),
            ]
            .spacing(6),
            row![
                sidebar_button(
                    "BACK 5S",
                    ButtonIcon::SkipBack,
                    Message::SkipSeconds(-SEEK_STEP_SECONDS),
                    ButtonVariant::Standard,
                ),
                sidebar_button(
                    "FWD 5S",
                    ButtonIcon::SkipForward,
                    Message::SkipSeconds(SEEK_STEP_SECONDS),
                    ButtonVariant::Standard,
                ),
            ]
            .spacing(6),
            row![
                sidebar_button(
                    "LOOP",
                    ButtonIcon::Loop,
                    Message::ToggleLoop,
                    if state.loop_playback {
                        ButtonVariant::LoopActive
                    } else {
                        ButtonVariant::Standard
                    },
                ),
                sidebar_button(
                    "FOLLOW",
                    ButtonIcon::Follow,
                    Message::ToggleFollow,
                    if state.follow_playhead {
                        ButtonVariant::FollowActive
                    } else {
                        ButtonVariant::Standard
                    },
                ),
            ]
            .spacing(6),
        ]
        .spacing(6)
        .into(),
    );

    let timeline_panel = panel(
        "TIMELINE",
        column![
            row![
                sidebar_button(
                    "PAN LEFT",
                    ButtonIcon::PanLeft,
                    Message::PanViewport(-PAN_STEP_VIEWPORTS),
                    ButtonVariant::Standard,
                ),
                sidebar_button(
                    "PAN RIGHT",
                    ButtonIcon::PanRight,
                    Message::PanViewport(PAN_STEP_VIEWPORTS),
                    ButtonVariant::Standard,
                ),
            ]
            .spacing(6),
            row![
                sidebar_button(
                    "ZOOM OUT",
                    ButtonIcon::ZoomOut,
                    Message::ZoomViewport(0.5, -ZOOM_STEP_LINES),
                    ButtonVariant::Standard,
                ),
                sidebar_button(
                    "ZOOM IN",
                    ButtonIcon::ZoomIn,
                    Message::ZoomViewport(0.5, ZOOM_STEP_LINES),
                    ButtonVariant::Standard,
                ),
            ]
            .spacing(6),
            sidebar_button(
                "FIT",
                ButtonIcon::Fit,
                Message::FitViewport,
                ButtonVariant::Standard,
            ),
            text(sidebar_visible_range_label)
                .font(mono_font())
                .size(10)
                .color(TEXT_SECONDARY),
        ]
        .spacing(6)
        .into(),
    );

    let gain_panel = panel(
        "GAIN",
        column![
            slider_row("GAIN", 0.5..=4.0, state.gain, 0.05, Message::GainChanged),
            slider_row("VOL", 0.0..=2.0, state.volume, 0.02, Message::VolumeChanged),
            slider_row("SPEED", 0.5..=2.0, state.speed, 0.05, Message::SpeedChanged),
        ]
        .spacing(8)
        .into(),
    );

    let position_panel = panel(
        "POSITION",
        column![
            text(state.current_position_label())
                .font(mono_font())
                .size(22)
                .color(TEXT_PRIMARY),
            text(state.total_duration_label())
                .font(mono_font())
                .size(12)
                .color(TEXT_SECONDARY),
        ]
        .spacing(4)
        .into(),
    );

    let waveform = widget::shader(WaveformProgram::new(
        Arc::clone(&state.waveform),
        state.gain,
        state.playhead_ratio,
        state.timeline.start_ratio,
        state.timeline.end_ratio(),
        WaveformActions {
            on_seek: Message::SeekRequested,
            on_pan: Message::PanViewport,
            on_zoom: Message::ZoomViewport,
        },
    ))
    .width(Length::Fill)
    .height(Length::Fill);

    let mut sidebar_column = column![transport_panel, timeline_panel, gain_panel, position_panel]
        .spacing(10)
        .width(Length::Fill);

    if let Some(status) = &state.status {
        sidebar_column = sidebar_column.push(
            container(
                text(status.as_str())
                    .font(italic_ui_font())
                    .size(11)
                    .color(STATUS_DANGER),
            )
            .width(Length::Fill),
        );
    }

    sidebar_column = sidebar_column.push(widget::Space::with_height(Length::Fill));

    let canvas = container(waveform)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| window_style());

    let sidebar = container(sidebar_column)
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Length::Fill)
        .padding(10)
        .style(|_| sidebar_style());

    let content_row = row![canvas, sidebar]
        .spacing(0)
        .height(Length::Fill)
        .width(Length::Fill);

    let footer_left = text(format!(
        "bins: {} · frames/bin: {}",
        state.waveform.peak_bin_count, state.waveform.peak_chunk_size
    ))
    .font(mono_font())
    .size(10)
    .color(TEXT_SECONDARY)
    .width(Length::Fill);

    let footer_center = text(footer_visible_range_label)
        .font(mono_font())
        .size(10)
        .color(TEXT_SECONDARY)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center);

    let footer_right = text("drag: seek · right-drag: pan · scroll: zoom")
        .font(mono_font())
        .size(10)
        .color(TEXT_SECONDARY)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Right);

    let footer = column![
        horizontal_rule(),
        container(
            row![
                container(footer_left).width(Length::FillPortion(2)),
                container(footer_center).width(Length::FillPortion(2)),
                container(footer_right).width(Length::FillPortion(3)),
            ]
            .align_y(Alignment::Center)
            .spacing(8)
        )
        .padding([0, 10])
        .center_y(Length::Fixed(FOOTER_HEIGHT - 1.0))
        .width(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fixed(FOOTER_HEIGHT));

    container(column![header, content_row, footer])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| window_style())
        .into()
}

fn panel<'a>(label: &str, content: Element<'a, Message>) -> Element<'a, Message> {
    container(column![
        horizontal_rule(),
        container(
            text(label.to_owned())
                .font(ui_font())
                .size(10)
                .color(TEXT_SECONDARY)
        )
        .padding([5, 10])
        .width(Length::Fill),
        container(content).padding([8, 10]).width(Length::Fill),
    ])
    .width(Length::Fill)
    .style(|_| panel_style())
    .into()
}

fn slider_row(
    label: &'static str,
    range: std::ops::RangeInclusive<f32>,
    value: f32,
    step: f32,
    on_change: fn(f32) -> Message,
) -> Element<'static, Message> {
    row![
        container(text(label).font(mono_font()).size(11).color(TEXT_SECONDARY))
            .width(Length::Fixed(40.0)),
        slider(range, value, on_change)
            .step(step)
            .height(16)
            .style(industry_slider_style),
        container(
            text(format!("{value:.2}"))
                .font(mono_font())
                .size(11)
                .color(ACCENT_AMBER)
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Right)
        )
        .width(Length::Fixed(36.0)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn sidebar_button(
    label: &'static str,
    icon: ButtonIcon,
    message: Message,
    variant: ButtonVariant,
) -> Element<'static, Message> {
    button(
        container(button_label(label, icon, variant))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fixed(BUTTON_HEIGHT))
    .padding([0, 10])
    .style(move |_theme, status| button_style(status, variant))
    .on_press(message)
    .into()
}

fn horizontal_rule() -> Element<'static, Message> {
    container(widget::Space::with_height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(|_| separator_style())
        .into()
}

fn window_style() -> container::Style {
    container::Style::default()
        .background(WINDOW_BG)
        .color(TEXT_PRIMARY)
}

fn sidebar_style() -> container::Style {
    container::Style::default()
        .background(WINDOW_BG)
        .border(Border {
            color: BORDER_COLOR,
            width: 1.0,
            radius: 0.0.into(),
        })
        .color(TEXT_PRIMARY)
}

fn panel_style() -> container::Style {
    container::Style::default()
        .background(SURFACE_BG)
        .border(Border {
            color: BORDER_COLOR,
            width: 1.0,
            radius: 0.0.into(),
        })
        .color(TEXT_PRIMARY)
}

fn separator_style() -> container::Style {
    container::Style::default().background(BORDER_COLOR)
}

#[derive(Debug, Clone, Copy)]
enum ButtonVariant {
    Standard,
    LoopActive,
    FollowActive,
}

#[derive(Debug, Clone, Copy)]
enum ButtonIcon {
    Play,
    Pause,
    Restart,
    SkipBack,
    SkipForward,
    Loop,
    Follow,
    PanLeft,
    PanRight,
    ZoomOut,
    ZoomIn,
    Fit,
}

fn button_style(status: button::Status, variant: ButtonVariant) -> button::Style {
    let (background, border_color, text_color, radius) = match variant {
        ButtonVariant::Standard => (RAISED_BG, BORDER_COLOR, TEXT_PRIMARY, 0.0),
        ButtonVariant::LoopActive => (ACCENT_AMBER, ACCENT_AMBER, WINDOW_BG, 14.0),
        ButtonVariant::FollowActive => (ACCENT_BLUE, ACCENT_BLUE, WINDOW_BG, 14.0),
    };

    let mut style = button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: radius.into(),
        },
        shadow: Default::default(),
    };

    match status {
        button::Status::Active => style,
        button::Status::Hovered => {
            if matches!(variant, ButtonVariant::Standard) {
                style.border.color = BORDER_HOVER;
                style.text_color = TEXT_HOVER;
            }
            style
        }
        button::Status::Pressed => {
            if matches!(variant, ButtonVariant::Standard) {
                style.background = Some(Background::Color(ACTIVE_BUTTON_BG));
                style.border.color = ACCENT_AMBER;
                style.text_color = ACCENT_AMBER;
            }
            style
        }
        button::Status::Disabled => {
            style.background = Some(Background::Color(background.scale_alpha(0.6)));
            style.border.color = border_color.scale_alpha(0.6);
            style.text_color = text_color.scale_alpha(0.6);
            style
        }
    }
}

fn button_label(
    label: &'static str,
    icon: ButtonIcon,
    variant: ButtonVariant,
) -> Element<'static, Message> {
    let (icon_color, label_color) = button_content_colors(variant);
    let icon = if *REMIX_ICON_AVAILABLE {
        text(remix_icon_glyph(icon).to_string())
            .font(icon_font())
            .size(BUTTON_ICON_SIZE)
            .color(icon_color)
    } else {
        text(fallback_icon_glyph(icon))
            .font(ui_font())
            .size(BUTTON_LABEL_SIZE)
            .color(icon_color)
    };

    row![
        icon,
        text(label)
            .font(ui_font())
            .size(BUTTON_LABEL_SIZE)
            .color(label_color),
    ]
    .spacing(5)
    .align_y(Alignment::Center)
    .into()
}

fn button_content_colors(variant: ButtonVariant) -> (Color, Color) {
    match variant {
        ButtonVariant::Standard => (ACCENT_AMBER, TEXT_PRIMARY),
        ButtonVariant::LoopActive | ButtonVariant::FollowActive => (WINDOW_BG, WINDOW_BG),
    }
}

fn remix_icon_glyph(icon: ButtonIcon) -> char {
    match icon {
        ButtonIcon::Play => '\u{f00a}',
        ButtonIcon::Pause => '\u{efd7}',
        ButtonIcon::Restart => '\u{f080}',
        ButtonIcon::SkipBack => '\u{f363}',
        ButtonIcon::SkipForward => '\u{f365}',
        ButtonIcon::Loop => '\u{f33d}',
        ButtonIcon::Follow => '\u{f0bd}',
        ButtonIcon::PanLeft => '\u{ea60}',
        ButtonIcon::PanRight => '\u{ea6c}',
        ButtonIcon::ZoomOut => '\u{f2dd}',
        ButtonIcon::ZoomIn => '\u{f2db}',
        ButtonIcon::Fit => '\u{ed4c}',
    }
}

fn fallback_icon_glyph(icon: ButtonIcon) -> &'static str {
    match icon {
        ButtonIcon::Play => "▶",
        ButtonIcon::Pause => "‖",
        ButtonIcon::Restart => "↺",
        ButtonIcon::SkipBack => "‹‹",
        ButtonIcon::SkipForward => "››",
        ButtonIcon::Loop => "⟲",
        ButtonIcon::Follow => "◎",
        ButtonIcon::PanLeft => "←",
        ButtonIcon::PanRight => "→",
        ButtonIcon::ZoomOut => "−",
        ButtonIcon::ZoomIn => "+",
        ButtonIcon::Fit => "□",
    }
}

fn industry_slider_style(_theme: &Theme, status: slider::Status) -> slider::Style {
    let handle_color = match status {
        slider::Status::Dragged => TEXT_PRIMARY,
        slider::Status::Hovered | slider::Status::Active => ACCENT_AMBER,
    };

    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(ACCENT_AMBER),
                Background::Color(BORDER_COLOR),
            ),
            width: 4.0,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 6.0 },
            background: Background::Color(handle_color),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

fn truncate_filename(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_owned();
    }

    let visible = max_chars.saturating_sub(1);
    let truncated: String = input.chars().take(visible).collect();
    format!("{truncated}…")
}

fn format_duration(duration: Duration) -> String {
    let total_millis = duration.as_millis();
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis / 60_000) % 60;
    let seconds = (total_millis / 1_000) % 60;
    let millis = total_millis % 1_000;

    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::TimelineView;

    #[test]
    fn zoom_keeps_anchor_stable() {
        let mut timeline = TimelineView::default();
        timeline.zoom_around(0.25, 0.5, 0.01);

        assert!((timeline.start_ratio - 0.125).abs() < 0.0001);
        assert!((timeline.span_ratio - 0.5).abs() < 0.0001);
    }

    #[test]
    fn keep_in_view_shifts_window() {
        let mut timeline = TimelineView {
            start_ratio: 0.0,
            span_ratio: 0.2,
        };

        timeline.keep_in_view(0.5, 0.2, 0.01);

        assert!(timeline.start_ratio > 0.3);
        assert!(timeline.end_ratio() >= 0.5);
    }
}
