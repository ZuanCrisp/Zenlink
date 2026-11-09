//! Read-only watchers that run beside the screen stream: SpringBoard's interface orientation
//! (the picture follows the device) and the lock notification (auto unlock). They never
//! change anything on the device.

use std::time::Duration;

use idevice::{
    notification_proxy::NotificationProxyClient,
    rsd::RsdHandshake,
    springboardservices::{InterfaceOrientation, SpringBoardServicesClient},
    tcp::handle::AdapterHandle,
    RsdService as _,
};

pub const LOCK_NOTIFICATION: &str = "com.apple.springboard.lockcomplete";

/// Clockwise rotation (degrees) the PC applies to the portrait picture to show it upright.
pub fn rotation(o: &InterfaceOrientation) -> Option<u32> {
    match o {
        InterfaceOrientation::Portrait => Some(0),
        InterfaceOrientation::PortraitUpsideDown => Some(180),
        InterfaceOrientation::LandscapeRight => Some(270),
        InterfaceOrientation::LandscapeLeft => Some(90),
        _ => None,
    }
}

/// Polls the orientation every 0.5 s and calls `on` when it changes. Never returns.
pub async fn orientation(
    mut adapter: AdapterHandle,
    mut rsd: RsdHandshake,
    mut on: impl FnMut(u32),
) {
    let mut last = None;
    loop {
        let mut sb = match SpringBoardServicesClient::connect_rsd(&mut adapter, &mut rsd).await {
            Ok(sb) => sb,
            Err(e) => {
                log::debug!("orientation: {e:?}");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };
        while let Ok(o) = sb.get_interface_orientation().await {
            if let Some(r) = rotation(&o).filter(|r| Some(*r) != last) {
                last = Some(r);
                on(r);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// Calls `on` each time the device locks (screen off, side button, auto-lock). Never returns.
pub async fn lock(mut adapter: AdapterHandle, mut rsd: RsdHandshake, mut on: impl FnMut()) {
    loop {
        match NotificationProxyClient::connect_rsd(&mut adapter, &mut rsd).await {
            Ok(mut np) => {
                if np.observe_notification(LOCK_NOTIFICATION).await.is_ok() {
                    while let Ok(name) = np.receive_notification().await {
                        if name == LOCK_NOTIFICATION {
                            on();
                        }
                    }
                }
            }
            Err(e) => log::debug!("lock watch: {e:?}"),
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
