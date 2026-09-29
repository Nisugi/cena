//! Who a relay is for: the character `;to` names, and the characters an
//! `;all` keeps or leaves out. Moved down from `relay.rs` when `;all`'s
//! lists took it past its cap.

use super::Running;

/// Which running characters an `;all` is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Who {
    /// Every one.
    Everyone,
    /// Every one but these, as typed.
    Except(Vec<String>),
    /// Only these, as typed.
    Only(Vec<String>),
}

/// The character `typed` picks among `running`: `GAME:Name` on that game,
/// or a name on whichever game has it. Its whole name, whatever the case,
/// or else the one whose name starts so.
///
/// # Errors
///
/// What to tell the player: nobody fits, or more than one does -- a whole
/// name on two games among them, which went to the first found
/// (the crate review of 2026-09-28, R6).
pub(crate) fn pick<'a>(typed: &str, running: &'a [Running]) -> Result<&'a Running, String> {
    let (game, name) = match typed.split_once(':') {
        Some((game, name)) => (Some(game.trim()), name.trim()),
        None => (None, typed),
    };
    let on_game = || {
        running
            .iter()
            .filter(move |one| game.is_none_or(|game| one.game.eq_ignore_ascii_case(game)))
    };
    let exact: Vec<&Running> = on_game()
        .filter(|one| one.name.eq_ignore_ascii_case(name))
        .collect();
    let lower = name.to_lowercase();
    let fits = if exact.is_empty() {
        on_game()
            .filter(|one| one.name.to_lowercase().starts_with(&lower))
            .collect()
    } else {
        exact
    };
    let labels = |fits: &[&Running]| {
        fits.iter()
            .map(|one| one.label(running))
            .collect::<Vec<_>>()
    };
    match fits.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("To: no character named {typed} is running here.")),
        [first, rest @ ..]
            if rest
                .iter()
                .all(|one| one.name.eq_ignore_ascii_case(&first.name)) =>
        {
            Err(format!(
                "To: {} is on more than one game; say which, as {}.",
                first.name,
                labels(&fits).join(" or ")
            ))
        }
        many => Err(format!(
            "To: {typed} could be {}; say more of the name.",
            labels(many).join(" or ")
        )),
    }
}

/// The characters `who` names among `running`, each name picked as `;to`
/// picks one.
///
/// # Errors
///
/// A name that picks nobody, or more than one, said as `;to` says it: the
/// whole line is refused.
pub(crate) fn chosen(who: &Who, running: &[Running]) -> Result<Vec<Running>, String> {
    // By place in `running`: two characters are two entries, whatever else
    // they share.
    let picked = |names: &[String]| -> Result<Vec<usize>, String> {
        names
            .iter()
            .map(|name| {
                let one = pick(name, running).map_err(|why| why.replacen("To:", "All:", 1))?;
                Ok(running
                    .iter()
                    .position(|each| std::ptr::eq(each, one))
                    .unwrap_or(usize::MAX))
            })
            .collect()
    };
    let keep = |keep: &dyn Fn(usize) -> bool| -> Vec<Running> {
        running
            .iter()
            .enumerate()
            .filter(|(at, _)| keep(*at))
            .map(|(_, one)| one.clone())
            .collect()
    };
    Ok(match who {
        Who::Everyone => running.to_vec(),
        Who::Except(names) => {
            let out = picked(names)?;
            keep(&|at| !out.contains(&at))
        }
        Who::Only(names) => {
            let only = picked(names)?;
            keep(&|at| only.contains(&at))
        }
    })
}
