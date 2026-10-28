use windows::UI::Notifications::Management::UserNotificationListener;

pub fn request_access() -> bool {
    unsafe {
        if let Ok(listener) = UserNotificationListener::Current() {
            if let Ok(op) = listener.RequestAccessAsync() {
                if let Ok(access) = op.get() {
                    // UserNotificationListenerAccessStatus::Allowed = 1
                    return access.0 == 1;
                }
            }
        }
    }
    false
}

pub fn notification_count() -> u32 {
    unsafe {
        if let Ok(listener) = UserNotificationListener::Current() {
            if let Ok(op) = listener.GetNotificationsAsync(windows::UI::Notifications::NotificationKinds(1)) {
                // 0 = NotificationKinds.Toast
                if let Ok(coll) = op.get() {
                    return coll.Size().unwrap_or(0);
                }
            }
        }
    }
    0
}
