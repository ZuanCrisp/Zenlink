//! Copy text from the device's screen (Alt+C) with Windows' built-in OCR (WinRT OcrEngine),
//! in process (port of backend/zenlink/ocr.py, which ran ocr.ps1 per capture).
//! Captures of a scrolled page are stitched: the longest run of lines that ends the stored
//! text and starts the new capture is kept once. Alt+R clears the stitch.

use std::sync::Mutex;

use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The lines in a BGRA picture.
fn recognize(width: u32, height: u32, bgra: &[u8]) -> windows::core::Result<Vec<String>> {
    let w = DataWriter::new()?;
    w.WriteBytes(bgra)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &w.DetachBuffer()?,
        BitmapPixelFormat::Bgra8,
        width as i32,
        height as i32,
    )?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
    let result = engine.RecognizeAsync(&bitmap)?.join()?;
    let mut out = Vec::new();
    for line in result.Lines()? {
        let t = line.Text()?.to_string();
        if !t.trim().is_empty() {
            out.push(t.trim_end().to_string());
        }
    }
    Ok(out)
}

fn norm(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Appends `new` to `old`, skipping lines already at the end of `old`. Returns how many
/// lines were added.
fn merge(old: &mut Vec<String>, new: Vec<String>) -> usize {
    let a: Vec<String> = old.iter().map(|x| norm(x)).collect();
    let b: Vec<String> = new.iter().map(|x| norm(x)).collect();
    let best = (1..=a.len().min(b.len()))
        .rev()
        .find(|&k| a[a.len() - k..] == b[..k])
        .unwrap_or(0);
    if !old.is_empty() && best == 0 && !b.is_empty() && b.iter().all(|x| a.contains(x)) {
        return 0; // same screen again
    }
    let added = new.len() - best;
    old.extend(new.into_iter().skip(best));
    added
}

/// Reads a capture: (stitched text, lines added, lines in total).
pub fn add_capture(width: u32, height: u32, bgra: &[u8]) -> Result<(String, usize, usize), String> {
    let lines = recognize(width, height, bgra).map_err(|e| e.message().to_string())?;
    let read = lines.len();
    let mut all = LINES.lock().unwrap();
    let added = merge(&mut all, lines);
    log::info!("ocr: {read} lines read, {added} new, {} stitched", all.len());
    Ok((all.join("\n"), added, all.len()))
}

pub fn clear() {
    LINES.lock().unwrap().clear();
}

#[cfg(test)]
mod tests {
    use super::merge;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    /// Needs a Windows OCR language: `cargo test -p zenlink-app ocr -- --ignored`.
    #[test]
    #[ignore]
    fn winrt_reads_a_blank_picture() {
        assert_eq!(super::recognize(64, 64, &[255; 64 * 64 * 4]).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn stitches_scrolled_captures() {
        let mut all = Vec::new();
        assert_eq!(merge(&mut all, v(&["a", "b", "c"])), 3);
        assert_eq!(merge(&mut all, v(&["B", "c ", "d"])), 1);
        assert_eq!(all, v(&["a", "b", "c", "d"]));
        assert_eq!(merge(&mut all, v(&["a", "b"])), 0, "same screen again");
        assert_eq!(merge(&mut all, v(&["x"])), 1);
    }
}
