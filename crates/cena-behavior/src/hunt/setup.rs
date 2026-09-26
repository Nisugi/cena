//! Map-driven configuration through the native TOML loader. No session handle,
//! game connection, authority, or command sender crosses this interface.

use super::profile::{FieldRest, Step, Target, Until};
use super::{Profile, chain};
use cena_map::{Map, RoomId};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The only operations setup can perform; starting is deliberately absent.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Render the effective configuration without writing.
    Preview,
    /// Create a new profile, only if it matches the reviewed preview.
    Save,
    /// Read an existing profile through the native inheritance chain.
    Load,
}

/// A bounded, explicit selection. Character identity is supplied by the host.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Read, preview, or create. Never run.
    pub operation: Operation,
    /// A new profile's safe filename, or the name to read.
    pub name: String,
    /// Exact map bytes used by the browser catalogue.
    pub map_sha256: String,
    /// The choice has been inspected, including provisional membership.
    pub acknowledged: bool,
    /// Explicit allowed membership, not legacy exclusion boundaries.
    pub allowed: Vec<u32>,
    /// Hunting start inside the selection.
    pub start: u32,
    /// Mandatory town recovery location.
    pub town: u32,
    /// Town commands; this does not install a script runtime.
    pub town_commands: Vec<String>,
    /// Optional field location, commands, and completion thresholds.
    pub field: Option<FieldRest>,
    /// Exact creature names; no automatic catch-all.
    pub targets: Vec<String>,
    /// Explicit native attack routine steps.
    pub attacks: Vec<String>,
    /// Return-to-hunt thresholds for town.
    pub until: Until,
    /// Previously previewed effective TOML, required for Save.
    pub preview: Option<String>,
}

/// What native configuration actually resolved to.
#[derive(Clone, Debug, Serialize)]
pub struct Reply {
    /// Profile filename stem.
    pub name: String,
    /// Effective, round-trippable native TOML.
    pub toml: String,
    /// True only after a new file was written and reloaded successfully.
    pub saved: bool,
    /// Always false; no operation here can start a hunt.
    pub started: bool,
}

/// Validate all pinned rooms against the actual native map before running.
///
/// # Errors
/// Missing rooms or an invalid allowed selection.
pub fn validate_map(profile: &Profile, map: &Map) -> Result<(), String> {
    let problems = profile.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let ids = profile
        .rooms
        .allowed
        .iter()
        .flatten()
        .copied()
        .chain(profile.rooms.hunting)
        .chain(profile.rooms.resting)
        .chain(profile.rest.field.as_ref().map(|field| field.room));
    for id in ids {
        if map.room(RoomId(id)).is_none() {
            return Err(format!("Room #{id} is absent from the native map"));
        }
    }
    Ok(())
}

/// A wandering-only graph. Rest/start travel still uses the full map.
/// Scripted crossings are omitted until their intermediate movement can be
/// proven inside membership; plain command exits retain their native costs.
///
/// # Errors
/// Invalid or missing membership rooms.
pub fn hunting_map(profile: &Profile, map: &Map) -> Result<Option<Map>, String> {
    let Some(allowed) = &profile.rooms.allowed else {
        return Ok(None);
    };
    validate_map(profile, map)?;
    let ids: std::collections::BTreeSet<_> = allowed.iter().copied().collect();
    let rooms = map
        .rooms()
        .iter()
        .filter(|r| ids.contains(&r.id.0))
        .map(|room| {
            let mut room = room.clone();
            room.exits.retain(|exit| {
                ids.contains(&exit.to.0) && matches!(exit.crossing, cena_map::Crossing::Command(_))
            });
            room
        })
        .collect();
    Map::from_rooms(rooms).map(Some).map_err(|e| e.to_string())
}

/// Configure a hunt for the host-selected character. Creates new profiles only;
/// existing/global/character files are never overwritten. Call off the actor.
///
/// # Errors
/// Unacknowledged or stale selection, invalid rooms/commands, inheritance
/// conflicts, changed preview, or any filesystem failure.
pub fn configure(
    dir: &Path,
    who: (&str, &str),
    map: &Map,
    hash: &str,
    request: &Request,
) -> Result<Reply, String> {
    if chain::file_name(&request.name).as_deref() != Some(&request.name) || request.name.len() > 100
    {
        return Err(
            "Use a profile name of lowercase letters, numbers, hyphens or underscores".into(),
        );
    }
    if request.map_sha256 != hash {
        return Err("Map changed: reload the matching map before setup".into());
    }
    if matches!(request.operation, Operation::Load) {
        let loaded =
            chain::load(dir, Some(who.0), Some(who.1), &request.name).map_err(|e| e.to_string())?;
        if loaded
            .profile
            .map_sha256
            .as_deref()
            .is_some_and(|saved| saved != hash)
        {
            return Err(
                "Saved profile belongs to different map bytes; review it before reuse".into(),
            );
        }
        validate_map(&loaded.profile, map)?;
        return Ok(Reply {
            name: request.name.clone(),
            toml: loaded.profile.to_toml()?,
            saved: false,
            started: false,
        });
    }
    let profile = candidate(request, hash)?;
    validate_map(&profile, map)?;
    let mut table = toml::Table::try_from(&profile).map_err(|e| e.to_string())?;
    // Only settings owned by this form belong in the new layer. In particular,
    // do not accidentally reset inherited loot, stance, signs, or preparation.
    table.retain(|key, _| {
        matches!(
            key,
            "map_sha256" | "rooms" | "rest" | "targets" | "routines"
        )
    });
    let settings = toml::to_string_pretty(&table).map_err(|e| e.to_string())?;
    let mut levels = Vec::new();
    if let Some(global) = optional(&chain::global_path(dir))? {
        levels.push(global);
    }
    levels.push(table);
    if let Some(path) = chain::character_path(dir, who.0, who.1)
        && let Some(character) = optional(&path)?
    {
        levels.push(character);
    }
    let effective = chain::resolve(levels)?;
    if effective.rooms != profile.rooms
        || effective.rest != profile.rest
        || effective.targets != profile.targets
        || effective.routines.get("map_attack") != profile.routines.get("map_attack")
        || effective.map_sha256 != profile.map_sha256
    {
        return Err("Character overrides change this selection, recovery, or attacks. Resolve those overrides explicitly; no file was changed".into());
    }
    validate_map(&effective, map)?;
    let toml = effective.to_toml()?;
    let save = matches!(request.operation, Operation::Save);
    if save {
        if request.preview.as_deref() != Some(&toml) {
            return Err(
                "Preview changed or missing: preview and review again before saving".into(),
            );
        }
        let path = chain::profile_path(dir, &request.name).ok_or("Invalid profile name")?;
        chain::write_new(&path, &settings).map_err(|e| format!("Profile not overwritten: {e}"))?;
        let loaded = chain::load(dir, Some(who.0), Some(who.1), &request.name)
            .map_err(|e| format!("Profile created but reload failed: {e}"))?;
        if loaded.profile != effective {
            return Err("Profile created, but inheritance changed during save. Reload and review; nothing started".into());
        }
    }
    Ok(Reply {
        name: request.name.clone(),
        toml,
        saved: save,
        started: false,
    })
}

fn optional(path: &Path) -> Result<Option<toml::Table>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => text
            .parse()
            .map(Some)
            .map_err(|e| format!("Invalid native settings: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Cannot read native settings: {e}")),
    }
}

fn lines(values: &[String]) -> Result<(), String> {
    if values.len() > 64
        || values.iter().any(|s| {
            s.trim().is_empty()
                || s.len() > 512
                || s.chars().any(char::is_control)
                || s.trim_start().starts_with(';')
        })
    {
        return Err("Use at most 64 nonempty single-line native commands, not ;scripts".into());
    }
    Ok(())
}

fn candidate(r: &Request, hash: &str) -> Result<Profile, String> {
    if !r.acknowledged {
        return Err("Inspect and acknowledge the chosen boundary before saving".into());
    }
    if r.allowed.is_empty() || r.allowed.len() > 4096 {
        return Err("Choose between 1 and 4096 allowed rooms".into());
    }
    if r.targets.is_empty()
        || r.targets.len() > 64
        || r.targets
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 120 || s.chars().any(char::is_control))
    {
        return Err("Choose explicit creature names".into());
    }
    lines(&r.attacks)?;
    lines(&r.town_commands)?;
    if r.attacks.is_empty() {
        return Err("Specify an attack routine; none is guessed".into());
    }
    if let Some(field) = &r.field {
        lines(&field.commands)?;
        completion(&field.until)?;
    }
    completion(&r.until)?;
    let mut p = Profile {
        map_sha256: Some(hash.into()),
        ..Profile::default()
    };
    let mut allowed = r.allowed.clone();
    allowed.sort_unstable();
    allowed.dedup();
    p.rooms.allowed = Some(allowed);
    p.rooms.hunting = Some(r.start);
    p.rooms.resting = Some(r.town);
    p.rest.fried = Some(100);
    p.rest.encumbered = Some(1);
    p.rest.mana_below = Some(20);
    p.rest.when.bleeding = true;
    p.rest.when.health_at_most = Some(99);
    p.rest.until = r.until.clone();
    p.rest.commands.clone_from(&r.town_commands);
    p.rest.field.clone_from(&r.field);
    p.targets = r
        .targets
        .iter()
        .map(|name| Target {
            name: Some(name.trim().to_lowercase()),
            any: false,
            routine: "map_attack".into(),
        })
        .collect();
    p.routines.insert(
        "map_attack".into(),
        r.attacks
            .iter()
            .map(|s| Step::parse(s))
            .collect::<Result<_, _>>()?,
    );
    Ok(p)
}

fn completion(until: &Until) -> Result<(), String> {
    if until.experience.is_none_or(|n| n >= 100)
        || until.mana.is_none_or(|n| !(20..=100).contains(&n))
        || until.stamina.is_some_and(|n| n > 100)
    {
        return Err("Recovery needs an experience threshold below 100 and a mana threshold from 20 to 100; stamina is a percentage".into());
    }
    Ok(())
}
