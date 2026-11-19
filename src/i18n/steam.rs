//! Read the per-game Steam language on a Steam launch. Standalone builds can
//! use the installed game's manifest without a Steamworks redistributable.
use std::{
    ffi::{CStr, c_char, c_void},
    fs,
    path::Path,
};

use keyvalues_parser::{Parser, Value};
use libloading::Library;

pub(super) struct Language {
    pub name: String,
    _session: Option<Session>,
}

pub(super) fn current() -> Option<Language> {
    let id = ["SteamAppId", "SteamGameId"].into_iter().find_map(|key| {
        std::env::var(key)
            .ok()?
            .parse::<u32>()
            .ok()
            .filter(|&id| id > 0)
    })?;
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let binary = libloading::library_filename(if cfg!(all(windows, target_pointer_width = "64")) {
        "steam_api64"
    } else {
        "steam_api"
    });
    let mut library = dir.join(&binary);
    if cfg!(target_os = "macos") && !library.is_file() {
        library = dir.parent()?.join("Frameworks").join(&binary);
    }
    if library.is_file()
        && let Some((session, name)) = Session::connect(&library)
    {
        return Some(Language {
            name,
            _session: Some(session),
        });
    }
    let steamapps = dir.ancestors().find(|p| {
        p.file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))
    })?;
    let manifest = fs::read_to_string(steamapps.join(format!("appmanifest_{id}.acf"))).ok()?;
    Some(Language {
        name: manifest_language(&manifest, id)?,
        _session: None,
    })
}

fn manifest_language(text: &str, id: u32) -> Option<String> {
    // App manifests can contain Windows paths with literal backslashes.
    let doc = Parser::new()
        .literal_special_chars(true)
        .parse(text.trim_start_matches('\u{feff}'))
        .ok()?
        .into_vdf();
    if !doc.key.eq_ignore_ascii_case("AppState") {
        return None;
    }
    let state = doc.value.get_obj()?;
    let read = |key: &str| state.get(key)?.first();
    if read("appid")?.get_str()?.parse::<u32>().ok()? != id {
        return None;
    }
    ["UserConfig", "MountedConfig"].into_iter().find_map(|key| {
        let value = read(key)?
            .get_obj()?
            .get("language")?
            .first()
            .and_then(Value::get_str)?;
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// Keep the API loaded until the game exits. Only open a library installed
/// beside this executable (or in its application bundle's Frameworks folder).
struct Session {
    shutdown: unsafe extern "C" fn(),
    _library: Library,
}

impl Session {
    fn connect(path: &Path) -> Option<(Self, String)> {
        // SAFETY: these function signatures and the 1024-byte SteamErrMsg match
        // Valve's flat C API. Resolve every symbol before initialising. The
        // session owns the library, and the returned C string is copied now.
        unsafe {
            let library = Library::new(path).ok()?;
            let init = *library
                .get::<unsafe extern "C" fn(*mut c_char) -> i32>(b"SteamAPI_InitFlat\0")
                .ok()?;
            let apps = *library
                .get::<unsafe extern "C" fn() -> *mut c_void>(b"SteamAPI_SteamApps_v009\0")
                .or_else(|_| library.get(b"SteamAPI_SteamApps_v008\0"))
                .ok()?;
            let language = *library
                .get::<unsafe extern "C" fn(*mut c_void) -> *const c_char>(
                    b"SteamAPI_ISteamApps_GetCurrentGameLanguage\0",
                )
                .ok()?;
            let shutdown = *library.get(b"SteamAPI_Shutdown\0").ok()?;
            let mut message = [0 as c_char; 1024];
            if init(message.as_mut_ptr()) != 0 {
                return None;
            }
            let session = Self {
                shutdown,
                _library: library,
            };
            let apps = apps();
            if apps.is_null() {
                return None;
            }
            let name = language(apps);
            if name.is_null() {
                return None;
            }
            let name = CStr::from_ptr(name).to_str().ok()?.to_owned();
            (!name.is_empty()).then_some((session, name))
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: construction succeeded and the library outlives this call.
        unsafe { (self.shutdown)() }
    }
}

pub(super) fn catalog_code(name: &str) -> Option<&'static str> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "arabic" => "ar",
        "bulgarian" => "bg",
        "czech" => "cs",
        "danish" => "da",
        "german" => "de",
        "greek" => "el",
        "english" => "en",
        "spanish" => "es",
        "latam" => "es-419",
        "finnish" => "fi",
        "french" => "fr",
        "hungarian" => "hu",
        "indonesian" => "id",
        "italian" => "it",
        "japanese" => "ja",
        "koreana" | "korean" => "ko",
        "malay" => "ms",
        "dutch" => "nl",
        "norwegian" => "no",
        "polish" => "pl",
        "brazilian" => "pt-BR",
        "portuguese" => "pt-PT",
        "romanian" => "ro",
        "russian" => "ru",
        "swedish" => "sv",
        "thai" => "th",
        "turkish" => "tr",
        "ukrainian" => "uk",
        "vietnamese" => "vi",
        "schinese" | "sc_schinese" => "zh-Hans",
        "tchinese" => "zh-Hant",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_reads_this_games_user_language_before_its_installed_depot() {
        let acf = r#""AppState" {
            "appid" "819" "name" "language" "installdir" "C:\Steam\Games"
            "MountedConfig" { "language" "english" }
            "UserConfig" { "language" "latam" }
        }"#;
        assert_eq!(manifest_language(acf, 819).as_deref(), Some("latam"));
        assert_eq!(manifest_language(acf, 820), None);
        assert_eq!(
            manifest_language(&acf.replace("UserConfig", "Unused"), 819).as_deref(),
            Some("english")
        );
        assert_eq!(manifest_language("invalid", 819), None);
    }
}
