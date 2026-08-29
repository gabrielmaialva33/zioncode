use crate::{
    constants::{MAX_LINE_BYTES, MAX_MARKER_NAME_BYTES, MAX_MARKERS_PER_BOOK},
    error::CorpusError,
    model::{BookDocument, CanonicalBook, Chapter, Marker, MarkerRecord},
};

pub(crate) struct ParsedUsfm {
    pub document: BookDocument,
    pub marker_count: usize,
    pub verse_count: usize,
}

#[derive(Default)]
struct InlineState {
    addition: bool,
    footnote: bool,
    reference_quote: bool,
}

impl InlineState {
    fn is_clear(&self) -> bool {
        !self.addition && !self.footnote && !self.reference_quote
    }
}

// Keeping the bounded single-pass grammar in one function makes state transitions auditable.
#[allow(clippy::too_many_lines)]
pub(crate) fn parse(raw: &[u8], book: &CanonicalBook) -> Result<ParsedUsfm, CorpusError> {
    validate_lines(raw, &book.id)?;
    let raw_text = std::str::from_utf8(raw).map_err(|_| CorpusError::InvalidUtf8 {
        book: book.id.clone(),
    })?;
    let text = raw_text.strip_prefix('\u{feff}').unwrap_or(raw_text);

    let first_marker = text
        .find('\\')
        .ok_or_else(|| malformed(book, "no USFM marker"))?;
    if !text[..first_marker].trim().is_empty() {
        return Err(malformed(
            book,
            "non-whitespace bytes precede the first marker",
        ));
    }

    let mut headers = Vec::with_capacity(7);
    let mut header_names = Vec::with_capacity(7);
    let mut chapters: Vec<Chapter> = Vec::with_capacity(usize::from(book.chapter_count));
    let mut state = InlineState::default();
    let mut marker_count = 0_usize;
    let mut verse_count = 0_usize;
    let mut source_id: Option<String> = None;
    let mut source_title: Option<String> = None;
    let mut cursor = first_marker;

    while cursor < text.len() {
        marker_count = marker_count
            .checked_add(1)
            .ok_or_else(|| malformed(book, "marker count overflow"))?;
        if marker_count > MAX_MARKERS_PER_BOOK {
            return Err(CorpusError::TooManyMarkers {
                book: book.id.clone(),
                limit: MAX_MARKERS_PER_BOOK,
            });
        }

        let name_start = cursor + 1;
        let tail = &text[name_start..];
        let base_len = tail.bytes().take_while(u8::is_ascii_alphanumeric).count();
        if base_len == 0 || base_len > MAX_MARKER_NAME_BYTES {
            return Err(malformed(book, "invalid marker name"));
        }
        let has_star = tail.as_bytes().get(base_len) == Some(&b'*');
        let name_len = base_len + usize::from(has_star);
        let name = &tail[..name_len];
        let value_start = name_start + name_len;
        let next = text[value_start..]
            .find('\\')
            .map_or(text.len(), |offset| value_start + offset);
        let value = normalized_value(&text[value_start..next], book)?;
        cursor = next;

        if name == "c" {
            require_clear_state(&state, book, "chapter boundary inside inline marker")?;
            validate_headers(&header_names, book)?;
            let number = parse_bare_number(&value, book, "chapter")?;
            let expected = chapters.len() + 1;
            if usize::from(number) != expected {
                return Err(malformed(
                    book,
                    &format!("chapter {number} is not contiguous; expected {expected}"),
                ));
            }
            chapters.push(Chapter {
                number,
                markers: Vec::new(),
            });
            continue;
        }

        let marker = marker_from_name(name, book)?;
        if chapters.is_empty() {
            if !is_header(marker) {
                return Err(malformed(book, "body marker appears before chapter 1"));
            }
            header_names.push(name.to_owned());
            if marker == Marker::Id {
                source_id = value.split_whitespace().next().map(str::to_owned);
            } else if marker == Marker::Toc2 {
                source_title = Some(value.trim().to_owned());
            }
            headers.push(MarkerRecord {
                marker,
                number: None,
                text: value,
            });
            continue;
        }
        if is_header(marker) {
            return Err(malformed(book, "header marker appears after chapter 1"));
        }

        update_inline_state(marker, &mut state, book)?;
        let (number, text) = if marker == Marker::Verse {
            verse_count = verse_count
                .checked_add(1)
                .ok_or_else(|| malformed(book, "verse count overflow"))?;
            let (number, verse_text) = parse_number_and_text(&value, book, "verse")?;
            (Some(number), verse_text)
        } else {
            (None, value)
        };
        chapters
            .last_mut()
            .ok_or_else(|| malformed(book, "body marker without a chapter"))?
            .markers
            .push(MarkerRecord {
                marker,
                number,
                text,
            });
    }

    require_clear_state(&state, book, "unclosed inline marker at end of book")?;
    validate_headers(&header_names, book)?;
    if source_id.as_deref() != Some(book.id.as_str()) {
        return Err(malformed(book, "USFM id does not match canonical mapping"));
    }
    if source_title.as_deref() != Some(book.title.as_str()) {
        return Err(malformed(
            book,
            "toc2 title does not match canonical mapping",
        ));
    }
    if chapters.len() != usize::from(book.chapter_count) {
        return Err(malformed(
            book,
            "chapter count does not match canonical mapping",
        ));
    }
    if verse_count != book.verse_count as usize {
        return Err(malformed(
            book,
            "verse-record count does not match canonical mapping",
        ));
    }

    Ok(ParsedUsfm {
        document: BookDocument::new(book, headers, chapters, raw_text.to_owned()),
        marker_count,
        verse_count,
    })
}

fn validate_lines(raw: &[u8], book: &str) -> Result<(), CorpusError> {
    for line in raw.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.len() > MAX_LINE_BYTES {
            return Err(CorpusError::LineTooLong {
                book: book.to_owned(),
                actual: line.len(),
                limit: MAX_LINE_BYTES,
            });
        }
    }
    Ok(())
}

fn normalized_value(raw: &str, book: &CanonicalBook) -> Result<String, CorpusError> {
    let normalized = raw.replace("\r\n", "\n");
    if normalized.contains('\r') {
        return Err(malformed(book, "bare carriage return"));
    }
    let value = normalized.strip_prefix(' ').unwrap_or(&normalized);
    Ok(value.trim_end_matches('\n').to_owned())
}

fn parse_bare_number(value: &str, book: &CanonicalBook, kind: &str) -> Result<u16, CorpusError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.bytes().any(|byte| !byte.is_ascii_digit()) {
        return Err(malformed(book, &format!("invalid {kind} number")));
    }
    let number = trimmed
        .parse::<u16>()
        .map_err(|_| malformed(book, &format!("invalid {kind} number")))?;
    if number == 0 {
        return Err(malformed(book, &format!("{kind} number is zero")));
    }
    Ok(number)
}

fn parse_number_and_text(
    value: &str,
    book: &CanonicalBook,
    kind: &str,
) -> Result<(u16, String), CorpusError> {
    let split = value
        .find(char::is_whitespace)
        .ok_or_else(|| malformed(book, &format!("{kind} has no text")))?;
    let number = parse_bare_number(&value[..split], book, kind)?;
    let rest = &value[split..];
    let text = rest.strip_prefix(' ').unwrap_or(rest).to_owned();
    Ok((number, text))
}

fn marker_from_name(name: &str, book: &CanonicalBook) -> Result<Marker, CorpusError> {
    let marker = match name {
        "id" => Marker::Id,
        "ide" => Marker::Ide,
        "h" => Marker::Header,
        "toc1" => Marker::Toc1,
        "toc2" => Marker::Toc2,
        "toc3" => Marker::Toc3,
        "mt" => Marker::MainTitle,
        "mt1" => Marker::MainTitle1,
        "p" => Marker::Paragraph,
        "v" => Marker::Verse,
        "d" => Marker::DescriptiveTitle,
        "add" => Marker::Addition,
        "add*" => Marker::AdditionEnd,
        "f" => Marker::Footnote,
        "f*" => Marker::FootnoteEnd,
        "fr" => Marker::FootnoteReference,
        "fq" => Marker::FootnoteQuote,
        "ft" => Marker::FootnoteText,
        "rq" => Marker::ReferenceQuote,
        "rq*" => Marker::ReferenceQuoteEnd,
        _ => {
            return Err(CorpusError::UnsupportedMarker {
                book: book.id.clone(),
                marker: name.to_owned(),
            });
        }
    };
    Ok(marker)
}

fn is_header(marker: Marker) -> bool {
    matches!(
        marker,
        Marker::Id
            | Marker::Ide
            | Marker::Header
            | Marker::Toc1
            | Marker::Toc2
            | Marker::Toc3
            | Marker::MainTitle
            | Marker::MainTitle1
    )
}

fn validate_headers(names: &[String], book: &CanonicalBook) -> Result<(), CorpusError> {
    let observed: Vec<&str> = names.iter().map(String::as_str).collect();
    let mt = ["id", "ide", "h", "toc1", "toc2", "toc3", "mt"];
    let mt1 = ["id", "ide", "h", "toc1", "toc2", "toc3", "mt1"];
    if observed != mt && observed != mt1 {
        return Err(malformed(book, "header marker sequence is not canonical"));
    }
    Ok(())
}

fn update_inline_state(
    marker: Marker,
    state: &mut InlineState,
    book: &CanonicalBook,
) -> Result<(), CorpusError> {
    match marker {
        Marker::Addition if state.addition => {
            return Err(malformed(book, "nested add marker"));
        }
        Marker::Addition => state.addition = true,
        Marker::AdditionEnd if !state.addition => {
            return Err(malformed(book, "add* without add"));
        }
        Marker::AdditionEnd => state.addition = false,
        Marker::Footnote if state.footnote => {
            return Err(malformed(book, "nested f marker"));
        }
        Marker::Footnote => state.footnote = true,
        Marker::FootnoteEnd if !state.footnote => {
            return Err(malformed(book, "f* without f"));
        }
        Marker::FootnoteEnd => state.footnote = false,
        Marker::FootnoteReference | Marker::FootnoteQuote | Marker::FootnoteText
            if !state.footnote =>
        {
            return Err(malformed(book, "footnote submarker outside f/f*"));
        }
        Marker::ReferenceQuote if state.reference_quote => {
            return Err(malformed(book, "nested rq marker"));
        }
        Marker::ReferenceQuote => state.reference_quote = true,
        Marker::ReferenceQuoteEnd if !state.reference_quote => {
            return Err(malformed(book, "rq* without rq"));
        }
        Marker::ReferenceQuoteEnd => state.reference_quote = false,
        _ => {}
    }
    Ok(())
}

fn require_clear_state(
    state: &InlineState,
    book: &CanonicalBook,
    detail: &str,
) -> Result<(), CorpusError> {
    if !state.is_clear() {
        return Err(malformed(book, detail));
    }
    Ok(())
}

fn malformed(book: &CanonicalBook, detail: &str) -> CorpusError {
    CorpusError::MalformedUsfm {
        book: book.id.clone(),
        detail: detail.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_book(raw: &[u8]) -> CanonicalBook {
        CanonicalBook {
            ordinal: 1,
            id: "TST".to_owned(),
            title: "Teste".to_owned(),
            source_file: "test.txt".to_owned(),
            chapter_count: 1,
            verse_count: 1,
            raw_bytes: u32::try_from(raw.len()).expect("fixture length fits"),
            raw_sha256: String::new(),
        }
    }

    #[test]
    fn observed_markers_and_compatibility_markers_are_preserved() {
        let raw = b"\xef\xbb\xbf\\id TST fixture\r\n\\ide UTF-8\r\n\\h Teste\r\n\\toc1 Teste\r\n\\toc2 Teste\r\n\\toc3 Ts\r\n\\mt1 Teste\r\n\\c 1\r\n\\d Heading\\p\r\n\\v 1 Text \\add supplied\\add*  more \\f + \\fr 1:1 \\fq Text \\ft note\\f*\r\ncontinued.\r\n\\rq Ref 1:1 \\rq*\r\n";
        let parsed = parse(raw, &fixture_book(raw)).expect("fixture parses");
        assert_eq!(parsed.verse_count, 1);
        assert!(
            parsed.document.chapters[0]
                .markers
                .iter()
                .any(|record| record.marker == Marker::FootnoteReference)
        );
        let payload_a = serde_json::to_vec(&parsed.document).expect("serialize");
        let payload_b = serde_json::to_vec(&parsed.document).expect("serialize again");
        assert_eq!(payload_a, payload_b);
        assert!(payload_a.starts_with(b"{\"schema\":1,"));
    }

    #[test]
    fn unbalanced_inline_marker_is_rejected() {
        let raw = b"\\id TST fixture\n\\ide UTF-8\n\\h Teste\n\\toc1 Teste\n\\toc2 Teste\n\\toc3 Ts\n\\mt Teste\n\\c 1\n\\p\n\\v 1 bad \\add text\n";
        assert!(matches!(
            parse(raw, &fixture_book(raw)),
            Err(CorpusError::MalformedUsfm { .. })
        ));
    }

    #[test]
    fn line_and_marker_limits_are_enforced_before_unbounded_growth() {
        let mut long_line = String::from(
            "\\id TST fixture\n\\ide UTF-8\n\\h Teste\n\\toc1 Teste\n\\toc2 Teste\n\\toc3 Ts\n\\mt Teste\n\\c 1\n\\v 1 ",
        );
        long_line.push_str(&"x".repeat(MAX_LINE_BYTES));
        let long_bytes = long_line.into_bytes();
        assert!(matches!(
            parse(&long_bytes, &fixture_book(&long_bytes)),
            Err(CorpusError::LineTooLong { .. })
        ));

        let mut many_markers = String::from(
            "\\id TST fixture\n\\ide UTF-8\n\\h Teste\n\\toc1 Teste\n\\toc2 Teste\n\\toc3 Ts\n\\mt Teste\n\\c 1\n",
        );
        for _ in 0..MAX_MARKERS_PER_BOOK {
            many_markers.push_str("\\p\n");
        }
        many_markers.push_str("\\v 1 text\n");
        let marker_bytes = many_markers.into_bytes();
        assert!(matches!(
            parse(&marker_bytes, &fixture_book(&marker_bytes)),
            Err(CorpusError::TooManyMarkers { .. })
        ));
    }

    #[test]
    fn unknown_marker_is_rejected_instead_of_silently_dropped() {
        let raw = b"\\id TST fixture\n\\ide UTF-8\n\\h Teste\n\\toc1 Teste\n\\toc2 Teste\n\\toc3 Ts\n\\mt Teste\n\\c 1\n\\q1 heading\n\\v 1 text\n";
        assert!(matches!(
            parse(raw, &fixture_book(raw)),
            Err(CorpusError::UnsupportedMarker { .. })
        ));
    }
}
