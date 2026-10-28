//! The machines this process can reach: one [`DeviceProvider`], installed by the app (the desktop
//! app's USB, the CLI's, none in tests unless a test installs one), and the [`Driver`]s for the
//! families of machines this version speaks to. Per process, as plug-ins are.

use std::sync::{Arc, Mutex, OnceLock};

use crate::device::{DeviceInfo, DeviceProvider, Driver};

fn provider_slot() -> &'static Mutex<Option<Arc<dyn DeviceProvider>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<dyn DeviceProvider>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn drivers_slot() -> &'static Mutex<Vec<Arc<dyn Driver>>> {
    static SLOT: OnceLock<Mutex<Vec<Arc<dyn Driver>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(Vec::new()))
}

/// Installs the provider machines are found and opened through (`None`: none).
pub fn install(provider: Option<Arc<dyn DeviceProvider>>) {
    if let Ok(mut p) = provider_slot().lock() {
        *p = provider;
    }
}

/// The installed provider.
pub fn provider() -> Option<Arc<dyn DeviceProvider>> {
    provider_slot().lock().ok().and_then(|p| p.clone())
}

/// Adds `driver`; a driver of the same name is replaced.
pub fn add_driver(driver: Arc<dyn Driver>) {
    if let Ok(mut d) = drivers_slot().lock() {
        d.retain(|x| x.name() != driver.name());
        d.push(driver);
    }
}

/// The driver for `device`, if one drives it.
pub fn driver_for(device: &DeviceInfo) -> Option<Arc<dyn Driver>> {
    drivers_slot().lock().ok().and_then(|d| d.iter().find(|x| x.drives(device)).cloned())
}

/// The connected machines a driver drives, each with its driver.
pub fn machines() -> Vec<(DeviceInfo, Arc<dyn Driver>)> {
    let Some(p) = provider() else { return vec![] };
    p.devices().into_iter().filter_map(|d| driver_for(&d).map(|x| (d, x))).collect()
}

/// The send this process last started, with the machine it went to: one at a time.
fn current_slot() -> &'static Mutex<Option<(DeviceInfo, crate::send::Running)>> {
    static SLOT: OnceLock<Mutex<Option<(DeviceInfo, crate::send::Running)>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// The send last started and its machine, running or finished.
pub fn current() -> Option<(DeviceInfo, crate::send::Running)> {
    current_slot().lock().ok().and_then(|c| c.clone())
}

/// Keeps `running` as the current send; refused while another is still running.
pub fn start(device: DeviceInfo, running: crate::send::Running) -> Result<(), String> {
    let mut c = current_slot().lock().map_err(|_| "the cutter's state is damaged".to_string())?;
    if let Some((d, r)) = c.as_ref()
        && !r.finished()
    {
        return Err(format!("a job is still being sent to {}", d.name));
    }
    *c = Some((device, running));
    Ok(())
}
