//! Generates a visual before/after pair for the experimental screenshot lab.

use std::{env, fs};

use zion_arc::screen_lab::{decode_screen_payload, encode_screen_payload};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let cover_path = arguments
        .next()
        .ok_or("usage: screen_lab_demo <1080x2340-cover.png> <encoded.png> [payload-bytes]")?;
    let output_path = arguments
        .next()
        .ok_or("usage: screen_lab_demo <1080x2340-cover.png> <encoded.png> [payload-bytes]")?;
    let payload_len = arguments
        .next()
        .map(|value| value.to_string_lossy().parse::<usize>())
        .transpose()?
        .unwrap_or(141_844);
    if arguments.next().is_some() {
        return Err("too many arguments".into());
    }

    let cover_png = fs::read(cover_path)?;
    let payload = (0..payload_len)
        .map(|index| ((index.wrapping_mul(37) + index / 251) & 0xff) as u8)
        .collect::<Vec<_>>();
    let encoded = encode_screen_payload(&payload, &cover_png)?;
    let recovered = decode_screen_payload(&encoded.png_bytes)?;
    if recovered != payload {
        return Err("screen laboratory roundtrip mismatch".into());
    }
    fs::write(output_path, encoded.png_bytes)?;

    println!(
        "payload={} capacity={} margin={} changed_channels={} psnr_db={:.3}",
        payload_len,
        encoded.capacity_bytes,
        encoded.capacity_bytes - payload_len,
        encoded.changed_channels,
        encoded.psnr_db,
    );
    Ok(())
}
