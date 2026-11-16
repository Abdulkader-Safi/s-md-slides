//! GIF of one slide's animation, sampled from the shared timeline (FR-EXP-05).

/// Encode RGBA frames (all `w` by `h`) as a GIF, `delay_cs` hundredths of a second each.
pub(crate) fn encode(
    frames: Vec<Vec<u8>>,
    w: u16,
    h: u16,
    delay_cs: u16,
    looped: bool,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut out, w, h, &[]).map_err(|e| e.to_string())?;
        if looped {
            enc.set_repeat(gif::Repeat::Infinite)
                .map_err(|e| e.to_string())?;
        }
        for mut rgba in frames {
            let mut f = gif::Frame::from_rgba_speed(w, h, &mut rgba, 10);
            f.delay = delay_cs;
            enc.write_frame(&f).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}
