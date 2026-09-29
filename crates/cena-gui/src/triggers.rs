//! The trigger editor's window (`plan/54`): the one triggers file, by
//! category, beside *Settings*.
//!
//! It draws the [`Book`] the binary last gave and asks for [`Change`]s; the
//! binary makes each through the writer `;trigger` uses, tells every running
//! character, and gives the window the file again. Nothing here reads or
//! writes the file.

mod editor;
mod form;
mod test;

pub(crate) use editor::Editor;

#[cfg(test)]
mod tests;
