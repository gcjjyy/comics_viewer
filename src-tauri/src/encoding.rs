/// Decodes a ZIP entry name. Names without the UTF-8 flag are UTF-8 if valid, otherwise CP949.
pub fn decode_name(raw: &[u8], utf8_flag: bool) -> String {
    if utf8_flag {
        return String::from_utf8_lossy(raw).into_owned();
    }
    match std::str::from_utf8(raw) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::EUC_KR.decode_without_bom_handling(raw).0.into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp949_without_flag() {
        let (raw, _, _) = encoding_rs::EUC_KR.encode("호문쿨루스 01권/001.jpg");
        assert_eq!(decode_name(&raw, false), "호문쿨루스 01권/001.jpg");
    }

    #[test]
    fn utf8_without_flag() {
        assert_eq!(decode_name("슬램덩크/01.jpg".as_bytes(), false), "슬램덩크/01.jpg");
    }

    #[test]
    fn utf8_with_flag() {
        assert_eq!(decode_name("a.jpg".as_bytes(), true), "a.jpg");
    }
}
