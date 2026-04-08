use std::env;
use std::path::PathBuf;

pub struct Config {
    pub path: PathBuf,
}

pub enum ParseOutcome {
    Run(Config),
    Exit(String),
}

pub fn parse() -> Result<ParseOutcome, String> {
    let args = env::args().skip(1);
    let mut path = None;

    for argument in args {
        match argument.as_str() {
            "-h" | "--help" => return Ok(ParseOutcome::Exit(help_text())),
            _ if argument.starts_with('-') => return Err(format!("unknown flag: {argument}")),
            _ => {
                if path.is_some() {
                    return Err("only one input file can be opened at a time".to_string());
                }
                path = Some(PathBuf::from(argument));
            }
        }
    }

    let Some(path) = path else {
        return Ok(ParseOutcome::Exit(help_text()));
    };

    Ok(ParseOutcome::Run(Config { path }))
}

fn help_text() -> String {
    [
        "waveform",
        "",
        "Cross-platform WAV player with an iced GUI and GPU-backed waveform rendering.",
        "",
        "USAGE:",
        "  waveform <file.wav>",
        "",
        "NOTES:",
        "  - Playback starts automatically when a default audio device is available.",
        "  - Left-drag seeks, right-drag pans, and the mouse wheel zooms the timeline.",
        "  - Space toggles playback, arrow keys skip/zoom, [ ] pans, 0 fits, F toggles follow, and L toggles looping.",
    ]
    .join("\n")
}
