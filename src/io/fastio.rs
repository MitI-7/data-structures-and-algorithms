use crate::numeric::number::Number;
use std::io::Read;

pub struct Scanner {
    buf: Vec<u8>,
    idx: usize,
}

impl Scanner {
    pub fn new() -> Self {
        Self::from_reader(std::io::stdin())
    }

    pub fn from_stdin() -> Self {
        #[cfg(unix)]
        {
            unsafe extern "C" {
                fn read(fd: std::ffi::c_int, buf: *mut std::ffi::c_void, count: usize) -> isize;
            }
            let mut buf = Vec::<u8>::with_capacity(32 * 1024);
            loop {
                if buf.len() == buf.capacity() {
                    buf.reserve(32 * 1024);
                }
                let len = buf.len();
                let count =
                    unsafe { read(0, buf.as_mut_ptr().add(len).cast(), buf.capacity() - len) };
                if count < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    panic!("stdin: {error}");
                }
                if count == 0 {
                    break;
                }
                unsafe { buf.set_len(len + count as usize) };
            }
            buf.push(b' ');
            Self { buf, idx: 0 }
        }
        #[cfg(not(unix))]
        Self::new()
    }

    pub fn from_reader(mut reader: impl Read) -> Self {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        buf.push(b' ');
        Self { buf, idx: 0 }
    }

    #[inline(always)]
    pub fn number<T: Number>(&mut self) -> T {
        let b = &self.buf;
        let mut i = self.idx;
        while b[i] <= b' ' {
            i += 1;
        }
        let neg = b[i] == b'-';
        if neg {
            i += 1;
        }
        let ten = T::from(10i8);
        let mut val = T::zero();
        while b[i].is_ascii_digit() {
            val = val * ten + T::from((b[i] - b'0') as i8);
            i += 1;
        }
        self.idx = i;
        if neg { -val } else { val }
    }

    #[inline(always)]
    pub fn usize(&mut self) -> usize {
        self.number::<i64>() as usize
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

pub trait AppendInteger {
    fn append_to(self, out: &mut Vec<u8>);
}

macro_rules! append_integer {
    ($($t:ty => $len:expr),* $(,)?) => {
        $(
            impl AppendInteger for $t {
                #[inline(always)]
                fn append_to(self, out: &mut Vec<u8>) {
                    let mut buf = [0u8; $len];
                    let mut n = self.unsigned_abs();
                    let mut len = 0;
                    loop {
                        buf[len] = (n % 10) as u8 + b'0';
                        len += 1;
                        n /= 10;
                        if n == 0 {
                            break;
                        }
                    }
                    if self < 0 {
                        out.push(b'-');
                    }
                    for i in (0..len).rev() {
                        out.push(buf[i]);
                    }
                    out.push(b'\n');
                }
            }
        )*
    };
}

append_integer!(i16 => 6, i32 => 11, i64 => 20, i128 => 40, isize => 20);

#[inline(always)]
pub fn append_num<T: AppendInteger>(out: &mut Vec<u8>, value: T) {
    value.append_to(out);
}

pub fn write_stdout(mut data: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn write(fd: std::ffi::c_int, buf: *const std::ffi::c_void, count: usize) -> isize;
        }
        while !data.is_empty() {
            let count = unsafe { write(1, data.as_ptr().cast(), data.len()) };
            if count < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if count == 0 {
                return Err(std::io::ErrorKind::WriteZero.into());
            }
            data = &data[count as usize..];
        }
        Ok(())
    }
    #[cfg(not(unix))]
    std::io::Write::write_all(&mut std::io::stdout(), data)
}
