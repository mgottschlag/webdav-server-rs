//
// Linux specific systemcalls for quota.
//
use std::ffi::CString;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::os::raw::{c_char, c_int};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::{FsQuota, FsType, FqError, Mtab};

// The actual implementation is done in C, and imported here.
extern "C" {
    fn fs_quota_linux_ext(
        device: *const c_char,
        id: c_int,
        do_group: c_int,
        bytes_used: *mut u64,
        bytes_limit: *mut u64,
        files_used: *mut u64,
        files_limit: *mut u64,
    ) -> c_int;
    fn fs_quota_linux_xfs(
        device: *const c_char,
        id: c_int,
        do_group: c_int,
        bytes_used: *mut u64,
        bytes_limit: *mut u64,
        files_used: *mut u64,
        files_limit: *mut u64,
    ) -> c_int;
}

// wrapper for the C functions.
pub(crate) fn get_quota(device: impl AsRef<Path>, fstype: FsType, uid: u32) -> Result<FsQuota, FqError> {
    let id = uid as c_int;
    let device = device.as_ref();

    let mut bytes_used = 0u64;
    let mut bytes_limit = 0u64;
    let mut files_used = 0u64;
    let mut files_limit = 0u64;

    let path = CString::new(device.as_os_str().as_bytes())?;
    let rc = match fstype {
        FsType::LinuxExt => unsafe {
            fs_quota_linux_ext(
                path.as_ptr(),
                id,
                0,
                &mut bytes_used as *mut u64,
                &mut bytes_limit as *mut u64,
                &mut files_used as *mut u64,
                &mut files_limit as *mut u64,
            )
        },
        FsType::LinuxXfs => unsafe {
            fs_quota_linux_xfs(
                path.as_ptr(),
                id,
                0,
                &mut bytes_used as *mut u64,
                &mut bytes_limit as *mut u64,
                &mut files_used as *mut u64,
                &mut files_limit as *mut u64,
            )
        },
        _ => 1,
    };

    // Error mapping.
    match rc {
        0 => {
            let m = |v| if v == 0xffffffffffffffff { None } else { Some(v) };
            Ok(FsQuota {
                bytes_used:  bytes_used,
                bytes_limit: m(bytes_limit),
                files_used:  files_used,
                files_limit: m(files_limit),
            })
        },
        1 => Err(FqError::NoQuota),
        _ => Err(FqError::IoError(io::Error::last_os_error())),
    }
}

// read /etc/mtab.
pub(crate) fn read_mtab() -> io::Result<Vec<Mtab>> {
    let f = File::open("/etc/mtab")?;
    let reader = BufReader::new(f);
    let mut result = Vec::new();
    for l in reader.lines() {
        let l2 = l?;
        let line = l2.trim();
        if line.len() == 0 || line.starts_with("#") {
            continue;
        }
        let words = line.split_whitespace().collect::<Vec<_>>();
        if words.len() < 3 {
            continue;
        }
        let (host, device) = if words[2].starts_with("nfs") {
            if !words[0].contains(":") {
                continue;
            }
            let mut s = words[0].splitn(2, ':');
            let host = s.next().unwrap();
            let path = s.next().unwrap();
            (Some(host.to_string()), path)
        } else {
            (None, words[2])
        };
        result.push(Mtab {
            host:      host,
            device:    device.to_string(),
            directory: words[1].to_string(),
            fstype:    words[2].to_string(),
        });
    }
    Ok(result)
}

// The libc statvfs() function.
fn do_statvfs<P: AsRef<Path>>(path: P) -> io::Result<libc::statvfs> {
    let cpath = CString::new(path.as_ref().as_os_str().as_bytes())?;
    let mut sv = unsafe { std::mem::zeroed::<libc::statvfs>() };
    let rc = unsafe { libc::statvfs(cpath.as_ptr(), &mut sv) };
    if rc != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(sv)
    }
}

pub(crate) fn statvfs(path: impl AsRef<Path>) -> Result<FsQuota, FqError> {
    let vfs = do_statvfs(path).map_err(|e| FqError::IoError(e))?;
    Ok(FsQuota {
        bytes_used:  ((vfs.f_blocks - vfs.f_bfree) * vfs.f_bsize) as u64,
        bytes_limit: Some(((vfs.f_blocks - (vfs.f_bfree - vfs.f_bavail)) * vfs.f_bsize) as u64),
        files_used:  (vfs.f_files - vfs.f_ffree) as u64,
        files_limit: Some((vfs.f_files - (vfs.f_ffree - vfs.f_favail)) as u64),
    })
}

