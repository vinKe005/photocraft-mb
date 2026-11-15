//! Bound content-provider allocations and distinguish failed reads from valid EOF.
use std::io::{self, Read};

pub(super) fn read_limited(reader: &mut impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = match reader.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(bytes);
        }
        let new_len = bytes.len().checked_add(count).ok_or_else(|| io::Error::other("Photo size overflow"))?;
        if new_len > limit {
            return Err(io::Error::other("This Android preview supports photos up to 128 MiB"));
        }
        let slice = chunk.get(..count).ok_or_else(|| io::Error::other("Invalid photo provider read size"))?;
        bytes.try_reserve(count).map_err(io::Error::other)?;
        bytes.extend_from_slice(slice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_limit_is_accepted_but_one_more_byte_is_rejected() {
        assert_eq!(read_limited(&mut &b"abcd"[..], 4).unwrap(), b"abcd");
        assert!(read_limited(&mut &b"abcde"[..], 4).is_err());
        assert!(read_limited(&mut &b"a"[..], 0).is_err());
        assert!(read_limited(&mut &b""[..], 0).unwrap().is_empty());
    }

    struct Provider {
        attempts: u8,
        fails: bool,
    }

    impl Read for Provider {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.attempts += 1;
            match self.attempts {
                1 => Err(io::ErrorKind::Interrupted.into()),
                2 if self.fails => Err(io::ErrorKind::PermissionDenied.into()),
                2 => {
                    out[0] = 42;
                    Ok(1)
                }
                _ => Ok(0),
            }
        }
    }

    #[test]
    fn interrupted_provider_read_retries_and_permission_failure_is_not_eof() {
        assert_eq!(read_limited(&mut Provider { attempts: 0, fails: false }, 8).unwrap(), [42]);
        let error = read_limited(&mut Provider { attempts: 0, fails: true }, 8).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn reads_spanning_multiple_jni_sized_chunks_are_bounded() {
        let photo = vec![1; 16385];
        assert_eq!(read_limited(&mut &photo[..], photo.len()).unwrap(), photo);
        assert!(read_limited(&mut &photo[..], 16384).is_err());
    }
}
