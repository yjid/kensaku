use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn extract_text(path: &Path, max_bytes: usize) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buffer = Vec::new();
    let mut handle = (&mut file).take(max_bytes as u64);
    handle.read_to_end(&mut buffer).ok()?;

    if buffer.is_empty() {
        return None;
    }

    // 1. Check for UTF-16 BOMs
    if buffer.len() >= 2 {
        if buffer[0] == 0xFF && buffer[1] == 0xFE {
            // UTF-16 LE
            let u16_slice: Vec<u16> = buffer[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            if let Ok(s) = String::from_utf16(&u16_slice) {
                return Some(clean_text(&s));
            }
        } else if buffer[0] == 0xFE && buffer[1] == 0xFF {
            // UTF-16 BE
            let u16_slice: Vec<u16> = buffer[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            if let Ok(s) = String::from_utf16(&u16_slice) {
                return Some(clean_text(&s));
            }
        }
    }

    // 2. Reject binary files (null bytes in first 1024 bytes)
    if buffer.iter().take(1024).any(|&b| b == 0) {
        return None;
    }

    // 3. UTF-8 decoding
    let raw_text = match String::from_utf8(buffer.clone()) {
        Ok(s) => s,
        Err(_) => {
            // Lossy fallback if mostly text
            String::from_utf8_lossy(&buffer).to_string()
        }
    };

    // 4. Markdown frontmatter stripping
    let cleaned = strip_markdown_frontmatter(&raw_text);

    Some(clean_text(&cleaned))
}

pub fn extract_rtf(path: &Path, max_bytes: usize) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buffer = Vec::new();
    let mut handle = (&mut file).take(max_bytes as u64);
    handle.read_to_end(&mut buffer).ok()?;

    let raw = String::from_utf8_lossy(&buffer);
    if !raw.starts_with("{\\rtf") {
        return None;
    }

    // Simple RTF text stripper: remove \{ ... \} control groups and \control words
    let mut text_out = String::new();
    let mut in_control = false;
    let mut depth = 0;

    for ch in raw.chars() {
        if text_out.len() >= max_bytes {
            break;
        }
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            '\\' => {
                in_control = true;
            }
            ' ' | '\t' | '\n' | '\r' => {
                if in_control {
                    in_control = false;
                } else if depth <= 1 {
                    text_out.push(ch);
                }
            }
            _ => {
                if in_control {
                    if !ch.is_alphanumeric() {
                        in_control = false;
                    }
                } else if depth <= 1 {
                    text_out.push(ch);
                }
            }
        }
    }

    let trimmed = text_out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn strip_markdown_frontmatter(text: &str) -> String {
    let trimmed = text.trim_start();
    if let Some(stripped) = trimmed.strip_prefix("---")
        && let Some(rest) = stripped
            .strip_prefix('\n')
            .or_else(|| stripped.strip_prefix("\r\n"))
        && let Some(end_pos) = rest.find("\n---")
    {
        let after = &rest[end_pos + 4..];
        return after.trim_start_matches(['\r', '\n']).to_string();
    }
    text.to_string()
}

fn clean_text(s: &str) -> String {
    // Strip BOM and null characters
    s.trim_start_matches('\u{feff}')
        .replace('\0', "")
        .trim()
        .to_string()
}
