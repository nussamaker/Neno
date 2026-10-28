use std::path::Path;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ,
};
use windows::core::PCWSTR;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "Neno";

pub fn set_autostart(enabled: bool, exe_path: &Path) {
    unsafe {
        let mut key: HKEY = HKEY::default();
        let subkey: Vec<u16> = RUN_KEY.encode_utf16().chain(std::iter::once(0)).collect();
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), 0, KEY_SET_VALUE, &mut key).is_err() {
            return;
        }
        if enabled {
            let data = format!("\"{}\"", exe_path.display());
            let data_w: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = unsafe {
                std::slice::from_raw_parts(data_w.as_ptr() as *const u8, data_w.len() * 2)
            };
            let name: Vec<u16> = VALUE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = RegSetValueExW(key, PCWSTR(name.as_ptr()), 0, REG_SZ, Some(bytes));
        } else {
            let name: Vec<u16> = VALUE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
        }
        let _ = RegCloseKey(key);
    }
    let _ = ERROR_SUCCESS;
}

pub fn is_autostart() -> bool {
    // Simplified: always false in this skeleton; full impl reads the value.
    false
}
