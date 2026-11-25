//! Test helpers: builds STORED zip archives with arbitrary raw (e.g. CP949) entry names.

pub fn make_zip(entries: &[(&[u8], bool, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, utf8, data) in entries {
        let flags: u16 = if *utf8 { 0x0800 } else { 0 };
        let crc = crc32fast::hash(data);
        let offset = out.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes()); // version made by
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // version needed
            h.extend_from_slice(&flags.to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // stored
            h.extend_from_slice(&0u32.to_le_bytes()); // dos time/date
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra len
            if central {
                h.extend_from_slice(&0u16.to_le_bytes()); // comment len
                h.extend_from_slice(&0u16.to_le_bytes()); // disk
                h.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
                h.extend_from_slice(&0u32.to_le_bytes()); // external attrs
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name);
            h
        };
        out.extend(header(0x04034b50, false));
        out.extend_from_slice(data);
        central.extend(header(0x02014b50, true));
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x06054b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

pub fn cp949(s: &str) -> Vec<u8> {
    encoding_rs::EUC_KR.encode(s).0.into_owned()
}

pub const JPG: &[u8] = b"\xFF\xD8\xFF\xE0fakejpeg";
