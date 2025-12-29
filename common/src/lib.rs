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

pub fn oct2int(oct: &[u8]) -> u64 {
    let mut dec = 0;
    for c in oct {
        if *c < b'0' || *c > b'7' {
            break;
        }
        dec = dec * 8 + ((*c - b'0') as u64);
    }
    dec
}

#[cfg(test)]
mod test {
    use super::oct2int;

    #[test]
    fn test_oct2int() {
        let src = b"00000000007 ";
        assert_eq!(oct2int(src), 7);
    }
}
