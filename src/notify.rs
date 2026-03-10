/// Desktop notification support for long-running tasks.
///
/// Sends a native OS notification when tasks complete.
/// Requires the `notify` feature flag.

/// Send a desktop notification.
#[cfg(feature = "notify")]
pub fn send_notification(title: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .appname("Synapse")
        .timeout(notify_rust::Timeout::Milliseconds(5000))
        .show();
}

/// No-op when notify feature is disabled.
#[cfg(not(feature = "notify"))]
pub fn send_notification(_title: &str, _body: &str) {}
