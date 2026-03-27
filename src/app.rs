use crate::audio::Waveform;
use crate::cli::{self, Config, ParseOutcome};
use crate::render::{render_frame, ViewState};
use crate::terminal::{Key, Session};
use std::io::{self, IsTerminal, Write};

pub fn run() -> Result<(), String> {
    match cli::parse()? {
        ParseOutcome::Exit(message) => {
            println!("{message}");
            Ok(())
        }
        ParseOutcome::Run(config) => run_with_config(config),
    }
}

fn run_with_config(config: Config) -> Result<(), String> {
    let waveform = Waveform::load(&config.path)?;

    if config.snapshot || !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        let width = config.width.unwrap_or(120);
        let height = config.height.unwrap_or(32);
        let state = ViewState::new(waveform.frame_count);
        print!("{}", render_frame(&waveform, &state, width, height, config.color));
        io::stdout()
            .flush()
            .map_err(|error| format!("failed to flush stdout: {error}"))?;
        return Ok(());
    }

    run_interactive(&waveform, config.color)
}

fn run_interactive(waveform: &Waveform, color: bool) -> Result<(), String> {
    let mut state = ViewState::new(waveform.frame_count);
    let mut session = Session::enter()?;

    loop {
        let (width, height) = session.size()?;
        state.clamp_to(waveform.frame_count);

        {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(b"\x1b[H")
                .map_err(|error| format!("failed to reposition the cursor: {error}"))?;
            stdout
                .write_all(render_frame(waveform, &state, width, height, color).as_bytes())
                .map_err(|error| format!("failed to draw the frame: {error}"))?;
            stdout
                .flush()
                .map_err(|error| format!("failed to flush the frame: {error}"))?;
        }

        match session.read_key()? {
            Key::Left => pan_left(&mut state, waveform.frame_count, 0.10),
            Key::Right => pan_right(&mut state, waveform.frame_count, 0.10),
            Key::Char('q') => break,
            Key::Char('h') => pan_left(&mut state, waveform.frame_count, 0.10),
            Key::Char('H') => pan_left(&mut state, waveform.frame_count, 0.50),
            Key::Char('l') => pan_right(&mut state, waveform.frame_count, 0.10),
            Key::Char('L') => pan_right(&mut state, waveform.frame_count, 0.50),
            Key::Char('+') | Key::Char('=') => zoom(&mut state, waveform.frame_count, 0.5),
            Key::Char('-') | Key::Char('_') => zoom(&mut state, waveform.frame_count, 2.0),
            Key::Char('0') => {
                state.start_frame = 0;
                state.frame_span = waveform.frame_count.max(1);
            }
            Key::Char('[') => state.gain = (state.gain * 0.8).max(0.25),
            Key::Char(']') => state.gain = (state.gain * 1.25).min(16.0),
            Key::Char('c') => cycle_focus(&mut state, waveform.channels.len()),
            Key::Char('g') => state.show_grid = !state.show_grid,
            Key::Char('?') => state.show_help = !state.show_help,
            _ => {}
        }
    }

    Ok(())
}

fn pan_left(state: &mut ViewState, total_frames: usize, ratio: f32) {
    let delta = ((state.frame_span as f32 * ratio).round() as usize).max(1);
    state.start_frame = state.start_frame.saturating_sub(delta);
    state.clamp_to(total_frames);
}

fn pan_right(state: &mut ViewState, total_frames: usize, ratio: f32) {
    let delta = ((state.frame_span as f32 * ratio).round() as usize).max(1);
    state.start_frame = state.start_frame.saturating_add(delta);
    state.clamp_to(total_frames);
}

fn zoom(state: &mut ViewState, total_frames: usize, factor: f32) {
    let center = state.start_frame.saturating_add(state.frame_span / 2);
    let next_span = ((state.frame_span as f32 * factor).round() as usize).clamp(64, total_frames.max(64));
    state.frame_span = next_span.min(total_frames.max(1));
    state.start_frame = center.saturating_sub(state.frame_span / 2);
    state.clamp_to(total_frames);
}

fn cycle_focus(state: &mut ViewState, channel_count: usize) {
    state.focus_channel = match state.focus_channel {
        None if channel_count > 0 => Some(0),
        Some(index) if index + 1 < channel_count => Some(index + 1),
        _ => None,
    };
}
