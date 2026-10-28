use windows::Devices::Bluetooth::BluetoothDevice;
use windows::Devices::Enumeration::DeviceInformation;

pub fn list_paired_devices() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let selector = match BluetoothDevice::GetDeviceSelector() {
            Ok(s) => s,
            Err(_) => return out,
        };
        let op = match DeviceInformation::FindAllAsyncAqsFilter(&selector) {
            Ok(o) => o,
            Err(_) => return out,
        };
        let collection = match op.get() {
            Ok(c) => c,
            Err(_) => return out,
        };
        let count = collection.Size().unwrap_or(0);
        for i in 0..count {
            if let Ok(info) = collection.GetAt(i) {
                if let Ok(name) = info.Name() {
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}
