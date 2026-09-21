//! Bounded manual command vocabulary; validation does not send anything.

use std::fmt;

pub const MAX_COMMAND_BYTES: usize = 4096;
pub const MAX_REQUEST_ID_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    InvalidSession,
    InvalidGeneration,
    InvalidRequestId,
    EmptyCommand,
    CommandTooLong,
    MultipleLines,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSession => "session must be a canonical decimal u64 string",
            Self::InvalidGeneration => "generation must be a canonical decimal u64 string",
            Self::InvalidRequestId => {
                "request ID must contain 1–64 ASCII letters, digits, hyphens or underscores"
            }
            Self::EmptyCommand => "command must contain non-whitespace text",
            Self::CommandTooLong => "command exceeds 4096 UTF-8 bytes",
            Self::MultipleLines => "command must not contain CR, LF or NUL",
        })
    }
}

impl std::error::Error for InputError {}

/// Validate a command without trimming or otherwise changing what gets sent.
///
/// This does not check the active session/generation or duplicate request IDs;
/// those depend on native state and the authenticated connection.
///
/// # Errors
/// Returns the first invalid identity, request ID, or command field.
pub fn validate_command(
    session: &str,
    generation: &str,
    request_id: &str,
    line: &str,
) -> Result<(), InputError> {
    if !decimal_id(session) {
        return Err(InputError::InvalidSession);
    }
    if !decimal_id(generation) {
        return Err(InputError::InvalidGeneration);
    }
    if request_id.is_empty()
        || request_id.len() > MAX_REQUEST_ID_BYTES
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(InputError::InvalidRequestId);
    }
    if line.len() > MAX_COMMAND_BYTES {
        return Err(InputError::CommandTooLong);
    }
    if line.contains(['\r', '\n', '\0']) {
        return Err(InputError::MultipleLines);
    }
    if line.trim().is_empty() {
        return Err(InputError::EmptyCommand);
    }
    Ok(())
}

fn decimal_id(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value.len() == 1 || !value.starts_with('0'))
        && value.parse::<u64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_commands_preserve_unicode_spaces_and_large_ids() {
        assert!(validate_command("18446744073709551615", "0", "cmd-1", " say 您好 ").is_ok());
        assert!(validate_command("0", "0", "a", &"a".repeat(MAX_COMMAND_BYTES)).is_ok());
    }

    #[test]
    fn invalid_identity_and_request_fields_never_reach_the_queue() {
        for bad in ["", "00", "+1", "-1", "1.0", " 1", "18446744073709551616"] {
            assert_eq!(
                validate_command(bad, "0", "a", "look"),
                Err(InputError::InvalidSession)
            );
            assert_eq!(
                validate_command("0", bad, "a", "look"),
                Err(InputError::InvalidGeneration)
            );
        }
        for bad in [
            String::new(),
            "a".repeat(65),
            "line\nbreak".to_owned(),
            "é".to_owned(),
        ] {
            assert_eq!(
                validate_command("0", "0", &bad, "look"),
                Err(InputError::InvalidRequestId)
            );
        }
    }

    #[test]
    fn command_size_is_bytes_and_newline_injection_is_rejected() {
        for bad in ["look\nquit", "look\rquit", "look\0quit"] {
            assert_eq!(
                validate_command("0", "0", "a", bad),
                Err(InputError::MultipleLines)
            );
        }
        assert_eq!(
            validate_command("0", "0", "a", " \t"),
            Err(InputError::EmptyCommand)
        );
        assert_eq!(
            validate_command("0", "0", "a", &"🦀".repeat(1025)),
            Err(InputError::CommandTooLong)
        );
    }
}
