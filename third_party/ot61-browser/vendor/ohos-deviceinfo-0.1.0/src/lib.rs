//! Provides information about the local device on OpenHarmony OS
//!
//! It allows querying basic information about the device, e.g. type and model, as well
//! as version information about the OS.
//! Since this Library is specific to OpenHarmony devices, it is empty on other platforms.
//!
//! Required System Capabilities: SystemCapability.Startup.SystemInfo
//!
//! [deviceinfo]: https://docs.openharmony.cn/pages/v5.0/en/application-dev/reference/apis-basic-services-kit/_device_info.md
#![cfg(target_env = "ohos")]
#![deny(unsafe_op_in_unsafe_fn)]

use ohos_deviceinfo_sys::*;
use std::ffi::{c_char, CStr};

#[derive(Debug, Clone, PartialOrd, PartialEq)]
#[non_exhaustive]
pub enum OhosDeviceType {
    Phone,
    Wearable,
    LiteWearable,
    Tablet,
    Tv,
    Car,
    SmartVision,
    /// Some other device type.
    ///
    /// If you encounter this, consider updating this library or opening an issue.
    Other(&'static str),
    /// Determining the device type was not possible.
    Unknown,
}

/// Convert a raw c-style string with static lifetime to a Rust str
///
/// # Safety
///
/// The Caller must pass a valid, null terminated c string. This string
/// muse be a non-mutable static string, that is valid for the whole remaining
/// lifetime of the program.
unsafe fn convert_to_rust_str(static_c_str: *const c_char) -> Option<&'static str> {
    if static_c_str.is_null() {
        return None;
    }
    // SAFETY: We require a valid, non-mutable c-string with `'static` lifetime, and
    // we checked for `null`.
    let c_str = unsafe { CStr::from_ptr(static_c_str) };
    c_str.to_str().ok().filter(|s| !s.is_empty())
}

/// Obtains the device type (e.g. phone or wearable)
pub fn get_device_type() -> OhosDeviceType {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetDeviceType() };
    // SAFETY: The c-string has a static lifetime.
    let Some(device_type) = (unsafe { convert_to_rust_str(raw) }) else {
        return OhosDeviceType::Unknown;
    };
    // See the documentation of [`OH_GetDeviceType()`].
    match device_type {
        "phone" | "default" => OhosDeviceType::Phone,
        "wearable" => OhosDeviceType::Wearable,
        "liteWearable" => OhosDeviceType::LiteWearable,
        "tablet" => OhosDeviceType::Tablet,
        "tv" => OhosDeviceType::Tv,
        "car" => OhosDeviceType::Car,
        "smartVision" => OhosDeviceType::SmartVision,
        other => OhosDeviceType::Other(other),
    }
}

/// Obtains the device manufacturer
pub fn get_device_manufacturer() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetManufacture() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the device brand
pub fn get_brand() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetBrand() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the product name speaded in the market
pub fn get_market_name() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetMarketName() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}

/// Obtains the product series
pub fn get_product_series() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetProductSeries() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the product model
pub fn get_product_model() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetProductModel() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the software model
pub fn get_software_model() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetSoftwareModel() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the hardware model
pub fn get_hardware_model() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetHardwareModel() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the bootloader version number as a string
pub fn get_bootloader_version() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetBootloaderVersion() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the application binary interface (Abi) list represented as a string.
pub fn get_abi_list() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetAbiList() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}

/// Obtains the security patch tag represented by a string.
pub fn get_security_patch_tag() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetSecurityPatchTag() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the product version displayed for customer represented by a string.
pub fn get_display_version() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetDisplayVersion() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}

/// Obtains the incremental version represented by a string.
pub fn get_incremental_version() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetIncrementalVersion() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the OS release type represented by a string.
///
/// The OS release category can be `Release`, `Beta`, or `Canary`.
/// The specific release type may be `Release`, `Beta1`, or others alike.
pub fn get_os_release_type() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetOsReleaseType() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the OS full version name represented by a string.
pub fn get_os_full_name() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetOSFullName() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}

/// Obtains the SDK API version number.
pub fn get_sdk_api_version() -> u32 {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetSdkApiVersion() };
    // default to 0 for negative numbers.
    raw.try_into().unwrap_or_default()
}

/// Obtains the first API version number.
pub fn get_first_api_version() -> u32 {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetFirstApiVersion() };
    // default to 0 for negative numbers.
    raw.try_into().unwrap_or_default()
}
/// Obtains the version ID by a string.
pub fn get_version_id() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetVersionId() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the build type of the current running OS.
pub fn get_build_type() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetBuildType() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the build user of the current running OS.
pub fn get_build_user() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetBuildUser() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}

/// Obtains the build host of the current running OS.
pub fn get_build_host() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetBuildHost() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}

/// Obtains the build time of the current running OS.
pub fn get_build_time() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetBuildTime() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}
/// Obtains the version hash of the current running OS.
pub fn get_build_hash() -> Option<&'static str> {
    // SAFETY: No side effects - always safe to call.
    let raw = unsafe { OH_GetBuildRootHash() };
    // SAFETY: The c-string has a static lifetime.
    unsafe { convert_to_rust_str(raw) }
}

/// Provides information about this distribution of OpenHarmony OS
///
/// Independent Software Vendors (ISV) may distribute OpenHarmony OS with a custom OS name
/// and customized distribution versions.
pub struct DistributionInfo;

impl DistributionInfo {
    /// Obtains the Distribution OS name represented by a string.
    ///
    /// May be `None` if the ISV did not specify a custom distribution name.
    pub fn name() -> Option<&'static str> {
        // SAFETY: No side effects - always safe to call.
        let raw = unsafe { OH_GetDistributionOSName() };
        // SAFETY: The c-string has a static lifetime.
        unsafe { convert_to_rust_str(raw) }
    }

    /// Obtains the ISV distribution OS version represented by a string.
    ///
    /// If ISV did not specify, returns the same value as [`get_os_full_name()`]
    pub fn version() -> Option<&'static str> {
        // SAFETY: No side effects - always safe to call.
        let raw = unsafe { OH_GetDistributionOSVersion() };
        // SAFETY: The c-string has a static lifetime.
        unsafe { convert_to_rust_str(raw) }
    }

    /// Obtains the ISV distribution OS api version
    ///
    /// If ISV did not specify, returns the same value as [`get_sdk_api_version()`].
    pub fn api_version() -> u32 {
        // SAFETY: No side effects - always safe to call.
        let raw = unsafe { OH_GetDistributionOSApiVersion() };
        // default to 0 for negative numbers.
        raw.try_into().unwrap_or_default()
    }

    /// Obtains the ISV distribution OS release type
    ///
    /// If the ISV did not specify, returns the same value as [`get_os_release_type()`]
    pub fn get_distribution_os_release_type() -> Option<&'static str> {
        // SAFETY: No side effects - always safe to call.
        let raw = unsafe { OH_GetDistributionOSReleaseType() };
        // SAFETY: The c-string has a static lifetime.
        unsafe { convert_to_rust_str(raw) }
    }
}
