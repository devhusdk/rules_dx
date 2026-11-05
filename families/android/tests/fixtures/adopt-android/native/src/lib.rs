const MOD: u32 = 65521;

fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() || len == 0 {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

pub fn word_count(data: &[u8]) -> usize {
    data.split(|byte| byte.is_ascii_whitespace())
        .filter(|word| !word.is_empty())
        .count()
}

pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + u32::from(*byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

#[no_mangle]
pub extern "C" fn adopt_word_count(ptr: *const u8, len: usize) -> usize {
    word_count(bytes(ptr, len))
}

#[no_mangle]
pub extern "C" fn adopt_adler32(ptr: *const u8, len: usize) -> u32 {
    adler32(bytes(ptr, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words() {
        assert_eq!(word_count(b"  one two\nthree "), 3);
        assert_eq!(word_count(b""), 0);
    }

    #[test]
    fn checksums_like_adler32() {
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn c_entry_points_match() {
        let text = b"hello world";
        assert_eq!(adopt_word_count(text.as_ptr(), text.len()), 2);
        assert_eq!(adopt_adler32(text.as_ptr(), text.len()), adler32(text));
        assert_eq!(adopt_word_count(std::ptr::null(), 4), 0);
    }
}
