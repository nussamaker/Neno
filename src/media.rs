use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

pub struct MediaSession {
    pub title: String,
    pub artist: String,
}

impl MediaSession {
    pub fn current() -> Option<Self> {
        unsafe {
            let mgr = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().ok()?.get().ok()?;
            let session = mgr.GetCurrentSession().ok()?;
            let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
            let title = props.Title().ok()?.to_string();
            let artist = props.Artist().ok()?.to_string();
            Some(Self { title, artist })
        }
    }

    pub fn session() -> Option<GlobalSystemMediaTransportControlsSessionManager> {
        unsafe { GlobalSystemMediaTransportControlsSessionManager::RequestAsync().ok()?.get().ok() }
    }

    pub fn current_session_obj() -> Option<windows::Media::Control::GlobalSystemMediaTransportControlsSession> {
        unsafe {
            let mgr = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().ok()?.get().ok()?;
            mgr.GetCurrentSession().ok()
        }
    }

    pub fn play() {
        unsafe {
            if let Some(s) = Self::current_session_obj() {
                let _ = s.TryPlayAsync();
            }
        }
    }

    pub fn pause() {
        unsafe {
            if let Some(s) = Self::current_session_obj() {
                let _ = s.TryPauseAsync();
            }
        }
    }

    pub fn next() {
        unsafe {
            if let Some(s) = Self::current_session_obj() {
                let _ = s.TrySkipNextAsync();
            }
        }
    }

    pub fn prev() {
        unsafe {
            if let Some(s) = Self::current_session_obj() {
                let _ = s.TrySkipPreviousAsync();
            }
        }
    }
}
