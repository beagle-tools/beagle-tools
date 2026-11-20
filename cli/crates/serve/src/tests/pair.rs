// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn the_link_encodes_the_url_and_token() {
    let token = Token::parse(TOKEN).unwrap();
    let url = server_url("mac.example-tailnet.ts.net", 8787);
    assert_eq!(url, "https://mac.example-tailnet.ts.net:8787");
    assert_eq!(
        pair_link(&url, &token),
        format!("beagle://pair?url=https%3A%2F%2Fmac.example-tailnet.ts.net%3A8787&token={TOKEN}")
    );
}

#[test]
fn percent_encoding_keeps_only_unreserved_characters() {
    assert_eq!(percent_encode("aZ09-._~"), "aZ09-._~");
    assert_eq!(percent_encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
}

/// Reads a drawn QR code back, as a phone camera would: the half-block
/// glyphs become a pixel grid (4 px per module) that `rqrr` decodes.
fn scan(drawn: &str) -> String {
    let rows: Vec<Vec<bool>> = drawn
        .lines()
        .flat_map(|line| {
            let glyphs = line
                .trim_start_matches(BLACK_ON_WHITE)
                .trim_end_matches(RESET);
            let (top, bottom): (Vec<bool>, Vec<bool>) = glyphs
                .chars()
                .map(|c| match c {
                    '█' => (true, true),
                    '▀' => (true, false),
                    '▄' => (false, true),
                    _ => (false, false),
                })
                .unzip();
            [top, bottom]
        })
        .collect();
    let scale = 4;
    let (w, h) = (rows[0].len() * scale, rows.len() * scale);
    let mut img = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
        if rows[y / scale][x / scale] {
            0
        } else {
            255
        }
    });
    let grids = img.detect_grids();
    assert_eq!(grids.len(), 1, "one QR code in the drawing");
    grids[0].decode().unwrap().1
}

#[test]
fn the_drawn_qr_code_scans_to_the_link() {
    let token = Token::parse(TOKEN).unwrap();
    let link = pair_link(&server_url("h.example-tailnet.ts.net", 8787), &token);
    let drawn = qr(&link).unwrap();
    assert!(drawn
        .lines()
        .all(|l| l.starts_with(BLACK_ON_WHITE) && l.ends_with(RESET)));
    assert_eq!(
        scan(&drawn),
        format!("beagle://pair?url=https%3A%2F%2Fh.example-tailnet.ts.net%3A8787&token={TOKEN}")
    );
}

#[test]
fn a_token_is_kept_unless_rotated_or_missing() {
    let config = Config {
        token: Some(TOKEN.into()),
        ..Config::default()
    };
    let (kept, new) = token_for(&config, false).unwrap();
    assert_eq!(kept.as_str(), TOKEN);
    assert!(!new);
    let (rotated, new) = token_for(&config, true).unwrap();
    assert_ne!(rotated.as_str(), TOKEN);
    assert!(new);
    let (fresh, new) = token_for(&Config::default(), false).unwrap();
    assert!(new);
    assert!(Token::parse(fresh.as_str()).is_some());
    let broken = Config {
        token: Some("short".into()),
        ..Config::default()
    };
    assert!(token_for(&broken, false).unwrap().1);
}
