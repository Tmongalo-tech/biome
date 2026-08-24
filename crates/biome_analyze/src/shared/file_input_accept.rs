use biome_unicode_table::{Dispatch::*, lookup_byte};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The reason an `accept` value is invalid.
pub enum InvalidAcceptValue {
    EmptyEntry,
    Extension,
    MimeType,
    WildcardMimeType,
}

impl InvalidAcceptValue {
    pub const fn explanation(self) -> &'static str {
        match self {
            Self::EmptyEntry => "Empty entries are not allowed.",
            Self::Extension => {
                "File extensions must start with . and cannot contain whitespace, commas, slashes, wildcards, or template markers."
            }
            Self::MimeType => "MIME types must use valid type/subtype syntax.",
            Self::WildcardMimeType => "Wildcard MIME types must be audio/*, image/*, or video/*.",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
/// The result of validating an `accept` value.
pub enum AcceptValueResult {
    Valid,
    Invalid(InvalidAcceptValue),
    Normalize,
}

/// Validates a file input `accept` value.
pub fn normalize_file_input_accept(value: &str) -> AcceptValueResult {
    let mut needs_normalization = false;

    for (index, raw_token) in value.split(',').enumerate() {
        let token = raw_token.trim();
        if token.is_empty() {
            return AcceptValueResult::Invalid(InvalidAcceptValue::EmptyEntry);
        }

        let replacement = match normalize_accept_token(token) {
            Ok(replacement) => replacement,
            Err(error) => return AcceptValueResult::Invalid(error),
        };
        let has_canonical_spacing = if index == 0 {
            raw_token == token
        } else {
            raw_token.strip_prefix(' ') == Some(token)
        };
        needs_normalization |= !has_canonical_spacing
            || !replacement.same_as_source(token)
            || normalized_token_seen_before(value, index, replacement);
    }

    if needs_normalization {
        AcceptValueResult::Normalize
    } else {
        AcceptValueResult::Valid
    }
}

/// Returns the normalized spelling of a valid file input `accept` value.
pub fn normalized_file_input_accept(value: &str) -> String {
    let mut output = String::with_capacity(value.len());

    for (index, raw_token) in value.split(',').enumerate() {
        let Ok(token) = normalize_accept_token(raw_token.trim()) else {
            continue;
        };
        if normalized_token_seen_before(value, index, token) {
            continue;
        }
        if !output.is_empty() {
            output.push_str(", ");
        }
        token.push_to(&mut output);
    }

    output
}

#[derive(Clone, Copy)]
struct NormalizedAcceptToken<'a> {
    value: &'a str,
    prefix_dot: bool,
}

impl NormalizedAcceptToken<'_> {
    fn chars(&self) -> impl Iterator<Item = char> + '_ {
        self.prefix_dot
            .then_some('.')
            .into_iter()
            .chain(self.value.chars().flat_map(char::to_lowercase))
    }

    fn same_as(&self, other: Self) -> bool {
        self.chars().eq(other.chars())
    }

    fn same_as_source(&self, source: &str) -> bool {
        self.chars().eq(source.chars())
    }

    fn push_to(&self, output: &mut String) {
        output.extend(self.chars());
    }
}

fn normalized_token_seen_before(value: &str, index: usize, token: NormalizedAcceptToken) -> bool {
    value
        .split(',')
        .take(index)
        .filter_map(|raw_token| normalize_accept_token(raw_token.trim()).ok())
        .any(|previous| previous.same_as(token))
}

fn normalize_accept_token(token: &str) -> Result<NormalizedAcceptToken<'_>, InvalidAcceptValue> {
    if token.starts_with('.') {
        normalize_extension(token)
    } else if token.contains('/') {
        normalize_mime_type(token)
    } else {
        normalize_bare_extension(token)
    }
}

fn normalize_extension(token: &str) -> Result<NormalizedAcceptToken<'_>, InvalidAcceptValue> {
    if token.strip_prefix('.').is_some_and(|extension| {
        !extension.is_empty() && !has_invalid_extension_character(extension)
    }) {
        Ok(NormalizedAcceptToken {
            value: token,
            prefix_dot: false,
        })
    } else {
        Err(InvalidAcceptValue::Extension)
    }
}

fn normalize_bare_extension(token: &str) -> Result<NormalizedAcceptToken<'_>, InvalidAcceptValue> {
    if !token.is_empty() && !has_invalid_extension_character(token) {
        Ok(NormalizedAcceptToken {
            value: token,
            prefix_dot: true,
        })
    } else {
        Err(InvalidAcceptValue::Extension)
    }
}

fn has_invalid_extension_character(value: &str) -> bool {
    value.chars().any(|character| {
        character.is_whitespace() || matches!(character, ',' | '/' | '*' | '$' | '{' | '}')
    })
}

fn normalize_mime_type(token: &str) -> Result<NormalizedAcceptToken<'_>, InvalidAcceptValue> {
    if token.contains('*') {
        return ["audio/*", "image/*", "video/*"]
            .into_iter()
            .any(|expected| token.eq_ignore_ascii_case(expected))
            .then_some(NormalizedAcceptToken {
                value: token,
                prefix_dot: false,
            })
            .ok_or(InvalidAcceptValue::WildcardMimeType);
    }

    let essence = token
        .split_once(';')
        .map_or(token, |(essence, _)| essence)
        .trim();
    let Some((media_type, subtype)) = essence.split_once('/') else {
        return Err(InvalidAcceptValue::MimeType);
    };
    if media_type.is_empty()
        || subtype.is_empty()
        || subtype.contains('/')
        || !media_type.bytes().all(is_http_token_byte)
        || !subtype.bytes().all(is_http_token_byte)
    {
        return Err(InvalidAcceptValue::MimeType);
    }

    let value = if essence.eq_ignore_ascii_case("application/x-rar-compressed") {
        "application/vnd.rar"
    } else if essence.eq_ignore_ascii_case("application/x-zip-compressed") {
        "application/zip"
    } else if essence.eq_ignore_ascii_case("image/jpg") {
        "image/jpeg"
    } else if essence.eq_ignore_ascii_case("image/svg") {
        "image/svg+xml"
    } else if essence.eq_ignore_ascii_case("image/x-icon") {
        "image/vnd.microsoft.icon"
    } else {
        essence
    };
    Ok(NormalizedAcceptToken {
        value,
        prefix_dot: false,
    })
}

fn is_http_token_byte(byte: u8) -> bool {
    matches!(
        lookup_byte(byte),
        IDT | ZER
            | DIG
            | EXL
            | HAS
            | DOL
            | PRC
            | AMP
            | MUL
            | PLS
            | MIN
            | PRD
            | CRT
            | TPL
            | PIP
            | TLD
    ) || byte == b'\''
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptValueResult, InvalidAcceptValue, normalize_file_input_accept,
        normalized_file_input_accept,
    };

    #[test]
    fn accepts_normalized_values() {
        assert_eq!(
            normalize_file_input_accept("image/png, .png, audio/*"),
            AcceptValueResult::Valid
        );
    }

    #[test]
    fn normalizes_values() {
        let value = "IMAGE/JPG,.PNG, image/jpeg";
        assert_eq!(
            normalize_file_input_accept(value),
            AcceptValueResult::Normalize
        );
        assert_eq!(normalized_file_input_accept(value), "image/jpeg, .png");

        let value = "image/svg; charset=utf-8";
        assert_eq!(
            normalize_file_input_accept(value),
            AcceptValueResult::Normalize
        );
        assert_eq!(normalized_file_input_accept(value), "image/svg+xml");
    }

    #[test]
    fn rejects_invalid_values() {
        assert_eq!(
            normalize_file_input_accept("image/png,"),
            AcceptValueResult::Invalid(InvalidAcceptValue::EmptyEntry)
        );
        assert_eq!(
            normalize_file_input_accept("text/*"),
            AcceptValueResult::Invalid(InvalidAcceptValue::WildcardMimeType)
        );
        assert_eq!(
            normalize_file_input_accept("image//png"),
            AcceptValueResult::Invalid(InvalidAcceptValue::MimeType)
        );
    }
}
