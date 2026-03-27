# waveform

Rust rewrite of the original [vulakn-waveform]([https://example.com](https://github.com/The-Mazeman/vulkan-waveform)) idea, rebuilt as a fast terminal waveform explorer instead of a fragile Windows/Vulkan prototype.

## What changed

- Pure Rust implementation
- No external crate dependencies
- Fast peak-pyramid preprocessing for efficient zoom and pan
- Interactive ASCII viewer with multi-channel support
- Snapshot mode for piping or quick previews
- Release profile tuned for smaller, faster binaries

## Why this is better

The original project was tightly coupled to Win32 + Vulkan setup code. This version keeps the core goal, viewing WAV waveforms, but removes the heavyweight platform stack and replaces it with:

- Manual WAV parsing for PCM and float WAV files
- Multi-resolution min/max aggregation inspired by waveform mipmaps
- O(columns) redraw behavior for large files after preprocessing
- A cleaner architecture that is much easier to extend

## Usage

```bash
cargo run --release -- path/to/file.wav
```

Print a single static frame:

```bash
cargo run --release -- path/to/file.wav --snapshot
```

Optional flags:

- `--width <cols>`
- `--height <rows>`
- `--no-color`

## Controls

- `h` / `l`: pan left or right
- `H` / `L`: pan faster
- `+` / `-`: zoom in or out
- `[` / `]`: adjust vertical gain
- `c`: cycle channel focus
- `g`: toggle grid
- `?`: toggle help
- `0`: fit the full file
- `q`: quit

Arrow left/right also work for panning.

## Supported WAV formats

- PCM 8-bit
- PCM 16-bit
- PCM 24-bit
- PCM 32-bit
- IEEE float 32-bit
- IEEE float 64-bit
- Basic `WAVE_FORMAT_EXTENSIBLE` variants that map back to PCM or float

## Notes

- Interactive mode expects a Unix-like terminal with `stty` available.
- If stdin/stdout is not a terminal, the app falls back to a one-shot snapshot render.
