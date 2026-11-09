//! `System/wifi.conf`: the card's own Wi-Fi credentials.
//!
//! Read by `System/slotnet.sh` — the direct-connect dialog (a hold of B on the shelf) and
//! the `network.on` boot hook — and written by the settings screen's Wi-Fi row. The file
//! is a wpa_supplicant `network={}` block and nothing else: the shape `wifi.conf.example`
//! ships, with `ctrl_interface` prepended by the script at run time.
//!
//! A card edited on a desktop must never be able to produce a device that will not boot,
//! so anything unreadable is "no credentials" rather than an error — exactly the rule
//! `theme.rs` follows.

use std::path::Path;

/// Where the credentials live, relative to the card root.
pub const WIFI_FILE: &str = "System/wifi.conf";

/// The two things a network needs. An empty `psk` is an open network, which is a
/// configuration and not a half-read one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WifiConf {
    pub ssid: String,
    pub psk: String,
}

impl WifiConf {
    /// The card's credentials, or `None` when there is no readable file with an `ssid`
    /// in it.
    pub fn read(root: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(root.join(WIFI_FILE)).ok()?;
        let mut ssid = None;
        let mut psk = None;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("ssid=") {
                ssid = unquote(rest);
            } else if let Some(rest) = line.strip_prefix("psk=") {
                psk = unquote(rest);
            }
        }
        // `key_mgmt=NONE` and a missing `psk` line are both the honest spelling of an
        // open network; either way there is no password to send.
        let ssid = ssid?;
        Some(WifiConf {
            ssid,
            psk: psk.unwrap_or_default(),
        })
    }

    /// Write the credentials where the script reads them. Quotes and backslashes are
    /// escaped rather than assumed: an SSID with a quote in it is legal, and a
    /// half-escaped one is a network nobody can join.
    pub fn write(root: &Path, conf: &WifiConf) -> std::io::Result<()> {
        let key = if conf.psk.is_empty() {
            "    key_mgmt=NONE\n"
        } else {
            ""
        };
        let psk = if conf.psk.is_empty() {
            String::new()
        } else {
            format!("    psk=\"{}\"\n", quote(&conf.psk))
        };
        let body = format!(
            "network={{\n    ssid=\"{}\"\n{psk}{key}}}\n",
            quote(&conf.ssid),
        );
        std::fs::write(root.join(WIFI_FILE), body)
    }
}

/// **Undo wpa_cli's escaping of a scanned SSID.**
///
/// `wpa_cli scan_results` prints only printable ASCII: every byte of a Chinese name comes
/// back as `\xNN`, so `中文` arrives as `\xe4\xb8\xad\xe6\x96\x87` and a list that printed
/// that would be a list nobody can read. `\\` and `\"` are the other two escapes it uses.
/// The bytes are decoded as UTF-8 afterwards — lossily, because an SSID is an arbitrary
/// byte string and a card with a broken one should still see its network listed.
pub fn unescape_ssid(raw: &str) -> String {
    fn nibble(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'x' | b'X' if i + 3 < bytes.len() => {
                    if let (Some(hi), Some(lo)) = (nibble(bytes[i + 2]), nibble(bytes[i + 3])) {
                        out.push(hi * 16 + lo);
                        i += 4;
                        continue;
                    }
                }
                b'\\' | b'"' => {
                    out.push(bytes[i + 1]);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Strip one layer of surrounding quotes and unescape what is inside them. `None` when
/// the value is not quoted at all — the example file's shape is the shape.
fn unquote(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('"')?.strip_suffix('"')?;
    Some(unescape(inner))
}

fn quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(body: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("System")).expect("mkdir");
        std::fs::write(dir.path().join(WIFI_FILE), body).expect("write");
        dir
    }

    #[test]
    fn reads_the_shape_the_example_ships() {
        let dir = card("network={\n    ssid=\"MyNet\"\n    psk=\"hunter2hunter2\"\n}\n");
        let conf = WifiConf::read(dir.path()).expect("read");
        assert_eq!(conf.ssid, "MyNet");
        assert_eq!(conf.psk, "hunter2hunter2");
    }

    #[test]
    fn a_card_with_no_file_is_no_credentials_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(WifiConf::read(dir.path()), None);
    }

    #[test]
    fn an_open_network_is_a_configuration() {
        let dir = card("network={\n    ssid=\"Cafe\"\n    key_mgmt=NONE\n}\n");
        let conf = WifiConf::read(dir.path()).expect("read");
        assert_eq!(conf.ssid, "Cafe");
        assert!(conf.psk.is_empty());
    }

    #[test]
    fn a_write_round_trips_through_a_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("System")).expect("mkdir");
        let want = WifiConf {
            ssid: "Quote\"Net".to_string(),
            psk: "back\\slash".to_string(),
        };
        WifiConf::write(dir.path(), &want).expect("write");
        assert_eq!(WifiConf::read(dir.path()), Some(want));
    }

    #[test]
    fn an_open_network_writes_key_mgmt_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("System")).expect("mkdir");
        WifiConf::write(
            dir.path(),
            &WifiConf {
                ssid: "Cafe".to_string(),
                psk: String::new(),
            },
        )
        .expect("write");
        let body = std::fs::read_to_string(dir.path().join(WIFI_FILE)).expect("read");
        assert!(body.contains("key_mgmt=NONE"), "{body}");
        assert!(!body.contains("psk="), "{body}");
    }

    #[test]
    fn a_file_with_no_ssid_is_no_credentials() {
        let dir = card("network={\n    psk=\"orphan\"\n}\n");
        assert_eq!(WifiConf::read(dir.path()), None);
    }

    #[test]
    fn a_chinese_ssid_comes_back_from_wpa_clis_escaping() {
        // `中文网` as wpa_cli prints it: every byte above 0x7f escaped.
        assert_eq!(unescape_ssid("\\xe4\\xb8\\xad\\xe6\\x96\\x87\\xe7\\xbd\\x91"), "中文网");
    }

    #[test]
    fn a_plain_ssid_is_left_alone() {
        assert_eq!(unescape_ssid("MyPhone 5G"), "MyPhone 5G");
    }

    #[test]
    fn the_two_literal_escapes_are_undone() {
        assert_eq!(unescape_ssid("a\\\\b\\\"c"), "a\\b\"c");
    }

    #[test]
    fn a_truncated_escape_is_kept_as_text() {
        // A half-written sequence is not a byte we can trust: it stays, visibly.
        assert_eq!(unescape_ssid("net\\x4"), "net\\x4");
    }
}
