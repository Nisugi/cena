//! The bytes under the format: a bounds-checked reader, an interning writer,
//! and what can go wrong with either.

use std::collections::HashMap;
use std::fmt;

use crate::map::DuplicateRoom;

/// The first eight bytes of a map file.
pub const MAGIC: &[u8; 8] = b"HYDRAMAP";
/// The format version this build reads and writes.
pub const VERSION: u32 = 1;
/// "No string": an optional reference that is absent.
pub(super) const NONE: u32 = u32::MAX;

/// A room's sheet: which plate it is drawn on, and which area it belongs to.
///
/// One extension for both because they are one fact about where a room lives,
/// and a plate without its area is the ambiguity `Room::area` exists to
/// resolve.
pub(super) const EXT_SHEET: &str = "sheet";

/// A room's corrected position, as an offset from an unmoved anchor.
pub(super) const EXT_PLACEMENT: &str = "placement";

/// **Legacy, read and never written:** stated bearings keyed by destination.
///
/// Superseded by [`EXT_EXIT_DIRTO`] because a destination does not name an
/// edge. Two exits from one room can reach the same room -- a gate and a wall
/// to climb -- and keyed by `to` their bearings merged: the probe that found
/// it wrote `[Some(North), None]` and read back `[Some(North), Some(North)]`.
///
/// Still decoded so a map built before the fix loads with the bearings it
/// has (exact wherever a room has one exit per destination, which is the
/// common case). Not *reinterpreted*: changing what this name's blob means
/// would have an older client read an exit index as a room id -- a silent
/// misread, which rule 2 exists to prevent. A new name is rule 1's answer.
pub(super) const EXT_DIRTO: &str = "dirto";

/// Stated bearings for this room's exits, keyed by **exit index** -- the
/// position of the exit in the room's own exit list.
///
/// **A room extension although `dirto` is a per-EDGE fact**, because the exit
/// record ends at `cost` and has no extension slot of its own: appending a
/// field there would be read as a malformed exit by every client built before
/// it. The room's extension list is the only place a new field can arrive
/// without a version bump. The index is what makes the room-level record
/// per-edge again: it is the one key two parallel exits cannot share.
///
/// A client built before this name existed skips it by length (rule 1) and
/// falls through to the command text, which is what an absent bearing means
/// anyway.
pub(super) const EXT_EXIT_DIRTO: &str = "exit_dirto";

/// Why a map file did not load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Not a map file at all.
    BadMagic,
    /// A map file of a version this build cannot read. The format changed
    /// incompatibly; this client needs updating, or the map rebuilding.
    UnsupportedVersion { found: u32, supported: u32 },
    /// The file ends, or a count claims more than remains, at this offset.
    Truncated { at: usize },
    /// A string reference points past the string table.
    BadStringRef { reference: u32, at: usize },
    /// A string is not UTF-8.
    BadUtf8 { at: usize },
    /// Bytes remain after the last room.
    TrailingBytes { at: usize },
    /// Two rooms share an id.
    Duplicate(DuplicateRoom),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::BadMagic => write!(f, "not a Hydra map file"),
            LoadError::UnsupportedVersion { found, supported } => write!(
                f,
                "map file is format version {found}; this build reads version {supported}"
            ),
            LoadError::Truncated { at } => write!(f, "map file is truncated at byte {at}"),
            LoadError::BadStringRef { reference, at } => {
                write!(
                    f,
                    "string reference {reference} at byte {at} is outside the table"
                )
            }
            LoadError::BadUtf8 { at } => write!(f, "string at byte {at} is not UTF-8"),
            LoadError::TrailingBytes { at } => write!(f, "unexpected bytes after the map, at {at}"),
            LoadError::Duplicate(duplicate) => write!(f, "{duplicate}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Why a map could not be written: something in it does not fit the format's
/// 32-bit counts. Not reachable with a real map; checked rather than cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeError;

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the map has a list or string too large for the file format"
        )
    }
}

impl std::error::Error for EncodeError {}

/// A cursor over a map file. Every read is bounds-checked.
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    pub(super) fn pos(&self) -> usize {
        self.pos
    }

    pub(super) fn is_at_end(&self) -> bool {
        self.pos == self.bytes.len()
    }

    pub(super) fn take(&mut self, len: usize) -> Result<&'a [u8], LoadError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(LoadError::Truncated { at: self.pos })?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(LoadError::Truncated { at: self.pos })?;
        self.pos = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], LoadError> {
        let at = self.pos;
        self.take(N)?
            .try_into()
            .map_err(|_| LoadError::Truncated { at })
    }

    pub(super) fn u8(&mut self) -> Result<u8, LoadError> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, LoadError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(super) fn i32(&mut self) -> Result<i32, LoadError> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub(super) fn i64(&mut self) -> Result<i64, LoadError> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    /// A count of items that each take at least `min_size` bytes. **Refused if
    /// the bytes that remain could not hold them**, so a hostile or corrupt
    /// count cannot make the loader allocate for four billion rooms.
    pub(super) fn count(&mut self, min_size: usize) -> Result<usize, LoadError> {
        let at = self.pos;
        let count = usize::try_from(self.u32()?).map_err(|_| LoadError::Truncated { at })?;
        let remaining = self.bytes.len() - self.pos;
        match count.checked_mul(min_size) {
            Some(needed) if needed <= remaining => Ok(count),
            _ => Err(LoadError::Truncated { at }),
        }
    }

    /// A length-prefixed blob.
    pub(super) fn blob(&mut self) -> Result<&'a [u8], LoadError> {
        let len = self.count(1)?;
        self.take(len)
    }
}

/// Builds a file body while interning its strings.
#[derive(Default)]
pub(super) struct Writer {
    pub(super) body: Vec<u8>,
    strings: Vec<String>,
    index: HashMap<String, u32>,
}

impl Writer {
    pub(super) fn u8(&mut self, value: u8) {
        self.body.push(value);
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.body.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn i32(&mut self, value: i32) {
        self.body.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn i64(&mut self, value: i64) {
        self.body.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn len(&mut self, len: usize) -> Result<(), EncodeError> {
        self.u32(u32::try_from(len).map_err(|_| EncodeError)?);
        Ok(())
    }

    /// The reference for a string, interning it on first sight.
    pub(super) fn intern(&mut self, text: &str) -> Result<u32, EncodeError> {
        if let Some(reference) = self.index.get(text) {
            return Ok(*reference);
        }
        let reference = u32::try_from(self.strings.len()).map_err(|_| EncodeError)?;
        if reference == NONE {
            return Err(EncodeError);
        }
        self.strings.push(text.to_owned());
        self.index.insert(text.to_owned(), reference);
        Ok(reference)
    }

    /// A named extension whose body is written in place: `u32 name ref,
    /// u32 length, blob`.
    ///
    /// **Distinct from [`Self::named`], which takes an already-built slice.**
    /// That is right for a blob of fixed-layout bytes and cannot serve a body
    /// containing *string references*, because interning happens against the
    /// file-level table and a caller building bytes on the side has no access
    /// to it.
    ///
    /// The length is **reserved and back-patched**, because the body is what
    /// `write` produces and its size is not known until it has run. Writing
    /// the blob into a nested `Writer` instead would intern its strings into a
    /// table nobody reads -- the string table is file-level.
    ///
    /// This is rule 1's mechanism: `decode` skips an extension it does not
    /// know by this length, so a name added later cannot break an older
    /// client.
    pub(super) fn extension(
        &mut self,
        name: &str,
        write: impl FnOnce(&mut Self) -> Result<(), EncodeError>,
    ) -> Result<(), EncodeError> {
        self.string(name)?;
        let at = self.body.len();
        self.u32(0);
        write(self)?;
        let len = u32::try_from(self.body.len() - at - 4).map_err(|_| EncodeError)?;
        self.body[at..at + 4].copy_from_slice(&len.to_le_bytes());
        Ok(())
    }

    pub(super) fn string(&mut self, text: &str) -> Result<(), EncodeError> {
        let reference = self.intern(text)?;
        self.u32(reference);
        Ok(())
    }

    /// A name and a length-prefixed blob: rule 1's unit.
    pub(super) fn named(&mut self, name: &str, blob: &[u8]) -> Result<(), EncodeError> {
        self.string(name)?;
        self.len(blob.len())?;
        self.body.extend_from_slice(blob);
        Ok(())
    }

    /// Header, string table, then the body.
    pub(super) fn finish(self) -> Result<Vec<u8>, EncodeError> {
        let mut out = Vec::with_capacity(self.body.len() + self.strings.len() * 16);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(self.strings.len())
                .map_err(|_| EncodeError)?
                .to_le_bytes(),
        );
        for text in &self.strings {
            out.extend_from_slice(
                &u32::try_from(text.len())
                    .map_err(|_| EncodeError)?
                    .to_le_bytes(),
            );
            out.extend_from_slice(text.as_bytes());
        }
        out.extend_from_slice(&self.body);
        Ok(out)
    }
}
