//! Keeps key secrets so the dashboard can show them again, encrypted to the Windows
//! account C0RE runs as (DPAPI). A copy of c0re.db on another account or PC can't be
//! decrypted. Elsewhere there's no such store, so secrets aren't kept at all.

use anyhow::Result;
use std::sync::Arc;

pub trait Vault: Send + Sync {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>>;
    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>>;
}

/// The vault for this system: DPAPI on Windows, none elsewhere.
pub fn system() -> Arc<dyn Vault> {
    #[cfg(windows)]
    {
        Arc::new(dpapi::Dpapi)
    }
    #[cfg(not(windows))]
    {
        Arc::new(Unavailable)
    }
}

/// No secure store: keys work, but can only be shown when created.
pub struct Unavailable;

impl Vault for Unavailable {
    fn seal(&self, _: &[u8]) -> Result<Vec<u8>> {
        anyhow::bail!("no secure store on this system")
    }
    fn open(&self, _: &[u8]) -> Result<Vec<u8>> {
        anyhow::bail!("no secure store on this system")
    }
}

#[cfg(windows)]
mod dpapi {
    use super::Vault;
    use anyhow::{Result, bail};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };

    /// Mixed into every secret, so other programs using DPAPI for this account can't
    /// decrypt C0RE's blobs by accident.
    const ENTROPY: &[u8] = b"C0RE key secret v1";

    pub struct Dpapi;

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    /// Copies DPAPI's output and frees it.
    unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let v = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe { LocalFree(out.pbData as _) };
        v
    }

    impl Vault for Dpapi {
        fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
            let (input, entropy) = (blob(plain), blob(ENTROPY));
            let mut out = CRYPT_INTEGER_BLOB::default();
            let ok = unsafe {
                CryptProtectData(
                    &input,
                    std::ptr::null(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out,
                )
            };
            if ok == 0 {
                bail!("Windows could not encrypt the key: {}", std::io::Error::last_os_error());
            }
            Ok(unsafe { take(out) })
        }

        fn open(&self, sealed: &[u8]) -> Result<Vec<u8>> {
            let (input, entropy) = (blob(sealed), blob(ENTROPY));
            let mut out = CRYPT_INTEGER_BLOB::default();
            let ok = unsafe {
                CryptUnprotectData(
                    &input,
                    std::ptr::null_mut(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out,
                )
            };
            if ok == 0 {
                bail!("Windows could not decrypt the key: {}", std::io::Error::last_os_error());
            }
            Ok(unsafe { take(out) })
        }
    }
}
