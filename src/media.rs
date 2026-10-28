use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

pub struct MediaSession {
    pub title: String,
    pub artist: String,
}

pub struct Playback {
    pub position: u64,
    pub duration: u64,
    pub playing: bool,
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

    pub fn playback() -> Option<Playback> {
        unsafe {
            let s = Self::current_session_obj()?;
            let info = s.GetPlaybackInfo().ok()?;
            let status = info.PlaybackStatus().ok()?;
            let playing = status
                == windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
            let timeline = s.GetTimelineProperties().ok()?;
            let position = timeline.Position().ok()?.Duration.max(0) as u64 / 10_000_000;
            let duration = timeline.EndTime().ok()?.Duration.max(0) as u64 / 10_000_000;
            Some(Playback { position, duration, playing })
        }
    }

    pub fn play() {
        Self::toggle()
    }

    pub fn toggle() {
        unsafe {
            if let Some(s) = Self::current_session_obj() {
                let _ = s.TryTogglePlayPauseAsync();
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
