use crate::paging::PageSize;
use core::error::Error;
use core::fmt::{Debug, Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum PagingError {
    MisalignedAddress(usize, PageSize),
}

impl Display for PagingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            PagingError::MisalignedAddress(addr, size) => {
                write!(f, "misaligned address {:x} for page size: {}", addr, size)
            }
        }
    }
}

impl Error for PagingError {}
