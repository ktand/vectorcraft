//! Fork: cutting machines over USB (`docs/cutting/ARCHITECTURE.md` › 6, R3). The app installs
//! the device provider `vectorcraft_cutter::registry` hands to the engine's `cutting.*` commands.
//!
//! On Windows, a Silhouette cutter is a USB printer-class device that the system's own
//! `usbprint.sys` drives (no vendor driver). It is found through the USB-printer device interface
//! and opened as a file: writes go to the machine, reads bring its answers. Nothing is installed
//! or replaced, so the maker's own software keeps working. Elsewhere none is installed yet.
//!
//! With `--virtual-cutter` a simulated Cameo 4 is listed too (C4.8): sending to it goes through
//! everything a real send does, without a machine.

use std::sync::Arc;

use vectorcraft_cutter::DeviceProvider;
use vectorcraft_cutter::simulated::Providers;

/// Installs the machines the app can send to: USB cutters, and a simulated one with `simulated`.
pub fn install(simulated: bool) {
    let mut providers: Vec<Arc<dyn DeviceProvider>> = vec![];
    #[cfg(windows)]
    providers.push(Arc::new(windows_usb::UsbPrint));
    if simulated {
        providers.push(Arc::new(vectorcraft_cutter_silhouette::simulated_cameo4()));
    }
    let provider: Option<Arc<dyn DeviceProvider>> = (!providers.is_empty()).then(|| Arc::new(Providers(providers)) as Arc<dyn DeviceProvider>);
    vectorcraft_cutter::registry::install(provider);
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_usb {
    use vectorcraft_cutter::{DeviceInfo, DeviceProvider, Transport, TransportError};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList,
        SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
    };
    use windows::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile,
    };
    use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
    use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
    use windows::core::{GUID, PCWSTR};

    /// The USB printer device interface (GUID_DEVINTERFACE_USBPRINT).
    const USBPRINT: GUID = GUID::from_u128(0x28d78fad_5a12_11d1_ae5b_0000f803a8c2);
    /// How long one write may take, ms.
    const WRITE_TIMEOUT_MS: u32 = 10_000;
    /// The longest device path read.
    const MAX_PATH_BYTES: u32 = 4_096;

    /// Finds and opens USB printer-class devices.
    pub struct UsbPrint;

    /// `vid_XXXX` and `pid_XXXX` in a device path.
    fn ids(path: &str) -> Option<(u16, u16)> {
        let lower = path.to_ascii_lowercase();
        let hex = |key: &str| lower.find(key).and_then(|i| lower.get(i + key.len()..i + key.len() + 4)).and_then(|h| u16::from_str_radix(h, 16).ok());
        Some((hex("vid_")?, hex("pid_")?))
    }

    fn device_paths() -> Vec<String> {
        let mut out = vec![];
        // SAFETY: the calls follow the SetupDi contract: the set is destroyed once, each detail
        // buffer is allocated at the size the first call reports (8-byte aligned) with cbSize set
        // to the fixed part's size, and only the path written into it is read back.
        unsafe {
            let Ok(set) = SetupDiGetClassDevsW(Some(&USBPRINT), PCWSTR::null(), None, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE) else { return out };
            for index in 0..256u32 {
                let mut data = SP_DEVICE_INTERFACE_DATA { cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32, ..Default::default() };
                if SetupDiEnumDeviceInterfaces(set, None, &USBPRINT, index, &mut data).is_err() {
                    break;
                }
                let mut needed = 0u32;
                let _ = SetupDiGetDeviceInterfaceDetailW(set, &data, None, 0, Some(&mut needed), None);
                if needed == 0 || needed > MAX_PATH_BYTES {
                    continue;
                }
                let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
                let detail = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
                (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
                if SetupDiGetDeviceInterfaceDetailW(set, &data, Some(detail), needed, None, None).is_err() {
                    continue;
                }
                // The path is the NUL-ended UTF-16 that starts at DevicePath, within the buffer.
                let start = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
                let offset = start as usize - buf.as_ptr() as usize;
                let max = (needed as usize).saturating_sub(offset) / 2;
                let wide = std::slice::from_raw_parts(start, max);
                let len = wide.iter().position(|c| *c == 0).unwrap_or(wide.len());
                out.push(String::from_utf16_lossy(wide.get(..len).unwrap_or_default()));
            }
            let _ = SetupDiDestroyDeviceInfoList(set);
        }
        out
    }

    impl DeviceProvider for UsbPrint {
        fn devices(&self) -> Vec<DeviceInfo> {
            device_paths()
                .into_iter()
                .filter_map(|path| {
                    let (vendor_id, product_id) = ids(&path)?;
                    Some(DeviceInfo { name: format!("USB device {vendor_id:04x}:{product_id:04x}"), id: path, vendor_id, product_id })
                })
                .collect()
        }

        fn open(&self, id: &str) -> Result<Box<dyn Transport>, TransportError> {
            let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: `wide` is a NUL-ended path that outlives the call; the handle returned is
            // owned by the transport, which closes it once.
            let handle = unsafe {
                CreateFileW(
                    PCWSTR(wide.as_ptr()),
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    None,
                )
            }
            .map_err(|e| TransportError::Busy(e.message()))?;
            // SAFETY: a manual-reset event, unnamed, owned by the transport.
            let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }.map_err(|e| TransportError::Io(e.message()));
            match event {
                Ok(event) => Ok(Box::new(UsbFile { handle, event })),
                Err(e) => {
                    // SAFETY: the handle was just opened and is closed once.
                    let _ = unsafe { CloseHandle(handle) };
                    Err(e)
                }
            }
        }
    }

    /// An open USB printer-class device.
    struct UsbFile {
        handle: HANDLE,
        event: HANDLE,
    }

    // SAFETY: the handles are owned by this value alone and used from one thread at a time (the
    // sender's); Windows handles may be used from any thread.
    unsafe impl Send for UsbFile {}

    impl UsbFile {
        /// One overlapped read or write of `buf`, waiting at most `timeout_ms`: the bytes moved.
        /// On a timeout the operation is cancelled and waited for before `buf` is let go.
        fn io(&mut self, buf: &mut [u8], write: bool, timeout_ms: u32) -> Result<usize, TransportError> {
            let mut ov = OVERLAPPED { hEvent: self.event, ..Default::default() };
            // SAFETY: `buf` and `ov` live until the operation has completed or been cancelled
            // and waited for (both paths below end in GetOverlappedResult with wait).
            unsafe {
                let started = if write {
                    WriteFile(self.handle, Some(buf), None, Some(&mut ov))
                } else {
                    ReadFile(self.handle, Some(buf), None, Some(&mut ov))
                };
                if let Err(e) = started
                    && e.code() != ERROR_IO_PENDING.to_hresult()
                {
                    return Err(TransportError::Io(e.message()));
                }
                let mut moved = 0u32;
                if WaitForSingleObject(self.event, timeout_ms) != WAIT_OBJECT_0 {
                    let _ = CancelIoEx(self.handle, Some(&ov));
                    let _ = GetOverlappedResult(self.handle, &ov, &mut moved, true);
                    return if write { Err(TransportError::Timeout) } else { Ok(0) };
                }
                GetOverlappedResult(self.handle, &ov, &mut moved, false).map_err(|e| TransportError::Io(e.message()))?;
                Ok(moved as usize)
            }
        }
    }

    impl Transport for UsbFile {
        fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
            let mut left = bytes.to_vec();
            while !left.is_empty() {
                let n = self.io(&mut left, true, WRITE_TIMEOUT_MS)?;
                if n == 0 {
                    return Err(TransportError::Timeout);
                }
                left.drain(..n.min(left.len()));
            }
            Ok(())
        }

        fn read(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, TransportError> {
            self.io(buf, false, timeout_ms)
        }
    }

    impl Drop for UsbFile {
        fn drop(&mut self) {
            // SAFETY: both handles are owned by this value and closed once, here.
            unsafe {
                let _ = CloseHandle(self.handle);
                let _ = CloseHandle(self.event);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ids_come_from_the_device_path() {
            let path = r"\\?\usb#vid_0b4d&pid_1137#845933a#{28d78fad-5a12-11d1-ae5b-0000f803a8c2}";
            assert_eq!(ids(path), Some((0x0B4D, 0x1137)));
            assert_eq!(ids(r"\\?\usb#vid_0B4D&PID_1137#x"), Some((0x0B4D, 0x1137)));
            assert_eq!(ids("no ids"), None);
        }

        /// Listing finds nothing or something, never fails (no cutter is attached here).
        #[test]
        fn listing_doesnt_fail() {
            let _ = UsbPrint.devices();
        }
    }
}
