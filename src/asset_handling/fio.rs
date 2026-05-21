use core::alloc::Layout;

use alloc::{
    alloc::dealloc,
    ffi::CString,
    format,
    string::{String, ToString},
};
use psp::sys::{IoOpenFlags, SceIoStat, SceUid, sceIoGetstat, sceIoOpen};

pub struct File {
    fd: SceUid,
    size: i64,
}

impl File {
    // TODO: Look into using the async functions provided in the Io api
    pub fn new(filepath: String, io_flags: IoOpenFlags) -> Result<File, super::IoError> {
        unsafe {
            let path = CString::new(filepath).map_err(|_| {
                super::IoError(format!(
                    "{}",
                    "Error in converting filepath to CString".to_string()
                ))
            })?;

            let stat_layout = Layout::new::<SceIoStat>();
            let stats = alloc::alloc::alloc_zeroed(stat_layout) as *mut SceIoStat;
            if sceIoGetstat(path.as_ptr() as *const u8, stats) < 0 {
                dealloc(stats as *mut u8, stat_layout);
                return Err(super::IoError(format!("Could not find file: {:?}", path)));
            }

            let fd = sceIoOpen(path.as_ptr() as *const u8, io_flags, 0777);
            if fd.0 < 0 {
                return Err(super::IoError(format!("Failed to open file: {:?}.", path)));
            }

            let size = (*stats).st_size;

            dealloc(stats as *mut u8, stat_layout);

            Ok(File { fd, size })
        }
    }

    pub fn fd(&self) -> SceUid {
        self.fd
    }

    pub fn size(&self) -> i64 {
        self.size
    }
}
