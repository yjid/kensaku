use mail_parser::decoders::html::html_to_text;
use mail_parser::MimeHeaders;
use std::path::Path;

/// Trims extracted text to `max_bytes` without splitting a character.
fn cap(mut out: String, max_bytes: usize) -> String {
    let keep = super::delimited::clip_to_char_boundary(&out, max_bytes).len();
    out.truncate(keep);
    out
}

/// Extracts searchable text from an Outlook `.msg` (OLE/MAPI) file.
pub fn extract_msg(path: &Path, max_bytes: usize) -> Option<String> {
    let outlook = msg_parser::Outlook::from_path(path).ok()?;
    let mut out = String::new();

    if !outlook.subject.is_empty() {
        out.push_str("Subject: ");
        out.push_str(&outlook.subject);
        out.push('\n');
    }

    if !outlook.message_delivery_time.is_empty() {
        out.push_str("Date: ");
        out.push_str(&outlook.message_delivery_time);
        out.push('\n');
    }

    push_msg_people(&mut out, "From:", std::slice::from_ref(&outlook.sender));
    push_msg_people(&mut out, "To:", &outlook.to);
    push_msg_people(&mut out, "Cc:", &outlook.cc);
    push_msg_people(&mut out, "Bcc:", &outlook.bcc);

    // Many messages have no plain-text body (PidTagBody), only HTML
    // (PidTagHtml) or compressed RTF (PidTagRtfCompressed). The RTF only
    // yields text when Outlook generated it from HTML (`\fromhtml1`); a native
    // rich-text body has no embedded HTML and is still missed.
    let body = if !outlook.body.trim().is_empty() {
        outlook.body.clone()
    } else if !outlook.html.trim().is_empty() {
        html_to_text(&msg_html(&outlook.html))
    } else {
        outlook
            .html_from_rtf()
            .map(|h| html_to_text(&h))
            .unwrap_or_default()
    };
    if !body.trim().is_empty() {
        out.push('\n');
        out.push_str(body.trim());
        out.push('\n');
    }

    for attachment in &outlook.attachments {
        if !attachment.long_file_name.is_empty() {
            out.push_str("Attachment: ");
            out.push_str(&attachment.long_file_name);
            out.push('\n');
        }
    }

    if out.is_empty() {
        return None;
    }

    Some(cap(out, max_bytes))
}

/// The HTML body as text. msg_parser renders binary MAPI properties as hex,
/// and `PidTagHtml` is binary in Outlook's own files, so the field usually
/// holds hex; writers that store it as a string property give the HTML as-is.
/// Real HTML contains `<`, which is not a hex digit, so a successful decode
/// identifies the binary form. The bytes are in the message's code page;
/// UTF-8 is assumed, so other code pages lose their non-ASCII characters.
fn msg_html(html: &str) -> String {
    match hex::decode(html) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => html.to_string(),
    }
}

/// One labelled line for `.msg` people. Exchange-internal sender IDs
/// (`/O=EXCHANGELABS/...`) are not addresses, so only values with `@` are
/// kept, and an address equal to the display name is written once.
fn push_msg_people(out: &mut String, label: &str, people: &[msg_parser::Person]) {
    let mut line = String::new();
    for person in people {
        if !person.name.is_empty() {
            line.push(' ');
            line.push_str(&person.name);
        }
        if person.email.contains('@') && person.email != person.name {
            line.push(' ');
            line.push_str(&person.email);
        }
    }
    if !line.is_empty() {
        out.push_str(label);
        out.push_str(&line);
        out.push('\n');
    }
}

/// Extracts searchable text from an `.eml` (RFC 822 / MIME) file.
pub fn extract_eml(path: &Path, max_bytes: usize) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let message = mail_parser::MessageParser::default().parse(&bytes)?;
    let mut out = String::new();

    if let Some(subject) = message.subject() {
        out.push_str("Subject: ");
        out.push_str(subject);
        out.push('\n');
    }

    if let Some(date) = message.date() {
        out.push_str("Date: ");
        out.push_str(&date.to_rfc3339());
        out.push('\n');
    }

    push_people(&mut out, "From:", message.from());
    push_people(&mut out, "To:", message.to());
    push_people(&mut out, "Cc:", message.cc());
    push_people(&mut out, "Bcc:", message.bcc());

    if let Some(body) = message.body_text(0) {
        out.push('\n');
        out.push_str(body.trim());
        out.push('\n');
    }

    for attachment in message.attachments() {
        if let Some(name) = attachment.attachment_name() {
            out.push_str("Attachment: ");
            out.push_str(name);
            out.push('\n');
        }
    }

    Some(cap(out, max_bytes))
}

fn push_people(out: &mut String, label: &str, people: Option<&mail_parser::Address>) {
    if let Some(list) = people {
        out.push_str(label);
        for person in list.iter() {
            if let Some(name) = person.name() {
                out.push(' ');
                out.push_str(name);
            }
            if let Some(address) = person.address() {
                out.push(' ');
                out.push_str(address);
            }
        }
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A temp file that removes itself on drop.
    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn bytes_fixture(tag: &str, bytes: &[u8]) -> Fixture {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "kensaku-email-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ));
        fs::write(&path, bytes).unwrap();
        Fixture(path)
    }

    #[test]
    fn extracts_subject() {
        let fx = bytes_fixture(
            "subject",
            b"From: a@x.com\r\nSubject: Hello\r\n\r\nbody\r\n",
        );
        assert_eq!(
            extract_eml(&fx.0, 10_000),
            Some("Subject: Hello\nFrom: a@x.com\n\nbody\n".to_string())
        );
    }

    #[test]
    fn extracts_subject_and_date() {
        let fx = bytes_fixture(
            "date",
            b"From: a@x.com\r\nSubject: Hello\r\nDate: Mon, 1 Jan 2024 10:00:00 +0000\r\n\r\nbody\r\n",
        );
        assert_eq!(
            extract_eml(&fx.0, 10_000),
            Some("Subject: Hello\nDate: 2024-01-01T10:00:00Z\nFrom: a@x.com\n\nbody\n".to_string())
        );
    }

    #[test]
    fn extracts_sender_name_and_address() {
        let fx = bytes_fixture(
            "from",
            b"From: Alice <a@x.com>\r\nSubject: Hi\r\n\r\nbody\r\n",
        );
        assert_eq!(
            extract_eml(&fx.0, 10_000),
            Some("Subject: Hi\nFrom: Alice a@x.com\n\nbody\n".to_string())
        );
    }

    #[test]
    fn extracts_all_recipient_headers() {
        let fx = bytes_fixture(
            "recipients",
            b"From: a@x.com\r\n\
              To: Bob <b@x.com>, c@x.com\r\n\
              Cc: Dana <d@x.com>\r\n\
              Bcc: e@x.com\r\n\
              \r\nbody\r\n",
        );
        assert_eq!(
            extract_eml(&fx.0, 10_000),
            Some(
                "From: a@x.com\nTo: Bob b@x.com c@x.com\nCc: Dana d@x.com\nBcc: e@x.com\n\nbody\n"
                    .to_string()
            )
        );
    }

    #[test]
    fn extracts_attachment_names() {
        let fx = bytes_fixture(
            "attach",
            b"From: a@x.com\r\n\
              Subject: Report\r\n\
              MIME-Version: 1.0\r\n\
              Content-Type: multipart/mixed; boundary=\"XYZ\"\r\n\
              \r\n\
              --XYZ\r\n\
              Content-Type: text/plain\r\n\
              \r\n\
              See attached.\r\n\
              --XYZ\r\n\
              Content-Type: application/pdf\r\n\
              Content-Disposition: attachment; filename=\"report.pdf\"\r\n\
              Content-Transfer-Encoding: base64\r\n\
              \r\n\
              JVBERi0xLjQK\r\n\
              --XYZ--\r\n",
        );
        assert_eq!(
            extract_eml(&fx.0, 10_000),
            Some(
                "Subject: Report\nFrom: a@x.com\n\nSee attached.\nAttachment: report.pdf\n"
                    .to_string()
            )
        );
    }

    #[test]
    fn malformed_input_does_not_panic() {
        let empty = bytes_fixture("empty", b"");
        let _ = extract_eml(&empty.0, 10_000);

        let garbage = bytes_fixture("garbage", &[0xff, 0x00, 0xd0, 0xcf, 0x11, 0xe0, 0x80, 0x0a]);
        let _ = extract_eml(&garbage.0, 10_000);

        // Multipart header promising a boundary that never appears.
        let broken = bytes_fixture(
            "broken",
            b"Subject: x\r\nContent-Type: multipart/mixed; boundary=\"NOPE\"\r\n\r\n--NOPE\r\n",
        );
        let _ = extract_eml(&broken.0, 10_000);

        assert_eq!(
            extract_eml(Path::new("/nonexistent/kensaku.eml"), 10_000),
            None
        );
    }

    fn msg_fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/email")
            .join(name)
    }

    #[test]
    fn extracts_outlook_msg() {
        assert_eq!(
            extract_msg(&msg_fixture("ascii.msg"), 10_000),
            Some(
                "Subject: creating an outlook message file\n\
                 From: from@domain.com\n\
                 To: to@domain.com\n\
                 \n\
                 This message is created by Aspose.Email\n"
                    .to_string()
            )
        );
    }

    /// Builds a minimal `.msg`: an OLE compound file whose root storage holds
    /// one `__substg1.0_<prop id><prop type>` stream per MAPI property.
    fn msg_bytes(props: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut file = cfb::CompoundFile::create_with_version(
            cfb::Version::V3,
            std::io::Cursor::new(Vec::new()),
        )
        .unwrap();
        for (prop, data) in props {
            let mut stream = file.create_stream(format!("/__substg1.0_{prop}")).unwrap();
            stream.write_all(data).unwrap();
        }
        file.flush().unwrap();
        file.into_inner().into_inner()
    }

    /// A `PT_UNICODE` property value: UTF-16LE, as Outlook writes strings.
    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    /// A `PidTagRtfCompressed` value in the uncompressed ("MELA") variant of
    /// MS-OXRTFCP. Outlook normally writes the LZ77 ("LZFu") variant; msg_parser
    /// decompresses both to the same RTF.
    fn rtf_uncompressed(rtf: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(rtf.len() as u32 + 12).to_le_bytes()); // CompSize
        out.extend_from_slice(&(rtf.len() as u32).to_le_bytes()); // RawSize
        out.extend_from_slice(b"MELA"); // CompType
        out.extend_from_slice(&0u32.to_le_bytes()); // CRC, unused when uncompressed
        out.extend_from_slice(rtf.as_bytes());
        out
    }

    const SUBJECT: &str = "0037001F";
    const BODY: &str = "1000001F";
    const RTF_COMPRESSED: &str = "10090102";

    #[test]
    fn msg_html_only_body_is_indexed_as_text() {
        let html =
            "<html><body><p>Revenue grew <b>twelve percent</b> this quarter.</p></body></html>";
        // PidTagHtml is PT_BINARY in Outlook's own files; some writers use PT_UNICODE.
        for (prop, value) in [
            ("10130102", html.as_bytes().to_vec()),
            ("1013001F", utf16(html)),
        ] {
            let fx = bytes_fixture(
                "html-msg",
                &msg_bytes(&[(SUBJECT, &utf16("Quarterly numbers")), (prop, &value)]),
            );
            let text = extract_msg(&fx.0, 10_000).unwrap();
            assert!(
                text.starts_with("Subject: Quarterly numbers\n"),
                "{prop}: {text:?}"
            );
            assert!(
                text.contains("Revenue grew twelve percent this quarter."),
                "{prop}: {text:?}"
            );
            assert!(
                !text.contains('<') && !text.contains('>'),
                "{prop}: {text:?}"
            );
        }
    }

    #[test]
    fn msg_rtf_only_body_is_recovered_from_embedded_html() {
        // RTF that Outlook generated from HTML (`\fromhtml1`): the original
        // tags sit in `\*\htmltag` groups, the text in `\htmlrtf0` regions.
        let rtf = r"{\rtf1\ansi\ansicpg1252\fromhtml1 \deff0{\fonttbl{\f0\fswiss Arial;}}
{\*\htmltag19 <html>}{\*\htmltag50 <body>}{\*\htmltag64 <p>}\htmlrtf {\htmlrtf0 Lunch moved to Thursday.\htmlrtf }\htmlrtf0 {\*\htmltag72 </p>}{\*\htmltag58 </body>}{\*\htmltag27 </html>}}";
        let fx = bytes_fixture(
            "rtf-msg",
            &msg_bytes(&[
                (SUBJECT, &utf16("Team lunch")),
                (RTF_COMPRESSED, &rtf_uncompressed(rtf)),
            ]),
        );
        let text = extract_msg(&fx.0, 10_000).unwrap();
        assert!(text.starts_with("Subject: Team lunch\n"), "{text:?}");
        assert!(text.contains("Lunch moved to Thursday."), "{text:?}");
        assert!(!text.contains('<') && !text.contains('>'), "{text:?}");
    }

    #[test]
    fn msg_without_recoverable_body_keeps_headers_only() {
        // Native rich text, not converted from HTML: no `\fromhtml1`, so there
        // is no embedded HTML to recover. Known limitation.
        let native_rtf = rtf_uncompressed(r"{\rtf1\ansi\deff0 Only in rich text.\par}");
        let cases: [&[(&str, &[u8])]; 3] = [
            &[],
            &[(BODY, &utf16(" \r\n"))],
            &[(RTF_COMPRESSED, &native_rtf)],
        ];
        for (i, extra) in cases.into_iter().enumerate() {
            let subject = utf16("Status");
            let mut props = vec![(SUBJECT, subject.as_slice())];
            props.extend_from_slice(extra);
            let fx = bytes_fixture("nobody-msg", &msg_bytes(&props));
            assert_eq!(
                extract_msg(&fx.0, 10_000),
                Some("Subject: Status\n".to_string()),
                "case {i}"
            );
        }
    }

    #[test]
    fn malformed_msg_returns_none() {
        let text = bytes_fixture("bad-msg", b"not an OLE file");
        assert_eq!(extract_msg(&text.0, 10_000), None);

        // Valid OLE header, truncated: must not panic.
        let truncated = bytes_fixture(
            "trunc-msg",
            &[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1],
        );
        assert_eq!(extract_msg(&truncated.0, 10_000), None);

        assert_eq!(
            extract_msg(Path::new("/nonexistent/kensaku.msg"), 10_000),
            None
        );
    }

    #[test]
    fn output_is_capped_on_a_char_boundary() {
        // "é" is two bytes, so a 10-byte cap lands mid-character.
        let fx = bytes_fixture("cap", "Subject: ééééé\r\n\r\nbody\r\n".as_bytes());
        let text = extract_eml(&fx.0, 10).unwrap();
        assert!(text.len() <= 10);
        assert_eq!(text, "Subject: ");
    }
}
