#[repr(C)]
#[derive(Debug)]
pub(crate) enum SYSCALL {
    SchedYield = 0x18,
    Nanosleep = 0x23,
}

pub(crate) struct Unknown(pub u64);

impl TryFrom<u64> for SYSCALL {
    type Error = Unknown;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            value if value == Self::SchedYield as u64 => Ok(Self::SchedYield),
            value if value == Self::Nanosleep as u64 => Ok(Self::Nanosleep),
            _ => Err(Unknown(value)),
        }
    }
}
