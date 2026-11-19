//! Pre-GPU startup tuning.
//!
//! On hybrid-GPU laptops the desktop runs on one GPU while the discrete NVIDIA
//! GPU stays in power saving. Probing NVIDIA's Vulkan ICD wakes that GPU and
//! costs ~2 seconds of window-open latency on every launch. If the kernel says
//! no display is connected to any NVIDIA GPU, tell the Vulkan loader to skip
//! that driver entirely. This is the loader's standard opt-out mechanism
//! (`VK_LOADER_DRIVERS_DISABLE`) and only affects this process.

use std::path::{Path, PathBuf};

/// PCI vendor id of NVIDIA.
const NVIDIA_VENDOR: &str = "0x10de";

/// Apply startup environment tuning. Must be called before GPUI initializes.
pub fn prepare() {
    if std::env::var_os("VK_LOADER_DRIVERS_SELECT").is_some() {
        return; // The user configured driver selection; leave it alone.
    }
    let disabled = std::env::var("VK_LOADER_DRIVERS_DISABLE").unwrap_or_default();
    if disabled.contains("nvidia") || nvidia_display_connected(Path::new("/sys")) {
        return;
    }
    let updated = if disabled.is_empty() {
        "*nvidia*".to_owned()
    } else {
        format!("{disabled};*nvidia*")
    };
    // Safety: called before any threads are spawned (GPUI hasn't started yet),
    // so no other code can observe the environment concurrently.
    unsafe { std::env::set_var("VK_LOADER_DRIVERS_DISABLE", updated) };
}

/// True if any DRM connector reports a connected display and its card is NVIDIA.
/// Unknown or unreadable layout counts as "connected" (conservative: never
/// disable the driver based on a guess).
fn nvidia_display_connected(sysfs_root: &Path) -> bool {
    let Ok(drm) = std::fs::read_dir(sysfs_root.join("class/drm")) else {
        return true;
    };
    // Connector dirs look like `card1-HDMI-A-1`; card dirs like `card1`.
    let cards: Vec<PathBuf> = drm
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    let Some(rest) = name.strip_prefix("card") else {
                        return false;
                    };
                    // A connector name is `cardN-<connector>`; a card is just `cardN`.
                    rest.contains('-')
                })
        })
        .collect();

    cards.iter().any(|connector| {
        let Ok(status) = std::fs::read_to_string(connector.join("status")) else {
            return false;
        };
        if !status.trim().eq("connected") {
            return false;
        }
        // cardN-CON -> cardN -> PCI device -> vendor
        let Some(conn_name) = connector.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let Some(card_name) = conn_name.split('-').next() else {
            return false;
        };
        let vendor_path = sysfs_root.join(format!("class/drm/{card_name}/device/vendor"));
        std::fs::read_to_string(vendor_path)
            .is_ok_and(|vendor| vendor.trim().eq_ignore_ascii_case(NVIDIA_VENDOR))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a fake /sys/class/drm tree: each card gets a vendor and any number
    /// of connectors with statuses.
    fn fake_sysfs(name: &str, cards: &[(&str, u32, &[&str])]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("progrs-test-{name}-{}", std::process::id()));
        let drm = root.join("class/drm");
        std::fs::create_dir_all(&drm).unwrap();
        for (card, vendor, statuses) in cards {
            let device = drm.join(card).join("device");
            std::fs::create_dir_all(&device).unwrap();
            std::fs::write(device.join("vendor"), format!("{vendor:#06x}\n")).unwrap();
            for (i, status) in statuses.iter().enumerate() {
                let connector = drm.join(format!("{card}-HDMI-A-{i}"));
                std::fs::create_dir_all(&connector).unwrap();
                std::fs::write(connector.join("status"), format!("{status}\n")).unwrap();
            }
        }
        root
    }

    fn cleanup(root: &Path) {
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nvidia_with_connected_display_keeps_driver() {
        let root = fake_sysfs(
            "connected",
            &[
                ("card0", 0x8086, &["connected"]),
                ("card1", 0x10de, &["connected", "disconnected"]),
            ],
        );
        assert!(nvidia_display_connected(&root));
        cleanup(&root);
    }

    #[test]
    fn nvidia_without_displays_allows_disabling() {
        let root = fake_sysfs(
            "headless-nvidia",
            &[
                ("card0", 0x8086, &["connected"]),
                ("card1", 0x10de, &["disconnected", "disconnected"]),
            ],
        );
        assert!(!nvidia_display_connected(&root));
        cleanup(&root);
    }

    #[test]
    fn missing_sysfs_is_conservative() {
        assert!(nvidia_display_connected(Path::new("/nonexistent-progrs-path")));
    }

    #[test]
    fn nvidia_only_laptop_keeps_driver() {
        let root = fake_sysfs("nvidia-only", &[("card0", 0x10de, &["connected"])]);
        assert!(nvidia_display_connected(&root));
        cleanup(&root);
    }
}
