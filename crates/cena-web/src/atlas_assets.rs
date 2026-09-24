//! Embedded explorer sources. Only these literal assets may be served.
pub(crate) fn get(name: &str) -> Option<(&'static str, &'static str)> {
    let text = match name {
        "LICH-LICENSE.txt" => include_str!("../assets/atlas/LICH-LICENSE.txt"),
        "app.mjs" => include_str!("../assets/atlas/app.mjs"),
        "atlas-view.mjs" => include_str!("../assets/atlas/atlas-view.mjs"),
        "display-layout.mjs" => include_str!("../assets/atlas/display-layout.mjs"),
        "hunting-regional.mjs" => include_str!("../assets/atlas/hunting-regional.mjs"),
        "hunting-sections.mjs" => include_str!("../assets/atlas/hunting-sections.mjs"),
        "hunting-view.mjs" => include_str!("../assets/atlas/hunting-view.mjs"),
        "icons.mjs" => include_str!("../assets/atlas/icons.mjs"),
        "index.html" => include_str!("../assets/atlas/index.html"),
        "interaction.mjs" => include_str!("../assets/atlas/interaction.mjs"),
        "labels.mjs" => include_str!("../assets/atlas/labels.mjs"),
        "model.mjs" => include_str!("../assets/atlas/model.mjs"),
        "navigation.mjs" => include_str!("../assets/atlas/navigation.mjs"),
        "offline-source.mjs" => include_str!("../assets/atlas/offline-source.mjs"),
        "preferences.mjs" => include_str!("../assets/atlas/preferences.mjs"),
        "profile.mjs" => include_str!("../assets/atlas/profile.mjs"),
        "reference-layout.mjs" => include_str!("../assets/atlas/reference-layout.mjs"),
        "region-model.mjs" => include_str!("../assets/atlas/region-model.mjs"),
        "region-overview-view.mjs" => include_str!("../assets/atlas/region-overview-view.mjs"),
        "region-overview.mjs" => include_str!("../assets/atlas/region-overview.mjs"),
        "region-view.mjs" => include_str!("../assets/atlas/region-view.mjs"),
        "style.css" => include_str!("../assets/atlas/style.css"),
        "town-layout.mjs" => include_str!("../assets/atlas/town-layout.mjs"),
        "world-model.mjs" => include_str!("../assets/atlas/world-model.mjs"),
        "world-view.mjs" => include_str!("../assets/atlas/world-view.mjs"),
        "world.css" => include_str!("../assets/atlas/world.css"),
        "world.html" => include_str!("../assets/atlas/world.html"),
        _ => return None,
    };
    let mime = match name.rsplit_once('.').map(|(_, extension)| extension) {
        Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "text/html; charset=utf-8",
    };
    Some((mime, text))
}
