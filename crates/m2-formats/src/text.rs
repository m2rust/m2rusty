//! Line and token splitting of `CMemoryTextFileLoader`
//! (`neoeter/src/platform/base/FileLoader.cpp`), byte for byte.

/// Splits like `CMemoryTextFileLoader::Bind`.
///
/// `\n`, `\r`, `\r\n`, `\n\r` and `\r\r`/`\n\n` pairs each end one line, and
/// the text after the last terminator is always a line, so a trailing newline
/// yields an empty last line. A byte >= 0x80 starts a two-byte (CP949)
/// character: it and the byte after it are copied without being checked for
/// a terminator.
pub fn lines(buf: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut pos = 0;

    while pos < buf.len() {
        let c = buf[pos];
        pos += 1;

        if c == b'\n' || c == b'\r' {
            let line = &buf[start..pos - 1];
            if pos < buf.len() && (buf[pos] == b'\n' || buf[pos] == b'\r') {
                pos += 1;
            }
            lines.push(line);
            start = pos;
        } else if c >= 0x80 {
            // C++ appends two bytes even when the lead byte is the last one,
            // reading one past the buffer; clamp instead.
            pos = (pos + 1).min(buf.len());
        }
    }

    lines.push(&buf[start..pos.min(buf.len())]);
    lines
}

/// Splits like `CMemoryTextFileLoader::SplitLineByTab`: on every tab, keeping
/// empty fields between tabs but not after a trailing tab. An empty line has
/// no tokens.
pub fn split_by_tab(line: &[u8]) -> Vec<&[u8]> {
    let mut tokens = Vec::new();
    let mut base = 0;

    while base < line.len() {
        match line[base..].iter().position(|&b| b == b'\t') {
            Some(offset) => {
                tokens.push(&line[base..base + offset]);
                base += offset + 1;
            }
            None => {
                tokens.push(&line[base..]);
                break;
            }
        }
    }
    tokens
}

/// The default `c_szDelimeter` of `SplitLine` and `SplitLine2`.
pub const DEFAULT_DELIMITERS: &[u8] = b" \t";

/// Why `split_line2` produced no tokens (its non-zero return values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitError {
    /// `-1`: the line is empty or only delimiters.
    Empty,
    /// `-2`: a `"` opens a token that is never closed.
    UnterminatedQuote,
}

/// Splits like `CMemoryTextFileLoader::SplitLine2`: tokens are runs of
/// non-delimiter bytes, or the bytes between a `"` that starts a token and
/// the next `"` (which may be empty and may contain delimiters). A `"`
/// inside an unquoted token is an ordinary byte, and the token after a
/// closing quote starts right after it even without a delimiter in between.
pub fn split_line2<'a>(line: &'a [u8], delimiters: &[u8]) -> Result<Vec<&'a [u8]>, SplitError> {
    let not_delimiter = |from: usize| (from..line.len()).find(|&i| !delimiters.contains(&line[i]));

    let mut tokens = Vec::new();
    let mut next = not_delimiter(0);
    while let Some(mut begin) = next {
        let (end, base) = if line[begin] == b'"' {
            begin += 1;
            let end = begin
                + line[begin..]
                    .iter()
                    .position(|&b| b == b'"')
                    .ok_or(SplitError::UnterminatedQuote)?;
            (end, end + 1)
        } else {
            let end = (begin..line.len()).find(|&i| delimiters.contains(&line[i])).unwrap_or(line.len());
            (end, end)
        };
        tokens.push(&line[begin..end]);
        next = not_delimiter(base);
    }

    if tokens.is_empty() { Err(SplitError::Empty) } else { Ok(tokens) }
}

/// Splits like `CMemoryTextFileLoader::SplitLine`: `split_line2` with both
/// failures folded into `None` (C++ returns `false`).
pub fn split_line<'a>(line: &'a [u8], delimiters: &[u8]) -> Option<Vec<&'a [u8]>> {
    split_line2(line, delimiters).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_line_tokens() {
        let split = |l: &'static [u8]| split_line2(l, DEFAULT_DELIMITERS);
        assert_eq!(split(b"  a\tbb  c "), Ok(vec![&b"a"[..], b"bb", b"c"]));
        assert_eq!(split(b"key \"a b\"\t\"\""), Ok(vec![&b"key"[..], b"a b", b""]));
        // A quote only matters at the start of a token.
        assert_eq!(split(b"a\"b c\""), Ok(vec![&b"a\"b"[..], b"c\""]));
        assert_eq!(split(b"\"a\"b"), Ok(vec![&b"a"[..], b"b"]));
        assert_eq!(split(b""), Err(SplitError::Empty));
        assert_eq!(split(b" \t "), Err(SplitError::Empty));
        assert_eq!(split(b"a \"b"), Err(SplitError::UnterminatedQuote));
        assert_eq!(split_line(b"a \"b", DEFAULT_DELIMITERS), None);
        assert_eq!(split_line(b"a|b||c", b"|"), Some(vec![&b"a"[..], b"b", b"c"]));
    }

    #[test]
    fn terminators() {
        assert_eq!(lines(b"a\nb\r\nc\rd"), [&b"a"[..], b"b", b"c", b"d"]);
        assert_eq!(lines(b"a\n"), [&b"a"[..], b""]);
        assert_eq!(lines(b""), [&b""[..]]);
        // A doubled terminator is one line break, a tripled one is two.
        assert_eq!(lines(b"a\n\nb"), [&b"a"[..], b"b"]);
        assert_eq!(lines(b"a\n\n\nb"), [&b"a"[..], b"", b"b"]);
    }

    #[test]
    fn double_byte_characters_hide_terminators() {
        // 0xB0 0x0A is one character, not a lead byte and a newline.
        assert_eq!(lines(b"\xb0\na\nb"), [&b"\xb0\na"[..], b"b"]);
        assert_eq!(lines(b"x\xb0"), [&b"x\xb0"[..]]);
    }

    #[test]
    fn tabs() {
        assert_eq!(split_by_tab(b"root\tZIP"), [&b"root"[..], b"ZIP"]);
        assert_eq!(split_by_tab(b"a\t\tb"), [&b"a"[..], b"", b"b"]);
        assert_eq!(split_by_tab(b"\tb"), [&b""[..], b"b"]);
        assert_eq!(split_by_tab(b"a\t"), [&b"a"[..]]);
        assert!(split_by_tab(b"").is_empty());
    }
}
