/// Bộ hàm phân tích JSON tối giản không phụ thuộc thư viện ngoài (dùng cho Updater và Anti hook/progress)
/// kèm hàm băm SHA-256 thuần Rust.
pub fn extract_json_string_field_top_level(obj: &str, target_key: &str) -> Option<String> {
    let bytes = obj.as_bytes();
    let mut i = 0_usize;
    let mut depth = 0_i32;

    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                i += 1;
            }
            b'"' => {
                let (key_str, after_key) = parse_json_string(obj, i)?;
                i = skip_ws(bytes, after_key);
                if depth == 1 && i < bytes.len() && bytes[i] == b':' {
                    i = skip_ws(bytes, i + 1);
                    if key_str == target_key && i < bytes.len() && bytes[i] == b'"' {
                        let (val, _) = parse_json_string(obj, i)?;
                        return Some(val);
                    }
                }
            }
            _ => i += 1,
        }
    }
    None
}

pub fn extract_json_bool_field_top_level(obj: &str, target_key: &str) -> Option<bool> {
    let raw = extract_json_primitive_token_top_level(obj, target_key)?;
    match raw {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub fn extract_json_u64_field_top_level(obj: &str, target_key: &str) -> Option<u64> {
    let raw = extract_json_primitive_token_top_level(obj, target_key)?;
    raw.parse::<u64>().ok()
}

pub fn extract_json_object_section<'a>(json: &'a str, target_key: &str) -> Option<&'a str> {
    extract_json_bracket_section(json, target_key, b'{', b'}')
}

pub fn extract_json_array_section<'a>(json: &'a str, target_key: &str) -> Option<&'a str> {
    extract_json_bracket_section(json, target_key, b'[', b']')
}

pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn extract_json_primitive_token_top_level<'a>(obj: &'a str, target_key: &str) -> Option<&'a str> {
    let bytes = obj.as_bytes();
    let mut i = 0_usize;
    let mut depth = 0_i32;

    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                i += 1;
            }
            b'"' => {
                let (key_str, after_key) = parse_json_string(obj, i)?;
                i = skip_ws(bytes, after_key);
                if depth == 1 && i < bytes.len() && bytes[i] == b':' {
                    i = skip_ws(bytes, i + 1);
                    if key_str == target_key && i < bytes.len() {
                        let start = i;
                        while i < bytes.len()
                            && !matches!(bytes[i], b',' | b'}' | b']')
                            && !bytes[i].is_ascii_whitespace()
                        {
                            i += 1;
                        }
                        if i > start {
                            return Some(&obj[start..i]);
                        }
                        return None;
                    }
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn extract_json_bracket_section<'a>(
    json: &'a str,
    target_key: &str,
    open_ch: u8,
    close_ch: u8,
) -> Option<&'a str> {
    let bytes = json.as_bytes();
    let mut i = 0_usize;
    let mut depth = 0_i32;

    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                i += 1;
            }
            b'"' => {
                let (key_str, after_key) = parse_json_string(json, i)?;
                i = skip_ws(bytes, after_key);
                if depth == 1 && i < bytes.len() && bytes[i] == b':' {
                    i = skip_ws(bytes, i + 1);
                    if key_str == target_key && i < bytes.len() && bytes[i] == open_ch {
                        let start = i;
                        let end = find_matching_bracket(json, start, open_ch, close_ch)?;
                        return Some(&json[start..=end]);
                    }
                }
            }
            _ => i += 1,
        }
    }
    None
}

pub fn iter_json_objects_in_array(array_slice: &str) -> Vec<&str> {
    let bytes = array_slice.as_bytes();
    let mut out = Vec::new();
    let mut i = 0_usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                if let Some((_, next)) = parse_json_string(array_slice, i) {
                    i = next;
                } else {
                    break;
                }
            }
            b'{' => {
                if let Some(end) = find_matching_bracket(array_slice, i, b'{', b'}') {
                    out.push(&array_slice[i..=end]);
                    i = end + 1;
                } else {
                    break;
                }
            }
            _ => i += 1,
        }
    }
    out
}

fn find_matching_bracket(s: &str, open_idx: usize, open_ch: u8, close_ch: u8) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0_i32;
    let mut i = open_idx;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let (_, next) = parse_json_string(s, i)?;
            i = next;
            continue;
        }
        if bytes[i] == open_ch {
            depth += 1;
        } else if bytes[i] == close_ch {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn parse_json_string(s: &str, start_quote: usize) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.get(start_quote).copied() != Some(b'"') {
        return None;
    }
    let mut i = start_quote + 1;
    let mut out = String::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            return Some((out, i + 1));
        }
        if b == b'\\' {
            i += 1;
            let esc = *bytes.get(i)?;
            match esc {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'b' => out.push('\x08'),
                b'f' => out.push('\x0C'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    let hex_end = i.checked_add(5)?;
                    let hex = s.get(i + 1..hex_end)?;
                    let code = u32::from_str_radix(hex, 16).ok()?;
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    }
                    i += 4;
                }
                other => out.push(other as char),
            }
            i += 1;
        } else {
            let slice = s.get(i..)?;
            let ch = slice.chars().next()?;
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    None
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

pub fn parse_sha256_file(content: &str) -> Option<String> {
    let token = content.split_whitespace().next()?;
    if token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(token.to_ascii_lowercase())
    } else {
        None
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity((data.len() + 9).div_ceil(64) * 64);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut w = [0_u32; 64];
    for chunk in padded.as_chunks::<64>().0 {
        for (word, raw) in w[..16].iter_mut().zip(chunk.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*raw);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut hex = String::with_capacity(64);
    for word in state {
        for byte in word.to_be_bytes() {
            const DIGITS: &[u8; 16] = b"0123456789abcdef";
            hex.push(DIGITS[(byte >> 4) as usize] as char);
            hex.push(DIGITS[(byte & 0x0F) as usize] as char);
        }
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_nist_standard_vector_and_parses_hash_file() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let file = "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD  win-dynamic-island.exe\n";
        assert_eq!(
            parse_sha256_file(file).as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert!(parse_sha256_file("not-a-sha").is_none());
    }
}
