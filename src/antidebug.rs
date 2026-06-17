//! Soft Anti-VM and Sandbox detection ("Ghost Mode").
//!
//! ## Why this exists in an open-source MIT project
//!
//! These functions are **gated behind the `stealth` feature flag** (`--features stealth`)
//! and are **not compiled by default**. They exist for advanced deployment scenarios
//! where the Microscope Memory binary runs on infrastructure that is also used by
//! malware analysts or automated sandboxes (e.g., CTF environments, public research
//! clusters). When the binary detects a high-confidence sandbox signature, it enables
//! "Ghost Mode" which adds timing jitter and IAT camouflage to avoid trivial
//! fingerprinting.
//!
//! Without `--features stealth`, all of this code compiles to a no-op.
//! The `stealth` feature is purely optional and does not affect the core memory engine.
//! (Red Audit - Phase 3)

use std::arch::x86_64::__cpuid;
use windows_sys::Win32::System::Registry::{RegOpenKeyExA, HKEY_LOCAL_MACHINE, KEY_READ};

/// Check for Hypervisor bit using CPUID (EAX=1, ECX bit 31).
pub fn has_hypervisor() -> bool {
    unsafe {
        let result = __cpuid(1);
        (result.ecx & (1 << 31)) != 0
    }
}

/// Check for VirtualBox/VMware registry keys.
pub fn has_vm_registry() -> bool {
    unsafe {
        let mut h_key = 0;
        let subkey = b"SYSTEM\\CurrentControlSet\\Enum\\PCI\\VEN_80EE&DEV_CAFE\0";
        let status = RegOpenKeyExA(HKEY_LOCAL_MACHINE, subkey.as_ptr(), 0, KEY_READ, &mut h_key);
        if status == 0 {
            return true;
        }

        let subkey_vmware = b"SOFTWARE\\VMware, Inc.\\VMware Tools\0";
        let status_vm = RegOpenKeyExA(
            HKEY_LOCAL_MACHINE,
            subkey_vmware.as_ptr(),
            0,
            KEY_READ,
            &mut h_key,
        );
        status_vm == 0
    }
}

/// Returns true if multiple VM indicators are present (Soft Detection).
pub fn is_sandbox() -> bool {
    let mut score = 0;
    if has_hypervisor() {
        score += 1;
    }
    if has_vm_registry() {
        score += 2;
    }

    // We only trigger "Ghost Mode" if we are highly certain (score >= 2)
    // This allows legitimate AWS/GCP (hypervisor=1 but no VBox registry) to run élesben.
    score >= 2
}
