//! The app's integration tests, headless on NSH's real engine: one binary, a module per area.

mod shell;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

use nsh_app_easytierui::App;
use nsh_engine::{
    EngineConfig, Idiom, Locale, Os, Points, Root, Size, StorageBackend, StorageChange,
};
use nsh_testing::Headless;

/// Where and how a test runs the app.
#[derive(Clone)]
pub struct Device {
    pub os: Os,
    pub idiom: Idiom,
    pub width: f32,
    pub height: f32,
    pub locale: &'static str,
    /// The app's storage, shared by the runs of one test (a restart keeps it).
    pub storage: Arc<MemoryStorage>,
}

impl Device {
    /// A desktop window of `width` points on `os`.
    pub fn desktop(os: Os, width: f32) -> Self {
        Self {
            os,
            idiom: Idiom::Desktop,
            width,
            height: 800.0,
            locale: "en-US",
            storage: Arc::default(),
        }
    }

    /// A phone on `os`.
    pub fn phone(os: Os) -> Self {
        Self {
            os,
            idiom: Idiom::Phone,
            width: 390.0,
            height: 844.0,
            locale: "en-US",
            storage: Arc::default(),
        }
    }

    pub fn locale(self, locale: &'static str) -> Self {
        Self { locale, ..self }
    }

    /// Starts the app on this device, with `theme.json`, once it has rendered.
    pub fn start(&self) -> Result<Headless, String> {
        let theme = nsh_base::Theme::from_json(include_str!("../../theme.json"))
            .map_err(|error| format!("theme.json: {error:?}"))?;
        let mut config = EngineConfig::new(Root::of::<App>().tokens(theme.into_tokens()));
        let size = Size {
            width: Points(self.width),
            height: Points(self.height),
        };
        config.viewport = size;
        config.environment.window = size;
        config.environment.os = self.os;
        config.environment.idiom = self.idiom;
        config.environment.locale =
            Locale::parse(self.locale).ok_or_else(|| format!("locale {:?}", self.locale))?;
        config.storage = Some(self.storage.clone());
        let mut app = Headless::start(config).map_err(|error| error.to_string())?;
        app.first_render()?;
        Ok(app)
    }
}

/// Storage kept in memory across the runs of one test.
#[derive(Default)]
pub struct MemoryStorage(Mutex<BTreeMap<String, String>>);

impl StorageBackend for MemoryStorage {
    fn load(&self) -> Result<BTreeMap<String, String>, String> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn write(&self, change: StorageChange) {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match change {
            StorageChange::Set(key, value) => {
                entries.insert(key, value);
            }
            StorageChange::Remove(key) => {
                entries.remove(&key);
            }
            StorageChange::Clear => entries.clear(),
        }
    }
}
