//! `CPythonNetworkStream::LoadInsultList` and `LoadConvertTable`
//! (`client/src/ui/PythonNetworkStreamPhaseLoading.cpp`).

use crate::text;

/// `LoadInsultList`: every non-empty line, in order
/// (`CInsultChecker::AppendInsult`).
pub fn parse_insult_list(data: &[u8]) -> Vec<Vec<u8>> {
    text::lines(data)
        .into_iter()
        .filter(|line| !line.is_empty())
        .map(<[u8]>::to_vec)
        .collect()
}

/// `CPythonNetworkStream::STextConvertTable`, one `lang%d.cvt`: the
/// substitution an empire's chat goes through for the other empires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextConvertTable {
    pub upper: [u8; 26],
    pub lower: [u8; 26],
    /// One pair per KS X 1001 Hangul syllable (lead 0xB0..=0xC8, trail
    /// 0xA1..=0xFE); the C++ array has room for 5000, only these are read.
    pub han: Vec<[u8; 2]>,
}

impl TextConvertTable {
    /// Hangul pairs in the file.
    pub const HAN_COUNT: usize = (0xc8 - 0xb0 + 1) * (0xfe - 0xa1 + 1);
    /// The size `LoadConvertTable` requires (longer files are fine).
    pub const FILE_SIZE: usize = 26 * 2 + Self::HAN_COUNT * 2;

    /// `LoadConvertTable` after the file is read: `None` when it is too short.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::FILE_SIZE {
            return None;
        }
        Some(Self {
            upper: data[..26].try_into().unwrap(),
            lower: data[26..52].try_into().unwrap(),
            han: data[52..Self::FILE_SIZE].as_chunks::<2>().0.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insults() {
        assert_eq!(
            parse_insult_list(b"a\n\nb\r\nc"),
            [b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
        );
        assert!(parse_insult_list(b"").is_empty());
    }

    #[test]
    fn convert_table() {
        assert_eq!(TextConvertTable::FILE_SIZE, 4752);
        assert!(TextConvertTable::parse(&[0; 4751]).is_none());
        let data: Vec<u8> = (0..4800).map(|i| i as u8).collect();
        let t = TextConvertTable::parse(&data).unwrap();
        assert_eq!(t.upper[0], 0);
        assert_eq!(t.lower[0], 26);
        assert_eq!(t.han.len(), 2350);
        assert_eq!(t.han[0], [52, 53]);
    }
}
