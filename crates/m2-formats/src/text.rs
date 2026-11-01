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

#[cfg(test)]
mod tests {
    use super::*;

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
