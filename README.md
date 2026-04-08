# waveform

Rust rewrite of the original [vulkan-waveform](https://github.com/The-Mazeman/vulkan-waveform) idea, now rebuilt as a desktop WAV player with `iced`.

It loads a WAV file, starts playback immediately when an audio device is available, and renders the waveform live inside the GUI. The visible waveform is processed on the GPU: the app uploads peak bins for each channel, runs a compute pass to aggregate them into screen columns, and draws the result in an `iced` shader widget with a live playhead.

## Features

- `iced` desktop GUI
- Live WAV playback through `rodio`
- GPU-backed waveform aggregation and rendering
- Multi-channel display with live playhead, zoom, pan, and follow mode
- Transport controls for skip, looping, volume, and playback speed
- Manual WAV decoding for PCM and float RIFF/WAVE files

## Usage

```bash
cargo run --release -- path/to/file.wav
```

Show CLI help:

```bash
cargo run -- --help
```

## Controls

- `Play` / `Pause`: toggle playback
- `Restart`: jump back to the beginning
- `<< 5s` / `5s >>`: seek in fixed steps
- `Loop` and `Follow`: toggle looping and automatic viewport following
- `Gain`, `Volume`, `Speed`: adjust waveform scale and playback behavior
- Left-drag on the waveform: seek
- Right-drag on the waveform: pan the visible time window
- Mouse wheel on the waveform: zoom around the cursor

Keyboard shortcuts:

- `Space`: play/pause
- `Home`: restart
- `Left` / `Right`: skip backward or forward 5 seconds
- `Shift` + `Left` / `Right` or `[` / `]`: pan viewport
- `Up` / `Down` or `+` / `-`: zoom
- `0`: fit the whole file
- `F`: toggle follow-playhead
- `L`: toggle looping

## Supported WAV formats

- PCM 8-bit
- PCM 16-bit
- PCM 24-bit
- PCM 32-bit
- IEEE float 32-bit
- IEEE float 64-bit
- Basic `WAVE_FORMAT_EXTENSIBLE` variants that map back to PCM or float
