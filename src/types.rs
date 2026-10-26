#[repr(C)]
pub struct InlineStr<const N: usize> {
    pub data: [u8; N],
}

impl Default for InlineStr<128> {
    fn default() -> Self {
        Self { data: [0; 128] }
    }
}

impl<const N: usize> InlineStr<N> {
    pub fn new(s: &str) -> Self {
        let mut data = [0; N];
        let len = s.len();

        if len >= N {
            panic!("String length exceeds maximum size of {}", N);
        }
        data[..len].copy_from_slice(&s.as_bytes()[..len]);
        Self { data }
    }
}
