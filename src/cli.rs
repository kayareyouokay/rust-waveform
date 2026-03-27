use std::env;
use std::path::PathBuf;

pub struct Config {
    pub path: PathBuf,
    pub snapshot: bool,
    pub width: Option<usize>,
    pub height: Option<usize>,
    pub color: bool,
}

pub enum ParseOutcome {
    Run(Config),
    Exit(String),
}

pub fn parse() -> Result<ParseOutcome, String> {
    let mut args = env::args().skip(1);
    let mut path = None;
    let mut snapshot = false;
    let mut width = None;
    let mut height = None;
    let mut color = true;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(ParseOutcome::Exit(help_text())),
            "--snapshot" => snapshot = true,
            "--no-color" => color = false,
            "--width" => {
                let value = args.next().ok_or("--width expects a number")?;
                width = Some(parse_dimension("--width", &value)?);
            }
            "--height" => {
                let value = args.next().ok_or("--height expects a number")?;
                height = Some(parse_dimension("--height", &value)?);
            }
            _ if argument.starts_with('-') => {
                return Err(format!("unknown flag: {argument}"));
            }
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

    Ok(ParseOutcome::Run(Config {
        path,
        snapshot,
        width,
        height,
        color,
    }))
}

fn parse_dimension(flag: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{flag} expects a positive integer"))?;
    if parsed < 32 {
        return Err(format!("{flag} must be at least 32"));
    }
    Ok(parsed)
}

fn help_text() -> String {
    [
        "waveform",
        "",
        "Rust waveform viewer with a fast peak pyramid and an ASCII terminal UI.",
        "",
        "USAGE:",
        "  waveform <file.wav> [--snapshot] [--width <cols>] [--height <rows>] [--no-color]",
        "",
        "CONTROLS:",
        "  h/l or Left/Right   pan",
        "  +/-                 zoom",
        "  [/]                 gain",
        "  c                   cycle channel focus",
        "  g                   toggle grid",
        "  ?                   toggle help",
        "  0                   fit full file",
        "  q                   quit",
    ]
    .join("\n")
}
