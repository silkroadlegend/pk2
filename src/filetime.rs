use std::time::{Duration, SystemTime};

#[allow(non_snake_case)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}

impl FILETIME {
    const MS_EPOCH: u64 = 11_6444_7360_0000_0000;

    #[inline]
    pub fn now() -> Self {
        SystemTime::now().into()
    }

    pub fn into_systime(self) -> Option<SystemTime> {
        let FILETIME {
            dwLowDateTime: low,
            dwHighDateTime: high,
        } = self;
        let ftime = ((high as u64) << 32) | low as u64;
        let nanos = (ftime.checked_sub(Self::MS_EPOCH)?) * 100;
        Some(SystemTime::UNIX_EPOCH + Duration::from_nanos(nanos))
    }
}

impl From<SystemTime> for FILETIME {
    fn from(time: SystemTime) -> Self {
        let duration = time.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        let ftime = (duration.as_nanos() / 100) as u64 + Self::MS_EPOCH;
        FILETIME {
            dwLowDateTime: ftime as u32,
            dwHighDateTime: (ftime >> 32) as u32,
        }
    }
}

#[test]
fn test_convert_roundtrip() {
    let now = SystemTime::now();
    let ftime = FILETIME::from(now);
    assert_eq!(Some(now), ftime.into_systime());
}
