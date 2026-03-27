use std::fs::File;
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

pub enum Key {
    Left,
    Right,
    Char(char),
}

pub struct Session {
    tty: File,
    saved_mode: String,
}

impl Session {
    pub fn enter() -> Result<Self, String> {
        let tty = File::options()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .map_err(|error| format!("failed to open terminal: {error}"))?;

        let saved_mode = run_stty(&tty, &["-g"])?;
        run_stty(&tty, &["raw", "-echo"])?;

        let mut stdout = io::stdout().lock();
        stdout
            .write_all(b"\x1b[?1049h\x1b[2J\x1b[H\x1b[?25l")
            .map_err(|error| format!("failed to switch terminal screen: {error}"))?;
        stdout.flush().map_err(|error| format!("failed to flush terminal: {error}"))?;

        Ok(Self { tty, saved_mode })
    }

    pub fn size(&self) -> Result<(usize, usize), String> {
        let output = Command::new("stty")
            .arg("size")
            .stdin(Stdio::from(self.tty.try_clone().map_err(|error| error.to_string())?))
            .output()
            .map_err(|error| format!("failed to query terminal size: {error}"))?;

        if !output.status.success() {
            return Err("stty size failed".to_string());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut parts = text.split_whitespace();
        let rows = parts
            .next()
            .ok_or("terminal size did not include rows")?
            .parse::<usize>()
            .map_err(|_| "invalid terminal row count".to_string())?;
        let cols = parts
            .next()
            .ok_or("terminal size did not include columns")?
            .parse::<usize>()
            .map_err(|_| "invalid terminal column count".to_string())?;

        Ok((cols.max(32), rows.max(16)))
    }

    pub fn read_key(&mut self) -> Result<Key, String> {
        let mut byte = [0u8; 1];
        self.tty
            .read_exact(&mut byte)
            .map_err(|error| format!("failed to read key press: {error}"))?;

        if byte[0] == 0x1B {
            let mut escape = [0u8; 2];
            self.tty
                .read_exact(&mut escape)
                .map_err(|error| format!("failed to read escape sequence: {error}"))?;
            if escape == [b'[', b'D'] {
                return Ok(Key::Left);
            }
            if escape == [b'[', b'C'] {
                return Ok(Key::Right);
            }
            return Ok(Key::Char('\u{1b}'));
        }

        Ok(Key::Char(byte[0] as char))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = run_stty(&self.tty, &[self.saved_mode.trim()]);
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(b"\x1b[?25h\x1b[?1049l");
        let _ = stdout.flush();
    }
}

fn run_stty(tty: &File, args: &[&str]) -> Result<String, String> {
    let output = Command::new("stty")
        .args(args)
        .stdin(Stdio::from(tty.try_clone().map_err(|error| error.to_string())?))
        .output()
        .map_err(|error| format!("failed to run stty: {error}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
