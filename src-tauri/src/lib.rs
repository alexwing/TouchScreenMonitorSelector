use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};
use winreg::enums::*;
use winreg::RegKey;

const TRAY_ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const TRAY_ICON_ACTIVE: &[u8] = include_bytes!("../icons/tray-active.png");
const TRAY_ID: &str = "main-tray";
const DIGIMON_REG_PATH: &str = r"SOFTWARE\Microsoft\Wisp\Pen\Digimon";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MonitorInfo {
    pub id: usize,
    pub name: String,
    pub gdi_name: String,
    pub friendly_name: String,
    pub interface_path: String,
    pub hardware_id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub hmonitor: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TouchDeviceInfo {
    pub id: usize,
    pub product_name: String,
    pub device_path: String,
    pub reg_key_name: String,
    pub device_type: String,
    pub max_contacts: u16,
    pub vendor_id: u32,
    pub product_id: u32,
    pub is_virtual: bool,
    pub mapped_monitor_path: Option<String>,
    pub mapped_monitor_id: Option<usize>,
    pub active_monitor_id: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemTouchState {
    pub monitors: Vec<MonitorInfo>,
    pub touch_devices: Vec<TouchDeviceInfo>,
    pub is_admin: bool,
}

#[cfg(target_os = "windows")]
mod win32 {
    use super::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MONITORINFOEXW {
        pub cb_size: u32,
        pub rc_monitor: RECT,
        pub rc_work: RECT,
        pub dw_flags: u32,
        pub sz_device: [u16; 32],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct DISPLAY_DEVICEW {
        pub cb: u32,
        pub device_name: [u16; 32],
        pub device_string: [u16; 128],
        pub state_flags: u32,
        pub device_id: [u16; 128],
        pub device_key: [u16; 128],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct POINTER_DEVICE_INFO {
        pub display_orientation: u32,
        pub device: isize,
        pub pointer_device_type: i32,
        pub monitor: isize,
        pub starting_cursor_id: u32,
        pub max_active_contacts: u16,
        pub product_string: [u16; 520],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RAWINPUTDEVICELIST {
        pub h_device: isize,
        pub dw_type: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RID_DEVICE_INFO_HID {
        pub dw_vendor_id: u32,
        pub dw_product_id: u32,
        pub dw_version_number: u32,
        pub us_usage_page: u16,
        pub us_usage: u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RID_DEVICE_INFO {
        pub cb_size: u32,
        pub dw_type: u32,
        pub hid: RID_DEVICE_INFO_HID,
        pub _padding: [u32; 4],
    }

    type MonitorEnumProc = unsafe extern "system" fn(isize, isize, *mut RECT, isize) -> i32;

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayMonitors(
            hdc: isize,
            lprc_clip: *const RECT,
            lpfn_enum: MonitorEnumProc,
            dw_data: isize,
        ) -> i32;
        fn GetMonitorInfoW(hmonitor: isize, lpmi: *mut MONITORINFOEXW) -> i32;
        fn EnumDisplayDevicesW(
            lp_device: *const u16,
            i_dev_num: u32,
            lp_display_device: *mut DISPLAY_DEVICEW,
            dw_flags: u32,
        ) -> i32;
        fn GetPointerDevices(
            device_count: *mut u32,
            pointer_devices: *mut POINTER_DEVICE_INFO,
        ) -> i32;
        fn GetRawInputDeviceList(
            p_raw_input_device_list: *mut RAWINPUTDEVICELIST,
            pui_num_devices: *mut u32,
            cb_size: u32,
        ) -> u32;
        fn GetRawInputDeviceInfoW(
            h_device: isize,
            ui_command: u32,
            p_data: *mut std::ffi::c_void,
            pcb_size: *mut u32,
        ) -> u32;
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: isize,
            fu_flags: u32,
            u_timeout: u32,
            lpdw_result: *mut usize,
        ) -> isize;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(lp_module_name: *const u16) -> isize;
        fn GetProcAddress(h_module: isize, lp_proc_name: *const u8) -> isize;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn IsUserAnAdmin() -> i32;
    }

    pub fn is_admin() -> bool {
        unsafe { IsUserAnAdmin() != 0 }
    }

    /// Calls undocumented USER32.dll Ordinal #2532 (SetPointerDeviceMonitor / UpdatePointerDeviceMonitor)
    /// which is the exact API invoked by Windows' native MultiDigiMon.exe at runtime to immediately
    /// rebind the active kernel/Win32k touch pointer device to the target HMONITOR without rebooting.
    pub fn apply_live_pointer_device_monitor(device_path: &str, target_hmonitor: isize) -> bool {
        if target_hmonitor == 0 {
            return false;
        }
        unsafe {
            let user32_name = str_to_u16_nul("user32.dll");
            let h_user32 = GetModuleHandleW(user32_name.as_ptr());
            if h_user32 == 0 {
                return false;
            }
            // Ordinal 2532 (0x09E4) passed via MAKEINTRESOURCEA(2532)
            let proc_addr = GetProcAddress(h_user32, 2532usize as *const u8);
            if proc_addr == 0 {
                return false;
            }
            type SetPointerDeviceMonitorFn = unsafe extern "system" fn(isize, isize) -> i32;
            let set_pointer_device_monitor: SetPointerDeviceMonitorFn =
                std::mem::transmute(proc_addr);

            let normalized_target = device_path.trim_start_matches("20-").to_lowercase();
            let mut count: u32 = 0;
            GetPointerDevices(&mut count, std::ptr::null_mut());
            if count == 0 {
                return false;
            }
            let mut ptr_devs: Vec<POINTER_DEVICE_INFO> =
                vec![std::mem::zeroed(); count as usize];
            if GetPointerDevices(&mut count, ptr_devs.as_mut_ptr()) == 0 {
                return false;
            }

            let mut applied = false;
            for pd in ptr_devs.into_iter().take(count as usize) {
                let raw_path = get_raw_device_name(pd.device).unwrap_or_default();
                let is_virtual = raw_path.to_uppercase().contains("VIRTUAL_DIGITIZER");
                if is_virtual {
                    continue;
                }
                if raw_path.to_lowercase() == normalized_target
                    || normalized_target.is_empty()
                    || pd.pointer_device_type == 3
                {
                    let ok = set_pointer_device_monitor(pd.device, target_hmonitor);
                    if ok != 0 {
                        applied = true;
                    }
                }
            }
            applied
        }
    }

    pub fn u16_to_string(buf: &[u16]) -> String {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        OsString::from_wide(&buf[..len])
            .to_string_lossy()
            .into_owned()
    }

    pub fn str_to_u16_nul(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Reads the monitor name from EDID in HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\<HWID>\<INSTANCE>\Device Parameters
    fn read_edid_friendly_name(interface_path: &str) -> Option<String> {
        // interface_path format: \\?\DISPLAY#GSM76FA#5&895aa87&0&UID4353#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}
        let parts: Vec<&str> = interface_path.split('#').collect();
        if parts.len() < 3 {
            return None;
        }
        let hwid = parts[1];
        let instance = parts[2];
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        // Try Device Parameters -> EDID first
        let dev_param_path = format!(
            r"SYSTEM\CurrentControlSet\Enum\DISPLAY\{}\{}\Device Parameters",
            hwid, instance
        );
        if let Ok(key) = hklm.open_subkey(&dev_param_path) {
            if let Ok(raw_val) = key.get_raw_value("EDID") {
                let edid = &raw_val.bytes;
                for offset in (54..=108).step_by(18) {
                    if edid.len() >= offset + 18
                        && edid[offset] == 0
                        && edid[offset + 1] == 0
                        && edid[offset + 2] == 0
                        && edid[offset + 3] == 0xFC
                    {
                        let name_bytes: Vec<u8> = edid[offset + 5..offset + 18]
                            .iter()
                            .copied()
                            .filter(|&b| (0x20..=0x7E).contains(&b))
                            .collect();
                        let name = String::from_utf8_lossy(&name_bytes).trim().to_string();
                        if !name.is_empty() {
                            return Some(name);
                        }
                    }
                }
            }
        }

        // Fallback to FriendlyName in parent key
        let enum_path = format!(r"SYSTEM\CurrentControlSet\Enum\DISPLAY\{}\{}", hwid, instance);
        if let Ok(key) = hklm.open_subkey(&enum_path) {
            if let Ok(friendly) = key.get_value::<String, _>("FriendlyName") {
                // Often formatted like "@System32\drivers\dxgkrnl.sys,#303;Generic Monitor (%1);(LG ULTRAWIDE)"
                if let Some(last_semi) = friendly.rfind(';') {
                    let tail = friendly[last_semi + 1..].trim_matches(|c| c == '(' || c == ')');
                    if !tail.is_empty() {
                        return Some(tail.to_string());
                    }
                }
                if !friendly.is_empty() {
                    return Some(friendly);
                }
            }
        }

        if !hwid.is_empty() {
            return Some(hwid.to_string());
        }
        None
    }

    struct RawMonitor {
        hmonitor: isize,
        gdi_name: String,
        rect: RECT,
        is_primary: bool,
    }

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: isize,
        _hdc: isize,
        _lprc: *mut RECT,
        dw_data: isize,
    ) -> i32 {
        let list = &mut *(dw_data as *mut Vec<RawMonitor>);
        let mut mi: MONITORINFOEXW = std::mem::zeroed();
        mi.cb_size = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmonitor, &mut mi) != 0 {
            let gdi_name = u16_to_string(&mi.sz_device);
            let is_primary = (mi.dw_flags & 1) != 0;
            list.push(RawMonitor {
                hmonitor,
                gdi_name,
                rect: mi.rc_monitor,
                is_primary,
            });
        }
        1
    }

    pub fn enumerate_monitors() -> Vec<MonitorInfo> {
        let mut raw_list: Vec<RawMonitor> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                0,
                std::ptr::null(),
                monitor_enum_proc,
                &mut raw_list as *mut Vec<RawMonitor> as isize,
            );
        }

        // Sort by GDI name (e.g. \\.\DISPLAY1, \\.\DISPLAY2) so monitor numbering matches Windows Settings
        raw_list.sort_by(|a, b| a.gdi_name.cmp(&b.gdi_name));

        let mut monitors = Vec::new();
        for (idx, rm) in raw_list.into_iter().enumerate() {
            let gdi_wide = str_to_u16_nul(&rm.gdi_name);

            let mut dd_iface: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
            dd_iface.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
            let has_iface =
                unsafe { EnumDisplayDevicesW(gdi_wide.as_ptr(), 0, &mut dd_iface, 1) != 0 };

            let mut dd_hw: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
            dd_hw.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
            let has_hw = unsafe { EnumDisplayDevicesW(gdi_wide.as_ptr(), 0, &mut dd_hw, 0) != 0 };

            let interface_path = if has_iface {
                u16_to_string(&dd_iface.device_id)
            } else {
                String::new()
            };

            let hardware_id = if has_hw {
                u16_to_string(&dd_hw.device_id)
            } else {
                String::new()
            };

            let device_string = if has_iface {
                u16_to_string(&dd_iface.device_string)
            } else {
                format!("Display {}", idx + 1)
            };

            let friendly_name = read_edid_friendly_name(&interface_path).unwrap_or_else(|| {
                if !device_string.is_empty() && device_string != "Generic PnP Monitor" {
                    device_string.clone()
                } else {
                    format!("Monitor {}", idx + 1)
                }
            });

            let short_gdi = rm.gdi_name.trim_start_matches(r"\\.\").to_string();
            let display_label = format!("{} ({})", friendly_name, short_gdi);

            monitors.push(MonitorInfo {
                id: idx,
                name: display_label,
                gdi_name: rm.gdi_name,
                friendly_name,
                interface_path,
                hardware_id,
                x: rm.rect.left,
                y: rm.rect.top,
                width: (rm.rect.right - rm.rect.left).max(0) as u32,
                height: (rm.rect.bottom - rm.rect.top).max(0) as u32,
                is_primary: rm.is_primary,
                hmonitor: rm.hmonitor as usize,
            });
        }

        monitors
    }

    fn get_raw_device_name(h_device: isize) -> Option<String> {
        let mut name_len: u32 = 0;
        const RIDI_DEVICENAME: u32 = 0x20000007;
        unsafe {
            GetRawInputDeviceInfoW(h_device, RIDI_DEVICENAME, std::ptr::null_mut(), &mut name_len);
            if name_len == 0 {
                return None;
            }
            let mut buf = vec![0u16; (name_len + 1) as usize];
            let res = GetRawInputDeviceInfoW(
                h_device,
                RIDI_DEVICENAME,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                &mut name_len,
            );
            if res == u32::MAX {
                return None;
            }
            Some(u16_to_string(&buf))
        }
    }

    fn get_raw_hid_info(h_device: isize) -> Option<RID_DEVICE_INFO_HID> {
        const RIDI_DEVICEINFO: u32 = 0x2000000b;
        let mut info: RID_DEVICE_INFO = unsafe { std::mem::zeroed() };
        info.cb_size = std::mem::size_of::<RID_DEVICE_INFO>() as u32;
        let mut pcb_size = info.cb_size;
        unsafe {
            let res = GetRawInputDeviceInfoW(
                h_device,
                RIDI_DEVICEINFO,
                &mut info as *mut _ as *mut std::ffi::c_void,
                &mut pcb_size,
            );
            if res == u32::MAX || info.dw_type != 2 {
                return None;
            }
            Some(info.hid)
        }
    }

    pub fn read_digimon_mappings() -> HashMap<String, String> {
        let mut map = HashMap::new();
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(key) = hklm.open_subkey(DIGIMON_REG_PATH) {
            for item in key.enum_values().flatten() {
                let (val_name, reg_val) = item;
                if let Ok(target) = String::from_utf8(
                    reg_val
                        .bytes
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .take_while(|&c| c != 0)
                        .flat_map(|c| char::from_u32(c as u32))
                        .collect::<String>()
                        .into_bytes(),
                ) {
                    map.insert(val_name.to_lowercase(), target);
                }
            }
        }
        map
    }

    pub fn enumerate_touch_devices(monitors: &[MonitorInfo]) -> Vec<TouchDeviceInfo> {
        let digimon_map = read_digimon_mappings();
        let mut devices = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        // 1. Enumerate active Pointer Devices via GetPointerDevices
        let mut count: u32 = 0;
        unsafe {
            GetPointerDevices(&mut count, std::ptr::null_mut());
            if count > 0 {
                let mut ptr_devs: Vec<POINTER_DEVICE_INFO> = vec![std::mem::zeroed(); count as usize];
                if GetPointerDevices(&mut count, ptr_devs.as_mut_ptr()) != 0 {
                    for pd in ptr_devs.into_iter().take(count as usize) {
                        let raw_path = get_raw_device_name(pd.device).unwrap_or_default();
                        if raw_path.is_empty() {
                            continue;
                        }
                        let is_virtual = raw_path.to_uppercase().contains("VIRTUAL_DIGITIZER");
                        let (vid, pid) = get_raw_hid_info(pd.device)
                            .map(|h| (h.dw_vendor_id, h.dw_product_id))
                            .unwrap_or((0, 0));

                        let prod = u16_to_string(&pd.product_string).trim().to_string();
                        let product_name = if prod.is_empty() || prod.contains("VIRTUAL_DIGITIZER") {
                            if is_virtual {
                                "Virtual Digitizer (Windows)".to_string()
                            } else if vid != 0 || pid != 0 {
                                format!("HID Touch Digitizer ({:04X}:{:04X})", vid, pid)
                            } else {
                                "HID Touch Digitizer".to_string()
                            }
                        } else {
                            prod
                        };

                        let device_type = match pd.pointer_device_type {
                            1 => "Integrated Pen".to_string(),
                            2 => "External Pen".to_string(),
                            3 => "Touch".to_string(),
                            4 => "Touchpad".to_string(),
                            _ => "Digitizer".to_string(),
                        };

                        let reg_key_name = format!("20-{}", raw_path);
                        let mapped_monitor_path = digimon_map
                            .get(&reg_key_name.to_lowercase())
                            .or_else(|| digimon_map.get(&raw_path.to_lowercase()))
                            .cloned();

                        let mapped_monitor_id = mapped_monitor_path.as_ref().and_then(|mp| {
                            monitors
                                .iter()
                                .find(|m| m.interface_path.eq_ignore_ascii_case(mp))
                                .map(|m| m.id)
                        });

                        let active_monitor_id = monitors
                            .iter()
                            .find(|m| m.hmonitor == pd.monitor as usize)
                            .map(|m| m.id)
                            .or(mapped_monitor_id);

                        seen_paths.insert(raw_path.to_lowercase());
                        devices.push(TouchDeviceInfo {
                            id: devices.len(),
                            product_name,
                            device_path: raw_path,
                            reg_key_name,
                            device_type,
                            max_contacts: pd.max_active_contacts,
                            vendor_id: vid,
                            product_id: pid,
                            is_virtual,
                            mapped_monitor_path,
                            mapped_monitor_id,
                            active_monitor_id,
                        });
                    }
                }
            }
        }

        // 2. Also check RawInput HID devices with UsagePage == 0x0D (Digitizer) in case any wasn't in GetPointerDevices
        unsafe {
            let mut raw_count: u32 = 0;
            let elem_size = std::mem::size_of::<RAWINPUTDEVICELIST>() as u32;
            GetRawInputDeviceList(std::ptr::null_mut(), &mut raw_count, elem_size);
            if raw_count > 0 {
                let mut raw_list = vec![
                    RAWINPUTDEVICELIST {
                        h_device: 0,
                        dw_type: 0
                    };
                    raw_count as usize
                ];
                let fetched = GetRawInputDeviceList(raw_list.as_mut_ptr(), &mut raw_count, elem_size);
                if fetched != u32::MAX {
                    for dev in raw_list.into_iter().take(fetched as usize) {
                        if dev.dw_type == 2 {
                            // RIM_TYPEHID
                            if let Some(hid) = get_raw_hid_info(dev.h_device) {
                                // UsagePage 0x0D = Digitizers, Usage 0x04 = Touch Screen, 0x02 = Pen
                                if hid.us_usage_page == 0x0D && (hid.us_usage == 0x04 || hid.us_usage == 0x02 || hid.us_usage == 0x01) {
                                    if let Some(raw_path) = get_raw_device_name(dev.h_device) {
                                        if !seen_paths.contains(&raw_path.to_lowercase()) {
                                            let is_virtual =
                                                raw_path.to_uppercase().contains("VIRTUAL_DIGITIZER");
                                            let reg_key_name = format!("20-{}", raw_path);
                                            let mapped_monitor_path = digimon_map
                                                .get(&reg_key_name.to_lowercase())
                                                .or_else(|| digimon_map.get(&raw_path.to_lowercase()))
                                                .cloned();
                                            let mapped_monitor_id =
                                                mapped_monitor_path.as_ref().and_then(|mp| {
                                                    monitors
                                                        .iter()
                                                        .find(|m| {
                                                            m.interface_path.eq_ignore_ascii_case(mp)
                                                        })
                                                        .map(|m| m.id)
                                                });

                                            seen_paths.insert(raw_path.to_lowercase());
                                            devices.push(TouchDeviceInfo {
                                                id: devices.len(),
                                                product_name: if is_virtual {
                                                    "Virtual Digitizer (Windows)".to_string()
                                                } else {
                                                    format!(
                                                        "HID Digitizer ({:04X}:{:04X})",
                                                        hid.dw_vendor_id, hid.dw_product_id
                                                    )
                                                },
                                                device_path: raw_path,
                                                reg_key_name,
                                                device_type: if hid.us_usage == 0x02 {
                                                    "Pen".to_string()
                                                } else {
                                                    "Touch".to_string()
                                                },
                                                max_contacts: 1,
                                                vendor_id: hid.dw_vendor_id,
                                                product_id: hid.dw_product_id,
                                                is_virtual,
                                                mapped_monitor_path,
                                                mapped_monitor_id,
                                                active_monitor_id: mapped_monitor_id,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Put physical touchscreens first, virtual digitizers last
        devices.sort_by_key(|d| d.is_virtual);
        for (i, d) in devices.iter_mut().enumerate() {
            d.id = i;
        }

        devices
    }

    pub fn broadcast_digitizer_mapping_changed() {
        const HWND_BROADCAST: isize = 0xffff;
        const WM_SETTINGCHANGE: u32 = 0x001A;
        const SMTO_ABORTIFHUNG: u32 = 0x0002;

        let msg_wide = str_to_u16_nul("TabletPCDigitizerMappingChanged");
        let mut result: usize = 0;
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                msg_wide.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                500,
                &mut result,
            );
        }
    }
}

fn update_tray_icon(app: &AppHandle, active: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let icon_bytes = if active {
            TRAY_ICON_ACTIVE
        } else {
            TRAY_ICON_IDLE
        };
        if let Ok(icon) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

#[tauri::command]
fn get_touch_system_state(app: AppHandle) -> Result<SystemTouchState, String> {
    let monitors = win32::enumerate_monitors();
    let touch_devices = win32::enumerate_touch_devices(&monitors);
    let is_admin = win32::is_admin();

    let has_mapped = touch_devices
        .iter()
        .any(|d| !d.is_virtual && d.active_monitor_id.is_some());
    update_tray_icon(&app, has_mapped);

    Ok(SystemTouchState {
        monitors,
        touch_devices,
        is_admin,
    })
}

/// Option B: Directly applies the digitizer -> monitor mapping in real time via
/// USER32.dll Ordinal #2532 (SetPointerDeviceMonitor, used by MultiDigiMon.exe)
/// and persists the mapping to HKLM\SOFTWARE\Microsoft\Wisp\Pen\Digimon.
#[tauri::command]
fn bind_touch_to_monitor(
    app: AppHandle,
    device_path: String,
    monitor_interface_path: String,
) -> Result<String, String> {
    if device_path.trim().is_empty() {
        return Err("Invalid touch device path".to_string());
    }
    if monitor_interface_path.trim().is_empty() {
        return Err("Invalid monitor interface path".to_string());
    }

    // 1. Find the target monitor's HMONITOR handle so we can immediately rebind the active touch device
    let monitors = win32::enumerate_monitors();
    let target_hmonitor = monitors
        .iter()
        .find(|m| m.interface_path.eq_ignore_ascii_case(&monitor_interface_path))
        .map(|m| m.hmonitor as isize)
        .unwrap_or(0);

    // 2. Apply live pointer-to-monitor binding via USER32.dll Ordinal #2532 (takes effect immediately!)
    let live_applied = win32::apply_live_pointer_device_monitor(&device_path, target_hmonitor);

    let reg_value_name = if device_path.starts_with("20-") {
        device_path.clone()
    } else {
        format!("20-{}", device_path)
    };

    // 3. Persist in HKLM\SOFTWARE\Microsoft\Wisp\Pen\Digimon so it survives reboots/reconnections
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    match hklm.create_subkey(DIGIMON_REG_PATH) {
        Ok((key, _)) => {
            let _ = key.delete_value(&reg_value_name);
            key.set_value(&reg_value_name, &monitor_interface_path)
                .map_err(|e| format!("Error writing to registry: {}", e))?;
            win32::broadcast_digitizer_mapping_changed();
            update_tray_icon(&app, true);
            Ok("OK".to_string())
        }
        Err(_) => {
            // Try elevating via UAC to persist to HKLM; if live binding already succeeded,
            // even if UAC is skipped the current session's touch mapping is already active!
            let escaped_name = reg_value_name.replace('\'', "''");
            let escaped_val = monitor_interface_path.replace('\'', "''");
            let ps_inner = format!(
                "$path = 'HKLM:\\SOFTWARE\\Microsoft\\Wisp\\Pen\\Digimon'; \
                 if (!(Test-Path $path)) {{ New-Item -Path $path -Force | Out-Null }}; \
                 Remove-ItemProperty -Path $path -Name '{}' -ErrorAction SilentlyContinue; \
                 Set-ItemProperty -Path $path -Name '{}' -Value '{}' -Type String; \
                 Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class WispB {{ [DllImport(\"user32.dll\", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessageTimeoutW(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult); }}'; \
                 $res = [UIntPtr]::Zero; \
                 [WispB]::SendMessageTimeoutW([IntPtr]0xffff, 0x001A, [UIntPtr]::Zero, 'TabletPCDigitizerMappingChanged', 2, 500, [ref]$res) | Out-Null",
                escaped_name, escaped_name, escaped_val
            );

            let utf16_bytes: Vec<u8> = ps_inner
                .encode_utf16()
                .flat_map(|u| u.to_le_bytes())
                .collect();
            let encoded = base64_encode(&utf16_bytes);

            let status = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &format!(
                        "Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -ArgumentList '-NoProfile -WindowStyle Hidden -EncodedCommand {}'",
                        encoded
                    ),
                ])
                .status();

            if let Ok(s) = status {
                if !s.success() && !live_applied {
                    return Err("UAC elevation was cancelled or failed.".to_string());
                }
            } else if !live_applied {
                return Err("Failed to apply touch monitor mapping.".to_string());
            }

            win32::broadcast_digitizer_mapping_changed();
            update_tray_icon(&app, true);
            Ok("OK".to_string())
        }
    }
}

/// Option A: Native Windows Calibration / Touch Mapping Wizard
#[tauri::command]
fn open_native_touch_tool(mode: String) -> Result<(), String> {
    match mode.as_str() {
        "multidigimon" => {
            Command::new("MultiDigiMon.exe")
                .arg("-touch")
                .spawn()
                .or_else(|_| {
                    // MultiDigiMon requires elevation; launch via Start-Process -Verb RunAs if direct spawn fails
                    Command::new("powershell")
                        .args([
                            "-NoProfile",
                            "-WindowStyle",
                            "Hidden",
                            "-Command",
                            "Start-Process 'MultiDigiMon.exe' -ArgumentList '-touch' -Verb RunAs",
                        ])
                        .spawn()
                })
                .map_err(|e| format!("Failed to launch MultiDigiMon.exe: {}", e))?;
        }
        _ => {
            Command::new("tabcal.exe")
                .arg("devicekind=touch")
                .spawn()
                .or_else(|_| {
                    Command::new("powershell")
                        .args([
                            "-NoProfile",
                            "-WindowStyle",
                            "Hidden",
                            "-Command",
                            "Start-Process 'tabcal.exe' -ArgumentList 'devicekind=touch' -Verb RunAs",
                        ])
                        .spawn()
                })
                .map_err(|e| format!("Failed to launch tabcal.exe: {}", e))?;
        }
    }
    Ok(())
}

#[tauri::command]
fn update_tray_locale(app: AppHandle, show_text: String, exit_text: String) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(show_i) = MenuItem::with_id(&app, "show", &show_text, true, None::<&str>) {
            if let Ok(quit_i) = MenuItem::with_id(&app, "quit", &exit_text, true, None::<&str>) {
                if let Ok(menu) = Menu::with_items(&app, &[&show_i, &quit_i]) {
                    let _ = tray.set_menu(Some(menu));
                }
            }
        }
    }
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let idle_icon = Image::from_bytes(TRAY_ICON_ACTIVE)?;

            let _tray = TrayIconBuilder::with_id(TRAY_ID)
                .icon(idle_icon)
                .tooltip("TouchScreen Monitor Selector")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_touch_system_state,
            bind_touch_to_monitor,
            open_native_touch_tool,
            update_tray_locale
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
