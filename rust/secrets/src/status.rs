//! A keychain's lock state, and its unlock, with no window and no process
//! (issue 19 step B, main m_13728): Security.framework's SecKeychainOpen,
//! SecKeychainGetStatus and SecKeychainUnlock, the only FFI of the crate,
//! all its `unsafe` here (architect m_13735).
//!
//! KEYCHAIN LEVEL ONLY. Items are read and written through
//! /usr/bin/security, never through this FFI: the items `security`
//! created have it in their access list, so it never asks; an item read
//! from the bise binary would not be in that list and macOS would show
//! a dialog. Don't "optimize" item reads into here.
//!
//! Measured (python ctypes probe, the same calls): an unlocked file = 7, a
//! locked one = 2 (kSecUnlockStateStatus clear), a missing file = -25294,
//! a file the sandbox can't read = an error (100022). No dialog, ~µs.

use std::path::Path;

/// A keychain's state, read without asking anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Unlocked,
    Locked,
    /// no such keychain file
    Missing,
    /// it can't be opened or read (the sandbox's read deny, not macOS)
    Unreadable,
}

#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::c_void;
    pub type OSStatus = i32;
    pub type SecKeychainRef = *mut c_void;
    pub const NO_SUCH_KEYCHAIN: OSStatus = -25294;
    pub const UNLOCKED: u32 = 1;

    #[link(name = "Security", kind = "framework")]
    extern "C" {
        pub fn SecKeychainOpen(path: *const std::ffi::c_char, kc: *mut SecKeychainRef) -> OSStatus;
        pub fn SecKeychainCopyDefault(kc: *mut SecKeychainRef) -> OSStatus;
        pub fn SecKeychainGetStatus(kc: SecKeychainRef, status: *mut u32) -> OSStatus;
        pub fn SecKeychainUnlock(kc: SecKeychainRef, len: u32, password: *const c_void, use_password: u8) -> OSStatus;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFRelease(cf: *const c_void);
    }
}

/// An open keychain reference, released on drop.
#[cfg(target_os = "macos")]
struct Open(ffi::SecKeychainRef);

#[cfg(target_os = "macos")]
impl Drop for Open {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: a reference SecKeychainOpen/CopyDefault returned, released once
            unsafe { ffi::CFRelease(self.0) }
        }
    }
}

#[cfg(target_os = "macos")]
fn open(file: Option<&Path>) -> Result<Open, ()> {
    let mut kc: ffi::SecKeychainRef = std::ptr::null_mut();
    let r = match file {
        Some(f) => {
            let c = std::ffi::CString::new(f.as_os_str().as_encoded_bytes()).map_err(|_| ())?;
            // SAFETY: a NUL-terminated path and an out pointer
            unsafe { ffi::SecKeychainOpen(c.as_ptr(), &mut kc) }
        }
        // SAFETY: an out pointer
        None => unsafe { ffi::SecKeychainCopyDefault(&mut kc) },
    };
    if r == 0 && !kc.is_null() {
        Ok(Open(kc))
    } else {
        Err(())
    }
}

/// The state of the keychain at `file` (None: the user's default one, his
/// login keychain). Asks nothing, runs nothing.
#[cfg(target_os = "macos")]
pub fn status(file: Option<&Path>) -> Status {
    if file.is_some_and(|f| !f.exists()) {
        return Status::Missing;
    }
    let Ok(kc) = open(file) else { return Status::Unreadable };
    let mut s: u32 = 0;
    // SAFETY: an open keychain and an out pointer
    match unsafe { ffi::SecKeychainGetStatus(kc.0, &mut s) } {
        0 if s & ffi::UNLOCKED != 0 => Status::Unlocked,
        0 => Status::Locked,
        ffi::NO_SUCH_KEYCHAIN => Status::Missing,
        _ => Status::Unreadable,
    }
}

/// Unlock the keychain at `file` with its password (no dialog: the
/// password is given).
#[cfg(target_os = "macos")]
pub fn unlock(file: &Path, password: &str) -> Result<(), String> {
    let kc = open(Some(file)).map_err(|_| "bise's keychain can't be opened".to_string())?;
    // SAFETY: an open keychain, a buffer and its length
    let r = unsafe { ffi::SecKeychainUnlock(kc.0, password.len() as u32, password.as_ptr().cast(), 1) };
    if r == 0 {
        Ok(())
    } else {
        Err(format!("bise's keychain didn't unlock (OSStatus {r})"))
    }
}

/// No keychain off macOS (store = keychain is macOS only).
#[cfg(not(target_os = "macos"))]
pub fn status(_file: Option<&Path>) -> Status {
    Status::Unreadable
}

#[cfg(not(target_os = "macos"))]
pub fn unlock(_file: &Path, _password: &str) -> Result<(), String> {
    Err("no keychain here".into())
}
