//! OSC 11 terminal background detection.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::time::Duration;

#[cfg(any(target_os = "linux", target_os = "macos"))]
const OSC_11_QUERY: &[u8] = b"\x1b]11;?\x07";
#[cfg(any(target_os = "linux", target_os = "macos"))]
const QUERY_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Appearance {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    pub(crate) fn appearance(self) -> Appearance {
        fn linear(channel: u8) -> f64 {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        let luminance =
            0.2126 * linear(self.red) + 0.7152 * linear(self.green) + 0.0722 * linear(self.blue);
        if luminance > 0.179 {
            Appearance::Light
        } else {
            Appearance::Dark
        }
    }

    pub(crate) const fn color(self) -> ratatui::style::Color {
        ratatui::style::Color::Rgb(self.red, self.green, self.blue)
    }
}

pub(crate) fn probe() -> Option<Rgb> {
    platform::read_response()
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(crate) fn parse_response(response: &[u8]) -> Option<Rgb> {
    for prefix in [b"\x1b]11;rgb:".as_slice(), b"\x9d11;rgb:".as_slice()] {
        for (offset, window) in response.windows(prefix.len()).enumerate() {
            if window == prefix {
                if let Some(rgb) = parse_rgb(&response[offset + prefix.len()..]) {
                    return Some(rgb);
                }
            }
        }
    }
    None
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn parse_rgb(input: &[u8]) -> Option<Rgb> {
    let mut cursor = 0;
    let red = parse_component(input, &mut cursor)?;
    expect(input, &mut cursor, b'/')?;
    let green = parse_component(input, &mut cursor)?;
    expect(input, &mut cursor, b'/')?;
    let blue = parse_component(input, &mut cursor)?;

    match input.get(cursor..) {
        Some([0x07, ..] | [0x9c, ..] | [0x1b, b'\\', ..]) => Some(Rgb { red, green, blue }),
        _ => None,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn parse_component(input: &[u8], cursor: &mut usize) -> Option<u8> {
    let start = *cursor;
    while input.get(*cursor).is_some_and(u8::is_ascii_hexdigit) {
        *cursor += 1;
    }
    let digits = input.get(start..*cursor)?;
    if !(1..=4).contains(&digits.len()) {
        return None;
    }

    let text = std::str::from_utf8(digits).ok()?;
    let value = u32::from_str_radix(text, 16).ok()?;
    let maximum = (1_u32 << (digits.len() * 4)) - 1;
    Some(((value * 255 + maximum / 2) / maximum) as u8)
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn expect(input: &[u8], cursor: &mut usize, expected: u8) -> Option<()> {
    if input.get(*cursor) != Some(&expected) {
        return None;
    }
    *cursor += 1;
    Some(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod platform {
    use std::ffi::{c_int, c_short};
    use std::fs::OpenOptions;
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, RawFd};
    use std::time::{Duration, Instant};

    use super::{parse_response, Rgb, OSC_11_QUERY, QUERY_TIMEOUT};

    const POLLIN: c_short = 0x0001;
    const MAX_RESPONSE_BYTES: usize = 256;

    #[cfg(target_os = "linux")]
    type PollCount = std::ffi::c_ulong;
    #[cfg(target_os = "macos")]
    type PollCount = std::ffi::c_uint;

    #[repr(C)]
    struct PollFd {
        fd: c_int,
        events: c_short,
        revents: c_short,
    }

    unsafe extern "C" {
        #[link_name = "poll"]
        fn system_poll(descriptors: *mut PollFd, count: PollCount, timeout: c_int) -> c_int;
    }

    pub(super) fn read_response() -> Option<Rgb> {
        if !crossterm::terminal::is_raw_mode_enabled().ok()? {
            return None;
        }

        let mut terminal = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        terminal.write_all(OSC_11_QUERY).ok()?;
        terminal.flush().ok()?;

        let deadline = Instant::now().checked_add(QUERY_TIMEOUT)?;
        let mut response = Vec::with_capacity(64);
        while response.len() < MAX_RESPONSE_BYTES {
            let remaining = deadline.checked_duration_since(Instant::now())?;
            match wait_readable(terminal.as_raw_fd(), remaining) {
                Ok(true) => {}
                Ok(false) => return None,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            }

            let mut byte = [0];
            match terminal.read(&mut byte) {
                Ok(1) => response.push(byte[0]),
                Ok(_) => return None,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            }
            if let Some(rgb) = parse_response(&response) {
                return Some(rgb);
            }
        }
        None
    }

    fn wait_readable(fd: RawFd, timeout: Duration) -> io::Result<bool> {
        let timeout_millis = timeout.as_millis().min(c_int::MAX as u128) as c_int;
        if timeout_millis == 0 {
            return Ok(false);
        }
        let mut descriptor = PollFd {
            fd,
            events: POLLIN,
            revents: 0,
        };
        // SAFETY: `fd` stays open and `PollFd` matches the platform C layout.
        let ready = unsafe { system_poll(&raw mut descriptor, 1, timeout_millis) };
        if ready < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ready > 0 && descriptor.revents & POLLIN != 0)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod platform {
    use super::Rgb;

    pub(super) fn read_response() -> Option<Rgb> {
        // Windows has no `/dev/tty`; unsupported platforms skip probing without reading input.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    #[test]
    fn unsupported_platform_probe_is_silent() {
        assert_eq!(probe(), None);
    }

    #[test]
    fn parses_four_digit_osc_11_with_bel_or_st() {
        assert_eq!(
            parse_response(b"\x1b]11;rgb:ffff/8080/0000\x07"),
            Some(Rgb {
                red: 255,
                green: 128,
                blue: 0,
            })
        );
        assert_eq!(
            parse_response(b"noise\x1b]11;rgb:0000/4040/ffff\x1b\\tail"),
            Some(Rgb {
                red: 0,
                green: 64,
                blue: 255,
            })
        );
    }

    #[test]
    fn parses_variable_precision_and_c1_terminator() {
        assert_eq!(
            parse_response(b"\x9d11;rgb:f/80/1234\x9c"),
            Some(Rgb {
                red: 255,
                green: 128,
                blue: 18,
            })
        );
    }

    #[test]
    fn rejects_wrong_or_malformed_responses() {
        assert_eq!(parse_response(b"\x1b]10;rgb:ffff/ffff/ffff\x07"), None);
        assert_eq!(parse_response(b"\x1b]11;rgb:ffff/zzzz/ffff\x07"), None);
        assert_eq!(parse_response(b"\x1b]11;rgb:fffff/0000/0000\x07"), None);
        assert_eq!(parse_response(b"\x1b]11;rgb:ffff/0000/0000"), None);
    }

    #[test]
    fn classifies_dark_and_light_backgrounds() {
        assert_eq!(
            Rgb {
                red: 0x1a,
                green: 0x1b,
                blue: 0x26,
            }
            .appearance(),
            Appearance::Dark
        );
        assert_eq!(
            Rgb {
                red: 0xf8,
                green: 0xf8,
                blue: 0xf2,
            }
            .appearance(),
            Appearance::Light
        );
    }

    #[test]
    fn preserves_the_detected_rgb_for_rendering() {
        let rgb = parse_response(b"\x1b]11;rgb:1a1a/2b2b/3c3c\x07").unwrap();
        assert_eq!(rgb.color(), ratatui::style::Color::Rgb(0x1a, 0x2b, 0x3c));
    }
}
