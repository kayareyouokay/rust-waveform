# waveform

`waveform` is a desktop WAV player and waveform viewer built with `iced`, inspired by the original [vulkan-waveform](https://github.com/The-Mazeman/vulkan-waveform) project.

It takes a single WAV path on the command line, decodes the file into memory, starts playback immediately when a default audio device is available, and renders a live multi-channel waveform in a native desktop window.

The waveform view is GPU-backed: the app uploads precomputed peak bins for each channel, aggregates them into screen columns in a compute pass, and draws the result in an `iced` shader widget with a live playhead.

## Features

- Native `iced` desktop UI with an industrial dark control surface
- Live WAV playback through `rodio`
- GPU-backed waveform aggregation and rendering
- Multi-channel display with live playhead, zoom, pan, fit, loop, and follow mode
- Transport controls for restart, skip, gain, volume, and playback speed
- Manual WAV decoding for PCM and float RIFF/WAVE files
- Graceful fallback when audio initialization fails: the window still opens and the waveform remains usable

## Usage

```bash
cargo run --release -- path/to/file.wav
```

Show CLI help:

```bash
cargo run -- --help
```

The CLI accepts exactly one positional WAV file. Unknown flags fail, and passing more than one file fails.

## Build Notes

This project depends on:

- `iced`
- `rodio`
- a working graphics stack supported by `wgpu`
- a default audio output device for playback

If no default audio device is available, the app still opens and renders the waveform, but transport actions remain inert and a status message is shown in the sidebar.

## Controls

- `Play` / `Pause`: toggle playback
- `Restart`: jump back to the beginning
- `Back 5S` / `Fwd 5S`: seek in fixed steps
- `Loop` and `Follow`: toggle looping and automatic viewport following
- `Gain`, `Volume`, `Speed`: adjust waveform scale and playback behavior
- Left-drag on the waveform: seek
- Right-drag on the waveform: pan the visible time window
- Mouse wheel on the waveform: zoom around the cursor
- Horizontal scroll on the waveform: pan

Keyboard shortcuts:

- `Space`: play/pause
- `Home`: restart
- `Left` / `Right` or `J` / `K`: skip backward or forward 5 seconds
- `Shift` + `Left` / `Right` or `[` / `]`: pan viewport
- `Up` / `Down` or `+` / `-`: zoom
- `0`: fit the whole file
- `F`: toggle follow-playhead
- `L`: toggle looping

## UI Notes

- The window opens at `1320x860`
- Playback position updates every 16 ms
- Follow mode keeps the playhead in view until you manually pan
- Zooming anchors around the mouse cursor when possible
- There is intentionally no file picker, playlist, spectrogram, or editing UI

## Supported WAV formats

- PCM 8-bit
- PCM 16-bit
- PCM 24-bit
- PCM 32-bit
- IEEE float 32-bit
- IEEE float 64-bit
- Basic `WAVE_FORMAT_EXTENSIBLE` variants that map back to PCM or float
