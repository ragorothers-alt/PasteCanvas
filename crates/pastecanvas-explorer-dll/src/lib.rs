//! PasteCanvas Windows Explorer context-menu COM server (CAP-007).
//!
//! Implements `IExplorerCommand` so Windows 10/11 can show "Send to PasteCanvas"
//! entries. When invoked, it collects the selected file paths and launches the
//! `pastecanvas-explorer` bridge binary (which captures into the local SQLite DB).
//! This keeps Explorer non-blocking (per PRD CAP-007) — the heavy work runs in a
//! separate process.
//!
//! Per AGENTS.md: all Windows-only COM code is isolated in this platform crate.

#![cfg(windows)]
#![allow(non_snake_case, clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, E_FAIL, E_NOINTERFACE, E_OUTOFMEMORY, E_POINTER, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    CoTaskMemAlloc, CoTaskMemFree, IClassFactory, IClassFactory_Impl,
};
use windows::Win32::UI::Shell::{
    IEnumExplorerCommand, IExplorerCommand, IExplorerCommand_Impl, IShellItemArray,
    SIGDN_FILESYSPATH,
};
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, PWSTR, implement};

/// ECS_ENABLED = the command is enabled in the menu.
const ECS_ENABLED: u32 = 1;
/// E_NOTIMPL HRESULT.
const E_NOTIMPL: HRESULT = HRESULT(0x80004001u32 as i32);

/// Stable CLSIDs for the two explorer commands (registered by the desktop app).
const CLSID_INBOX: GUID = GUID::from_u128(0x7e4a0c1a_6f2c_4a31_9c7a_1a2b3c4d5e01);
const CLSID_BOARD: GUID = GUID::from_u128(0x7e4a0c1a_6f2c_4a31_9c7a_1a2b3c4d5e02);

/// Global module ref count for DllCanUnloadNow.
static MODULE_REFCOUNT: AtomicU32 = AtomicU32::new(0);

/// The explorer command. `to_board` selects Inbox (false) vs Board (true).
#[implement(IExplorerCommand)]
struct PasteCanvasCommand {
    to_board: bool,
}

/// Allocates a CoTaskMem-backed PWSTR copy of a Rust string (the shell frees it).
fn alloc_pwstr(s: &str) -> windows::core::Result<PWSTR> {
    let wide: Vec<u16> = s.encode_utf16().collect();
    let byte_len = (wide.len() + 1) * 2;
    unsafe {
        let ptr = CoTaskMemAlloc(byte_len) as *mut u16;
        if ptr.is_null() {
            return Err(E_OUTOFMEMORY.into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        ptr.add(wide.len()).write(0);
        Ok(PWSTR(ptr))
    }
}

/// Collects filesystem paths from a shell item array.
fn collect_paths(items: &IShellItemArray) -> Vec<String> {
    let mut paths = Vec::new();
    let count = unsafe { items.GetCount() }.unwrap_or(0);
    for i in 0..count {
        if let Ok(item) = unsafe { items.GetItemAt(i) } {
            if let Ok(name) = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) } {
                let s = unsafe { name.to_string() }.unwrap_or_default();
                if !s.is_empty() {
                    paths.push(s);
                }
                unsafe { CoTaskMemFree(Some(name.0 as *const c_void)) };
            }
        }
    }
    paths
}

/// Resolves the pastecanvas-explorer bridge path (next to this DLL).
///
/// Must use this DLL's module handle. `GetModuleFileNameW(NULL)` returns the
/// host process (`explorer.exe` / `dllhost.exe`) and would look for the bridge
/// next to Explorer itself.
fn this_dll_dir() -> Option<std::path::PathBuf> {
    const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x0000_0004;
    const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 0x0000_0002;
    unsafe {
        let mut module = std::ptr::null_mut();
        let ok = GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            DllGetClassObject as *const u16,
            &mut module,
        );
        if ok == 0 || module.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let len = GetModuleFileNameW(module, buf.as_mut_ptr(), buf.len() as u32);
        if len == 0 {
            return None;
        }
        let dll_path = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&dll_path).parent().map(|p| p.to_path_buf())
    }
}

fn bridge_exe_path() -> Option<std::path::PathBuf> {
    Some(this_dll_dir()?.join("pastecanvas-explorer.exe"))
}

impl IExplorerCommand_Impl for PasteCanvasCommand_Impl {
    fn GetTitle(
        &self,
        _items: windows::core::Ref<IShellItemArray>,
    ) -> windows::core::Result<PWSTR> {
        let title = if self.to_board {
            "Send to PasteCanvas Board"
        } else {
            "Send to PasteCanvas Inbox"
        };
        alloc_pwstr(title)
    }

    fn GetIcon(&self, _items: windows::core::Ref<IShellItemArray>) -> windows::core::Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetToolTip(
        &self,
        _items: windows::core::Ref<IShellItemArray>,
    ) -> windows::core::Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> windows::core::Result<GUID> {
        Ok(if self.to_board {
            CLSID_BOARD
        } else {
            CLSID_INBOX
        })
    }

    fn GetState(
        &self,
        _items: windows::core::Ref<IShellItemArray>,
        _ok_to_be_slow: BOOL,
    ) -> windows::core::Result<u32> {
        Ok(ECS_ENABLED)
    }

    fn Invoke(
        &self,
        items: windows::core::Ref<IShellItemArray>,
        _bc: windows::core::Ref<windows::Win32::System::Com::IBindCtx>,
    ) -> windows::core::Result<()> {
        let items = items.as_ref().ok_or(E_POINTER)?;
        let paths = collect_paths(items);
        if paths.is_empty() {
            return Err(E_FAIL.into());
        }

        let exe = bridge_exe_path().ok_or(E_FAIL)?;
        let mut cmd = std::process::Command::new(exe);
        if self.to_board {
            cmd.arg("--board");
        }
        for p in &paths {
            cmd.arg(p);
        }
        // Detached background process — never blocks Explorer (CAP-007).
        cmd.spawn()
            .map_err(|_| windows::core::Error::from(E_FAIL))?;
        Ok(())
    }

    fn GetFlags(&self) -> windows::core::Result<u32> {
        Ok(0)
    }

    fn EnumSubCommands(&self) -> windows::core::Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

/// Class factory producing PasteCanvasCommand instances.
#[implement(IClassFactory)]
struct PasteCanvasFactory {
    to_board: bool,
}

impl IClassFactory_Impl for PasteCanvasFactory_Impl {
    fn CreateInstance(
        &self,
        outer: windows::core::Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        // Aggregation is not supported.
        if !outer.is_null() {
            return Err(E_FAIL.into());
        }
        unsafe { *ppvobject = std::ptr::null_mut() };

        let cmd: IExplorerCommand = PasteCanvasCommand {
            to_board: self.to_board,
        }
        .into();
        let unknown: IUnknown = cmd.cast()?;
        unsafe { unknown.query(riid, ppvobject).ok() }
    }

    fn LockServer(&self, increment: BOOL) -> windows::core::Result<()> {
        if increment.as_bool() {
            MODULE_REFCOUNT.fetch_add(1, Ordering::SeqCst);
        } else {
            MODULE_REFCOUNT.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

// --- DLL exports ---

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)] // COM export ABI cannot be `unsafe`
pub extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return E_POINTER;
        }
        *ppv = std::ptr::null_mut();

        let clsid = *rclsid;
        let to_board = if clsid == CLSID_BOARD {
            true
        } else if clsid == CLSID_INBOX {
            false
        } else {
            return CLASS_E_CLASSNOTAVAILABLE;
        };

        let factory: IClassFactory = PasteCanvasFactory { to_board }.into();
        let unknown: IUnknown = match factory.cast() {
            Ok(u) => u,
            Err(e) => return e.code(),
        };
        // QueryInterface fills *ppv and adds a reference; dropping `unknown`
        // releases our local reference, leaving one held by the caller.
        let hr = unknown.query(riid, ppv);
        if hr.is_err() {
            *ppv = std::ptr::null_mut();
        }
        hr
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if MODULE_REFCOUNT.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

// kernel32 — avoid pulling extra windows features just for module path lookup.
unsafe extern "system" {
    fn GetModuleFileNameW(hmodule: *mut c_void, filename: *mut u16, nsize: u32) -> u32;
    fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut *mut c_void) -> i32;
}

// Referenced to keep the unused-import lint quiet if a variant is not built.
#[allow(dead_code)]
const _E_NOINTERFACE_REF: HRESULT = E_NOINTERFACE;
