use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;

pub fn extract_docx(path: &Path, max_bytes: usize) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(BufReader::new(file)).ok()?;

    let mut doc_file = archive.by_name("word/document.xml").ok()?;
    let mut xml_content = String::new();
    doc_file.read_to_string(&mut xml_content).ok()?;

    let mut reader = Reader::from_str(&xml_content);
    reader.config_mut().trim_text(true);

    let mut text_out = String::new();
    let mut in_text = false;
    let mut buf = Vec::new();

    loop {
        if text_out.len() >= max_bytes {
            text_out.push_str("\n... [content truncated at limit]");
            break;
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == "w:t" {
                    in_text = true;
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                if name.as_ref() == "w:t" {
                    in_text = false;
                } else if name.as_ref() == "w:p" && !text_out.ends_with('\n') {
                    text_out.push('\n');
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_text {
                    text_out.push_str(e.as_ref());
                    text_out.push(' ');
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let trimmed = text_out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn extract_pptx(path: &Path, max_bytes: usize) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(BufReader::new(file)).ok()?;

    let mut slide_names: Vec<String> = archive
        .file_names()
        .filter_map(|r| r.ok())
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .map(|s| s.to_string())
        .collect();

    // Sort slides numerically: slide1.xml, slide2.xml, ...
    slide_names.sort_by_key(|n| {
        n.strip_prefix("ppt/slides/slide")
            .and_then(|s| s.strip_suffix(".xml"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });

    let mut text_out = String::new();

    for (idx, slide_name) in slide_names.iter().enumerate() {
        if text_out.len() >= max_bytes {
            text_out.push_str("\n... [content truncated at limit]");
            break;
        }

        let mut slide_file = match archive.by_name(slide_name) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut xml_content = String::new();
        if slide_file.read_to_string(&mut xml_content).is_err() {
            continue;
        }

        let mut reader = Reader::from_str(&xml_content);
        reader.config_mut().trim_text(true);

        let mut slide_text = String::new();
        let mut in_text = false;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == "a:t" {
                        in_text = true;
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = e.name();
                    if name.as_ref() == "a:t" {
                        in_text = false;
                    } else if name.as_ref() == "a:p" {
                        slide_text.push(' ');
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_text {
                        slide_text.push_str(e.as_ref());
                        slide_text.push(' ');
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        let trimmed_slide = slide_text.trim();
        if !trimmed_slide.is_empty() {
            if !text_out.is_empty() {
                text_out.push_str("\n\n");
            }
            text_out.push_str(&format!("--- Slide {} ---\n", idx + 1));
            text_out.push_str(trimmed_slide);
        }
    }

    let trimmed = text_out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn extract_xlsx(path: &Path, max_bytes: usize) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(BufReader::new(file)).ok()?;

    // 1. Read shared strings if present
    let mut shared_strings = Vec::new();
    if let Ok(mut ss_file) = archive.by_name("xl/sharedStrings.xml") {
        let mut xml_content = String::new();
        if ss_file.read_to_string(&mut xml_content).is_ok() {
            let mut reader = Reader::from_str(&xml_content);
            reader.config_mut().trim_text(true);
            let mut in_t = false;
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(ref e)) => {
                        if e.name().as_ref() == "t" {
                            in_t = true;
                        }
                    }
                    Ok(Event::End(ref e)) => {
                        if e.name().as_ref() == "t" {
                            in_t = false;
                        }
                    }
                    Ok(Event::Text(ref e)) => {
                        if in_t {
                            shared_strings.push(e.as_ref().to_string());
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
                buf.clear();
            }
        }
    }

    // 2. Read sheets
    let mut sheet_names: Vec<String> = archive
        .file_names()
        .filter_map(|r| r.ok())
        .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
        .map(|s| s.to_string())
        .collect();
    sheet_names.sort();

    let mut text_out = String::new();

    // If shared strings were extracted, include them as a summary
    if !shared_strings.is_empty() {
        for s in shared_strings.iter().take(2000) {
            if text_out.len() >= max_bytes {
                break;
            }
            text_out.push_str(s);
            text_out.push('\n');
        }
    }

    // Also scan worksheet cells for inline strings or numbers
    for sheet_name in sheet_names.iter().take(10) {
        if text_out.len() >= max_bytes {
            text_out.push_str("\n... [content truncated at limit]");
            break;
        }

        let mut sheet_file = match archive.by_name(sheet_name) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut xml_content = String::new();
        if sheet_file.read_to_string(&mut xml_content).is_err() {
            continue;
        }

        let mut reader = Reader::from_str(&xml_content);
        reader.config_mut().trim_text(true);

        let mut in_v = false;
        let mut buf = Vec::new();

        loop {
            if text_out.len() >= max_bytes {
                break;
            }
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let name = e.name();
                    if name.as_ref() == "v" || name.as_ref() == "t" {
                        in_v = true;
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = e.name();
                    if name.as_ref() == "v" || name.as_ref() == "t" {
                        in_v = false;
                    } else if name.as_ref() == "row" {
                        text_out.push('\n');
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_v {
                        text_out.push_str(e.as_ref());
                        text_out.push(' ');
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }

    let trimmed = text_out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn extract_epub(path: &Path, max_bytes: usize) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(BufReader::new(file)).ok()?;

    let mut chapter_files: Vec<String> = archive
        .file_names()
        .filter_map(|r| r.ok())
        .filter(|n| n.ends_with(".xhtml") || n.ends_with(".html"))
        .map(|s| s.to_string())
        .collect();
    chapter_files.sort();

    let mut text_out = String::new();

    for chap_name in chapter_files.iter().take(50) {
        if text_out.len() >= max_bytes {
            text_out.push_str("\n... [content truncated at limit]");
            break;
        }

        let mut chap_file = match archive.by_name(chap_name) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut html_content = String::new();
        if chap_file.read_to_string(&mut html_content).is_err() {
            continue;
        }

        let mut reader = Reader::from_str(&html_content);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        loop {
            if text_out.len() >= max_bytes {
                break;
            }
            match reader.read_event_into(&mut buf) {
                Ok(Event::Text(ref e)) => {
                    text_out.push_str(e.as_ref());
                    text_out.push(' ');
                }
                Ok(Event::End(ref e)) => {
                    let name = e.name();
                    let tag = name.as_ref();
                    if tag == "p" || tag == "h1" || tag == "h2" || tag == "h3" {
                        text_out.push('\n');
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }

    let trimmed = text_out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn extract_odt(path: &Path, max_bytes: usize) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut archive = ZipArchive::new(BufReader::new(file)).ok()?;

    let mut content_file = archive.by_name("content.xml").ok()?;
    let mut xml_content = String::new();
    content_file.read_to_string(&mut xml_content).ok()?;

    let mut reader = Reader::from_str(&xml_content);
    reader.config_mut().trim_text(true);

    let mut text_out = String::new();
    let mut buf = Vec::new();

    loop {
        if text_out.len() >= max_bytes {
            text_out.push_str("\n... [content truncated at limit]");
            break;
        }

        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(ref e)) => {
                text_out.push_str(e.as_ref());
                text_out.push(' ');
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let tag = name.as_ref();
                if tag == "text:p" || tag == "text:h" {
                    text_out.push('\n');
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let trimmed = text_out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
