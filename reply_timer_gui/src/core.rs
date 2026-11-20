//! Logic with no GUI dependency: modes, time parsing, synthesized sounds.

use std::f64::consts::PI;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum Mode {
    Casual,
    RealQuestion,
    Slow(Duration),
}

impl Mode {
    pub fn label(&self) -> String {
        match self {
            Mode::Casual => "Casual / low stakes".into(),
            Mode::RealQuestion => "She asked something real".into(),
            Mode::Slow(d) => format!("She's been slow ({})", fmt_words(*d)),
        }
    }
    pub fn duration(&self) -> Duration {
        match self {
            Mode::Casual => Duration::from_secs(20 * 60),
            Mode::RealQuestion => Duration::from_secs(30 * 60),
            Mode::Slow(d) => *d,
        }
    }
    pub fn tip(&self) -> &'static str {
        match self {
            Mode::Casual => "Keep it light. She's not your whole day.",
            Mode::RealQuestion => "Take a beat. Then give her a real answer.",
            Mode::Slow(_) => "Match her pace. Don't out-text her.",
        }
    }
}

/// Accepts: "45" (minutes), "45m", "2h", "1h30m", "1.5h", "90s", "1:30" (h:mm)
pub fn parse_duration(input: &str) -> Option<Duration> {
    let mut s = input.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    for (from, to) in [
        ("hours", "h"), ("hour", "h"), ("hrs", "h"), ("hr", "h"),
        ("minutes", "m"), ("minute", "m"), ("mins", "m"), ("min", "m"),
        ("seconds", "s"), ("second", "s"), ("secs", "s"), ("sec", "s"),
    ] {
        s = s.replace(from, to);
    }
    if let Some((h, m)) = s.split_once(':') {
        let h: u64 = h.trim().parse().ok()?;
        let m: u64 = m.trim().parse().ok()?;
        return cap(Duration::from_secs(h * 3600 + m * 60));
    }
    let (mut total, mut num, mut any) = (0.0f64, String::new(), false);
    for c in s.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'h' | 'm' | 's' => {
                let v: f64 = num.parse().ok()?;
                num.clear();
                total += v * match c { 'h' => 3600.0, 'm' => 60.0, _ => 1.0 };
                any = true;
            }
            c if c.is_whitespace() || c == ',' => {}
            _ => return None,
        }
    }
    if !num.is_empty() {
        total += num.parse::<f64>().ok()? * 60.0; // bare number = minutes
        any = true;
    }
    if !any || total <= 0.0 {
        return None;
    }
    cap(Duration::from_secs_f64(total))
}

fn cap(d: Duration) -> Option<Duration> {
    let max = Duration::from_secs(24 * 3600);
    if d.is_zero() { None } else { Some(d.min(max)) }
}

pub fn fmt_words(d: Duration) -> String {
    let t = d.as_secs();
    let (h, m, s) = (t / 3600, (t % 3600) / 60, t % 60);
    let mut parts = vec![];
    if h > 0 { parts.push(format!("{h}h")); }
    if m > 0 { parts.push(format!("{m}m")); }
    if s > 0 && h == 0 { parts.push(format!("{s}s")); }
    parts.join(" ")
}

/// Rounds up so the display hits 00:00 exactly when time is up.
pub fn fmt_clock(d: Duration) -> String {
    let t = d.as_secs_f64().ceil() as u64;
    let (h, m, s) = (t / 3600, (t % 3600) / 60, t % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

// ───────────── Sound ─────────────
/// notes: (frequency Hz, start sec, duration sec)
pub fn make_wav(notes: &[(f64, f64, f64)], tail: f64) -> Vec<u8> {
    let rate = 22_050u32;
    let total = notes.iter().map(|n| n.1 + n.2).fold(0.0, f64::max) + tail;
    let n = (total * rate as f64) as usize;
    let mut mix = vec![0.0f64; n];
    for &(f, start, dur) in notes {
        let s0 = (start * rate as f64) as usize;
        let len = ((dur + tail) * rate as f64) as usize;
        for i in 0..len {
            if s0 + i >= n { break; }
            let t = i as f64 / rate as f64;
            let env = (-t * 3.5).exp() * (t / 0.005).min(1.0);
            let tone = (2.0 * PI * f * t).sin() * 0.7
                + (2.0 * PI * f * 2.0 * t).sin() * 0.2
                + (2.0 * PI * f * 3.0 * t).sin() * 0.1;
            mix[s0 + i] += tone * env;
        }
    }
    let peak = mix.iter().fold(0.0f64, |a, &b| a.max(b.abs())).max(1e-9);
    let data: Vec<u8> = mix
        .iter()
        .flat_map(|&v| (((v / peak) * 0.6 * i16::MAX as f64) as i16).to_le_bytes())
        .collect();
    let mut w = Vec::with_capacity(44 + data.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data.len() as u32).to_le_bytes());
    w.extend_from_slice(&data);
    w
}

fn sound_path(name: &str, notes: &[(f64, f64, f64)]) -> PathBuf {
    let p = std::env::temp_dir().join(format!("reply_timer_{name}.wav"));
    let _ = std::fs::write(&p, make_wav(notes, 0.5));
    p
}

/// Rising "you're clear" arpeggio.
pub fn chime_path() -> PathBuf {
    sound_path("chime", &[(659.25, 0.0, 0.25), (783.99, 0.18, 0.25), (1046.5, 0.36, 0.25), (1318.5, 0.54, 0.6)])
}

/// Low descending "nope".
pub fn warning_path() -> PathBuf {
    sound_path("warning", &[(220.0, 0.0, 0.35), (165.0, 0.3, 0.7)])
}

fn play_file(path: &Path) -> bool {
    let p = path.to_string_lossy().to_string();
    let mut cands: Vec<(&str, Vec<String>)> = vec![];
    if cfg!(target_os = "macos") {
        cands.push(("afplay", vec![p.clone()]));
    } else if cfg!(target_os = "windows") {
        cands.push((
            "powershell",
            vec!["-NoProfile".into(), "-WindowStyle".into(), "Hidden".into(), "-Command".into(),
                 format!("(New-Object Media.SoundPlayer '{p}').PlaySync()")],
        ));
    } else {
        cands.push(("paplay", vec![p.clone()]));
        cands.push(("pw-play", vec![p.clone()]));
        cands.push(("aplay", vec!["-q".into(), p.clone()]));
        cands.push(("play", vec!["-q".into(), p.clone()]));
        cands.push(("ffplay", vec!["-nodisp".into(), "-autoexit".into(), "-loglevel".into(), "quiet".into(), p.clone()]));
    }
    for (cmd, args) in cands {
        if let Ok(mut child) = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

/// Plays on a background thread so the UI never blocks; terminal bell as last resort.
pub fn play_async(path: PathBuf, times: u32) {
    thread::spawn(move || {
        for _ in 0..times {
            if !play_file(&path) {
                print!("\x07");
                let _ = io::stdout().flush();
                thread::sleep(Duration::from_millis(400));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses() {
        assert_eq!(parse_duration("45"), Some(Duration::from_secs(2700)));
        assert_eq!(parse_duration("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(parse_duration("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("1:30"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("1.5 hours"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("90s"), Some(Duration::from_secs(90)));
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration("0"), None);
    }
    #[test]
    fn clock() {
        assert_eq!(fmt_clock(Duration::from_secs(1200)), "20:00");
        assert_eq!(fmt_clock(Duration::from_secs(3725)), "1:02:05");
    }
}
