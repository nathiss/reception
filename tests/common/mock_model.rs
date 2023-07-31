use std::fmt::Display;

lazy_static::lazy_static! {
    static ref VALID_DATA: Vec<u8> = vec![42u8, 42u8];
    static ref INVALID_DATA: Vec<u8> = vec![13u8, 37u8];
}

#[derive(Debug)]
pub(crate) struct MockModelError {}

impl Display for MockModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MockModelError")
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct MockModel {
    data: Vec<u8>,
}

impl MockModel {
    #[inline(always)]
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    #[inline(always)]
    pub(crate) fn valid() -> Self {
        Self::new(VALID_DATA.clone())
    }

    pub(crate) fn valid_with_size(size: usize) -> Self {
        let mut data = Vec::with_capacity(size);
        for _ in 0..size {
            data.push(42u8);
        }

        Self::new(data)
    }

    #[inline(always)]
    pub(crate) fn invalid() -> Self {
        Self::new(INVALID_DATA.clone())
    }
}

impl Into<Vec<u8>> for MockModel {
    fn into(self) -> Vec<u8> {
        self.data
    }
}

impl TryFrom<Vec<u8>> for MockModel {
    type Error = MockModelError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if *INVALID_DATA == value {
            return Err(MockModelError {});
        }
        Ok(Self::new(value))
    }
}
