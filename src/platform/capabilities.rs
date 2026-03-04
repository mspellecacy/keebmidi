use tracing::warn;

/// Check platform capabilities and return warnings for limited features.
pub fn check_platform_warnings() -> Vec<String> {
    let mut warnings = Vec::new();

    #[cfg(target_os = "linux")]
    {
        // Check if we have a display server for enigo
        if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
            warnings.push(
                "No DISPLAY or WAYLAND_DISPLAY set. Keyboard injection may not work.".to_string(),
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        warnings.push(
            "macOS: Accessibility permissions may be required for keyboard injection.".to_string(),
        );
    }

    for w in &warnings {
        warn!("{w}");
    }

    warnings
}

/// Check platform capabilities for media key support and return warnings.
pub fn check_media_key_support() -> Vec<String> {
    let mut warnings = Vec::new();

    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            warnings.push(
                "Wayland detected: media key injection may be limited. \
                 Consider running under X11/XWayland for full support."
                    .to_string(),
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        warnings.push(
            "macOS: ensure Accessibility permissions are granted for media key injection."
                .to_string(),
        );
    }

    for w in &warnings {
        warn!("{w}");
    }

    warnings
}
