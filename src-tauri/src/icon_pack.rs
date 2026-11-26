//! Spell icons fresher than the ones shipped with the app: a pack published on
//! the repo's `icons-pack` branch (version.txt + pack.json), downloaded at
//! startup into the data folder when its version changed, and used from the
//! next launch on. The pack only adds to (or refreshes) data/spellIcons.json:
//! spells it lacks keep their shipped icon.
//!
//! Its icons are served as `assets/pack/spells/<id>.png`, by every window and
//! the OBS overlay alike: `PackAssets` puts them in front of the files
//! embedded in the exe.

use base64::Engine;
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};
use tauri::{App, Assets, Runtime};

use crate::catalog::{CLASSES, SHARED_SPELLS_CLASS};

/// Follows the repo if it's renamed (GitHub redirects), but better updated then.
const PACK_URL: &str = "https://raw.githubusercontent.com/Utruna/wakfu-combo-overlay/icons-pack";
const DIR_NAME: &str = "icon-pack";
const ASSET_PREFIX: &str = "assets/pack/";
const SPELLS_FILE: &str = "spells.json";
const VERSION_FILE: &str = "version.txt";
/// The pack is a few MB; anything far bigger isn't it.
const MAX_PACK_BYTES: usize = 64 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(60);

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_NAME)
}

/// The icon path the catalog gives a pack spell.
pub fn spell_icon_path(icon_id: u64) -> String {
    format!("{ASSET_PREFIX}spells/{icon_id}.png")
}

/// The cached pack's spells: class -> spell name -> icon id, only for the
/// classes the app knows. None when there's no (readable) pack.
pub fn load_spells(pack_dir: &Path) -> Option<Map<String, Value>> {
    let text = std::fs::read_to_string(pack_dir.join(SPELLS_FILE)).ok()?;
    let table: Map<String, Value> = serde_json::from_str(&text).ok()?;
    Some(table.into_iter().filter(|(class, spells)| known_class(class) && spells.is_object()).collect())
}

fn known_class(class: &str) -> bool {
    class == SHARED_SPELLS_CLASS || CLASSES.contains(&class)
}

/// `spells/<digits>.png`, the only kind of file a pack holds.
fn pack_file(pack_dir: &Path, key: &str) -> Option<PathBuf> {
    let name = key.trim_start_matches('/').strip_prefix(ASSET_PREFIX)?.strip_prefix("spells/")?;
    let id = name.strip_suffix(".png")?;
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(pack_dir.join("spells").join(name))
}

/// Puts the cached pack's icons in front of the exe's embedded files, for the
/// windows and (through the asset resolver) the OBS overlay.
pub fn put_in_front<R: Runtime>(context: &mut tauri::Context<R>, pack_dir: PathBuf) {
    let embedded = context.set_assets(Box::new(NoAssets));
    context.set_assets(Box::new(PackAssets { embedded, pack_dir }));
}

struct PackAssets<R: Runtime> {
    embedded: Box<dyn Assets<R>>,
    pack_dir: PathBuf,
}

/// Holds the context's place for the instant its assets are swapped.
struct NoAssets;

impl<R: Runtime> Assets<R> for NoAssets {
    fn get(&self, _key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        None
    }

    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(std::iter::empty())
    }

    fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

impl<R: Runtime> Assets<R> for PackAssets<R> {
    fn setup(&self, app: &App<R>) {
        self.embedded.setup(app);
    }

    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        match pack_file(&self.pack_dir, key.as_ref()) {
            Some(path) => std::fs::read(path).ok().map(Cow::Owned),
            None => self.embedded.get(key),
        }
    }

    fn iter(&self) -> Box<AssetsIter<'_>> {
        self.embedded.iter()
    }

    fn csp_hashes(&self, html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        self.embedded.csp_hashes(html_path)
    }
}

/// Checks for a newer pack in the background; it's used from the next launch.
pub fn spawn_update(pack_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        match update(&pack_dir).await {
            Ok(Some(version)) => println!("[icon_pack] Pack {version} downloaded, used from the next launch"),
            Ok(None) => {}
            Err(error) => eprintln!("[icon_pack] Update check failed: {error}"),
        }
    });
}

async fn update(pack_dir: &Path) -> Result<Option<String>, String> {
    // Same TLS setup as the updater plugin (reqwest without a built-in provider).
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let client = reqwest::Client::builder().timeout(TIMEOUT).build().map_err(|e| e.to_string())?;

    let remote = fetch(&client, VERSION_FILE).await?;
    let remote = String::from_utf8_lossy(&remote).trim().to_string();
    let local = std::fs::read_to_string(pack_dir.join(VERSION_FILE)).unwrap_or_default();
    if remote.is_empty() || remote == local.trim() {
        return Ok(None);
    }

    let pack: Value = serde_json::from_slice(&fetch(&client, "pack.json").await?).map_err(|e| e.to_string())?;
    let version = install(pack_dir, &pack)?;
    Ok(Some(version))
}

async fn fetch(client: &reqwest::Client, file: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(format!("{PACK_URL}/{file}"))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("{file}: {e}"))?;
    let bytes = response.bytes().await.map_err(|e| format!("{file}: {e}"))?;
    if bytes.len() > MAX_PACK_BYTES {
        return Err(format!("{file}: too big ({} bytes)", bytes.len()));
    }
    Ok(bytes.to_vec())
}

/// Writes the pack next to the cached one, then swaps them, so a failure
/// halfway leaves the previous pack whole. Returns the installed version.
///
/// pack.json: `{ "version": "…", "spells": { class: { name: iconId } },
/// "icons": { iconId: "<base64 png>" } }`.
pub fn install(pack_dir: &Path, pack: &Value) -> Result<String, String> {
    let version = pack.get("version").and_then(Value::as_str).filter(|v| !v.trim().is_empty());
    let version = version.ok_or("pack without a version")?.trim().to_string();
    let spells = pack.get("spells").and_then(Value::as_object).ok_or("pack without spells")?;
    let icons = pack.get("icons").and_then(Value::as_object).ok_or("pack without icons")?;

    let staging = pack_dir.with_extension("new");
    let _ = std::fs::remove_dir_all(&staging);
    let written = (|| -> Result<(), String> {
        let icons_dir = staging.join("spells");
        std::fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;

        let mut table = Map::new();
        for (class, class_spells) in spells.iter().filter(|(class, _)| known_class(class)) {
            let class_spells: Map<String, Value> = class_spells
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, id)| id.as_u64().is_some_and(|id| icons.contains_key(&id.to_string())))
                .map(|(name, id)| (name.clone(), id.clone()))
                .collect();
            table.insert(class.clone(), Value::Object(class_spells));
        }

        for (id, data) in icons {
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let Some(png) = data.as_str().and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok()) else {
                continue;
            };
            std::fs::write(icons_dir.join(format!("{id}.png")), png).map_err(|e| e.to_string())?;
        }

        let table = serde_json::to_string(&table).map_err(|e| e.to_string())?;
        std::fs::write(staging.join(SPELLS_FILE), table).map_err(|e| e.to_string())?;
        std::fs::write(staging.join(VERSION_FILE), &version).map_err(|e| e.to_string())
    })();
    if let Err(error) = written {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }

    let _ = std::fs::remove_dir_all(pack_dir);
    std::fs::rename(&staging, pack_dir).map_err(|e| e.to_string())?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serves_only_numbered_spell_icons() {
        let dir = Path::new("pack");
        assert_eq!(pack_file(dir, "/assets/pack/spells/2282.png"), Some(dir.join("spells").join("2282.png")));
        assert_eq!(pack_file(dir, "assets/pack/spells/2282.png"), Some(dir.join("spells").join("2282.png")));
        assert_eq!(pack_file(dir, "/assets/pack/spells/../x.png"), None);
        assert_eq!(pack_file(dir, "/assets/pack/version.txt"), None);
        assert_eq!(pack_file(dir, "/assets/icons/feca/2282.png"), None);
        assert_eq!(pack_file(dir, "/assets/pack/spells/.png"), None);
    }

    #[test]
    fn installs_a_pack_and_keeps_only_known_classes_and_present_icons() {
        let root = std::env::temp_dir().join(format!("taktik-pack-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let pack_dir = dir(&root);

        let png = base64::engine::general_purpose::STANDARD.encode(b"png");
        let pack = json!({
            "version": "1.2.3",
            "spells": {
                "feca": { "Goutte": 2282, "Sans icône": 9999 },
                "inconnue": { "X": 2282 },
                "commun": { "Os à Moelle": 6327 },
            },
            "icons": { "2282": png, "6327": png, "../evil": png },
        });
        assert_eq!(install(&pack_dir, &pack).unwrap(), "1.2.3");

        let spells = load_spells(&pack_dir).unwrap();
        assert_eq!(spells["feca"], json!({ "Goutte": 2282 }));
        assert_eq!(spells["commun"], json!({ "Os à Moelle": 6327 }));
        assert!(!spells.contains_key("inconnue"));
        assert_eq!(std::fs::read(pack_dir.join("spells/2282.png")).unwrap(), b"png");
        assert!(!root.join("evil").exists());
        assert_eq!(std::fs::read_to_string(pack_dir.join(VERSION_FILE)).unwrap(), "1.2.3");

        // A broken pack leaves the installed one alone.
        assert!(install(&pack_dir, &json!({ "version": "2" })).is_err());
        assert_eq!(std::fs::read_to_string(pack_dir.join(VERSION_FILE)).unwrap(), "1.2.3");

        let _ = std::fs::remove_dir_all(&root);
    }
}
