//! Public reference snapshot, separate from private character state/control.
//! All assets use a compile-time allowlist and the server's existing guard/CSP.

use axum::{
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};

pub(crate) async fn home() -> Redirect {
    Redirect::temporary("/atlas/corpus/")
}

pub(crate) async fn asset(Path(path): Path<String>) -> Response {
    response(&path)
}

fn response(path: &str) -> Response {
    if let Some(bytes) = crate::atlas_data::get(path) {
        return ([("content-type", "application/json")], bytes).into_response();
    }
    let Some((region, name)) = path.split_once('/') else {
        // Shared modules are also imported by the character page. Still a
        // literal allowlist, never a filesystem lookup.
        if let Some((mime, text)) = crate::atlas_assets::get(path) {
            return ([("content-type", mime)], text).into_response();
        }
        return StatusCode::NOT_FOUND.into_response();
    };
    if region != "corpus" && !crate::atlas_data::region(region) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let name = if name.is_empty() || name == "index.html" {
        if region == "corpus" {
            "world.html"
        } else {
            "index.html"
        }
    } else {
        name
    };
    match crate::atlas_assets::get(name) {
        Some((mime, bytes)) => ([("content-type", mime)], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_assets_and_regions_are_served() {
        for path in [
            "corpus/",
            "landing/",
            "landing/app.mjs",
            "icemule/style.css",
        ] {
            assert_eq!(response(path).status(), StatusCode::OK, "{path}");
        }
        for path in [
            "../index.html",
            "landing/../app.js",
            "landing/.env",
            "unknown/",
            "landing/data.json.gz",
            "landing/nested/app.mjs",
            "landing",
        ] {
            assert_eq!(response(path).status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[test]
    fn snapshots_are_json_not_executable_content() {
        for path in [
            "corpus/world.json",
            "corpus/search.json",
            "landing/data.json",
            "landing/region-data.json",
        ] {
            let response = response(path);
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["content-type"], "application/json");
            let text = crate::atlas_data::get(path).expect("snapshot");
            assert!(serde_json::from_str::<serde_json::Value>(text).is_ok());
        }
    }
}
