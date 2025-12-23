#![no_std]

#[derive(Debug, Clone, Copy)]
pub enum Syscall {
    PUTCHAR,
    GETCHAR,
    EXIT,
}

impl Into<u64> for Syscall {
    fn into(self) -> u64 {
        match self {
            Self::PUTCHAR => 1,
            Self::GETCHAR => 2,
            Self::EXIT => 3,
        }
    }
}

impl TryFrom<u64> for Syscall {
    type Error = u64;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::PUTCHAR),
            2 => Ok(Self::GETCHAR),
            3 => Ok(Self::EXIT),
            _ => Err(value),
        }
    }
}
