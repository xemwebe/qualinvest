use htmlescape::*;

use unic_ucd::category::GeneralCategory as gc;

/// Filters out separators, control codes, unicode surrogates, and a few others
/// as well as single/double quotes, backslashes, and angle braces.
///
/// This is used mainly for passwords/usernames and other user input that
/// should be sanitized without using with the `htmlescape` crate
pub fn filter_non_characters(string: &str) -> String {
    let mut output = String::with_capacity(string.len() + 5);
    for c in string.chars() {
        match c {
            '\'' | '"' | '\\' | '<' | '>' => {}
            _ => match gc::of(c) {
                gc::OtherSymbol
                | gc::SpaceSeparator
                | gc::LineSeparator
                | gc::ParagraphSeparator
                | gc::Control
                | gc::Format
                | gc::Surrogate
                | gc::PrivateUse
                | gc::Unassigned => {}
                _ => output.push(c),
            },
        }
    }
    output
}

/// Sanitize usernames to prevent xss and other vulnerabilities
/// Use sanitize() when escaping text that may be included in a html attribute (like value="<text>")
///
/// Usernames get embedded in a form input value attribute like:
/// <input type="text" name="username" value="<username>">
/// where the <username> is whatever is in the page's query string or Cookie/FlashMessage
///
/// The normal htmlescape::encode_minimal() encodes basic html entities
/// while the htmlescape::encode_attribute() encodes those from encode_minimal plus more,
/// as well as any non alpha-numeric ascii characters are hex encoded ( &#x00 );
pub fn sanitize(string: &str) -> String {
    encode_attribute(&filter_non_characters(string))
}

/// Used to remove all non-hexadecimal characters from passwords
/// Passwords must be only hex characters as it is expecting a hash, like sha-256 or md5 for example
pub fn sanitize_password(string: &str) -> String {
    filter_non_characters(string)
}
