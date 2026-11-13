//! Windows ACL for `secrets.env`: a protected DACL with a single entry for the current user.
//!
//! Done through the Win32 API instead of `icacls`: `icacls /grant:r` keeps explicit
//! entries for other principals, and reading its output means parsing localized names.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_REVISION, AddAccessAllowedAce,
    DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetLengthSid, GetSecurityDescriptorControl,
    GetTokenInformation, InitializeAcl, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Replaces the file DACL with full access for the current user only, no inheritance.
pub fn restrict(path: &Path) -> anyhow::Result<()> {
    let user = token_user()?;
    let sid = user_sid(&user);
    unsafe {
        let sid_len = GetLengthSid(sid) as usize;
        let acl_len = size_of::<ACL>() + size_of::<ACCESS_ALLOWED_ACE>() + sid_len;
        let mut acl_buf = vec![0u64; acl_len.div_ceil(8)];
        let acl = acl_buf.as_mut_ptr() as *mut ACL;
        if InitializeAcl(acl, acl_len as u32, ACL_REVISION) == 0 {
            anyhow::bail!("InitializeAcl: {}", std::io::Error::last_os_error());
        }
        if AddAccessAllowedAce(acl, ACL_REVISION, FILE_ALL_ACCESS, sid) == 0 {
            anyhow::bail!("AddAccessAllowedAce: {}", std::io::Error::last_os_error());
        }
        let wide = wide(path);
        let info = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
        let code = SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            info,
            null_mut(),
            null_mut(),
            acl,
            null(),
        );
        if code != ERROR_SUCCESS {
            anyhow::bail!(
                "restringir ACL de {}: {}",
                path.display(),
                std::io::Error::from_raw_os_error(code as i32)
            );
        }
    }
    Ok(())
}

/// True if the DACL is protected and every allow entry is for the current user.
pub fn is_private(path: &Path) -> anyhow::Result<bool> {
    let user = token_user()?;
    let sid = user_sid(&user);
    unsafe {
        let wide = wide(path);
        let mut dacl: *mut ACL = null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        let code = GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut sd,
        );
        if code != ERROR_SUCCESS {
            anyhow::bail!(
                "ler ACL de {}: {}",
                path.display(),
                std::io::Error::from_raw_os_error(code as i32)
            );
        }
        let private = !dacl.is_null() && protected(sd) && only_user_allowed(dacl, sid);
        LocalFree(sd);
        Ok(private)
    }
}

unsafe fn protected(sd: PSECURITY_DESCRIPTOR) -> bool {
    let mut control = 0u16;
    let mut revision = 0u32;
    let ok = unsafe { GetSecurityDescriptorControl(sd, &mut control, &mut revision) };
    ok != 0 && control & SE_DACL_PROTECTED != 0
}

unsafe fn only_user_allowed(dacl: *mut ACL, sid: *mut c_void) -> bool {
    let count = unsafe { (*dacl).AceCount };
    for index in 0..count as u32 {
        let mut ace: *mut c_void = null_mut();
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 {
            return false;
        }
        let kind = unsafe { (*(ace as *const ACE_HEADER)).AceType } as u32;
        if kind == ACCESS_DENIED_ACE_TYPE {
            continue;
        }
        if kind != ACCESS_ALLOWED_ACE_TYPE {
            return false;
        }
        let ace_sid = unsafe { &raw const (*(ace as *const ACCESS_ALLOWED_ACE)).SidStart };
        if unsafe { EqualSid(ace_sid as *mut c_void, sid) } == 0 {
            return false;
        }
    }
    true
}

/// `TOKEN_USER` of the current process, in an 8-byte aligned buffer.
fn token_user() -> anyhow::Result<Vec<u64>> {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            anyhow::bail!("OpenProcessToken: {}", std::io::Error::last_os_error());
        }
        let mut len = 0u32;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut len);
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buf.as_mut_ptr() as *mut c_void,
            len,
            &mut len,
        );
        CloseHandle(token);
        if ok == 0 {
            anyhow::bail!("GetTokenInformation: {}", std::io::Error::last_os_error());
        }
        Ok(buf)
    }
}

fn user_sid(token_user: &[u64]) -> *mut c_void {
    unsafe { (*(token_user.as_ptr() as *const TOKEN_USER)).User.Sid }
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
