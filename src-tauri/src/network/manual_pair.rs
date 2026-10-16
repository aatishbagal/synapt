//! Manual device pairing helpers: invite-code encoding and local address lookup.
//!
//! Manual pairing exists for networks where multicast discovery does not work
//! (guest Wi-Fi, VLAN-separated subnets, wired-to-wireless bridges). It only
//! bypasses *discovery*: the pairing ceremony itself is byte-for-byte the same
//! X25519 ECDH exchange used for auto-discovered peers, and the responder still
//! validates every inbound request. An invite code carries nothing secret, only
//! the address the initiator should dial.

use std::net::Ipv4Addr;

use crate::network::discovery::PAIRING_PORT;

/// Alphabet for invite codes. Excludes characters that are easily confused when
/// read aloud or handwritten (B, I, O, S, Z, 0, 1, 2, 5, 6, 8), so a code can be
/// dictated over a phone call without ambiguity.
const ALPHABET: &[u8; 25] = b"ACDEFGHJKLMNPQRTUVWXY3479";

/// Number of encoded characters, excluding the separator.
///
/// A code packs an IPv4 address and a port into 48 bits. base-25 needs 11 digits
/// to cover that range: 25^10 is about 9.5e13, short of the 2.8e14 a full 48-bit
/// value can reach, so ten digits would silently truncate high addresses.
const CODE_LEN: usize = 11;

/// Index in the rendered code at which the readability separator is inserted.
const DASH_AT: usize = 5;

/// Encode an address and port as a human-dictatable invite code.
///
/// The rendered form is `XXXXX-XXXXXX`: eleven alphabet characters split by a
/// dash. Because the port occupies the low 16 bits, a mistyped character almost
/// always decodes to a port other than the pairing port and is rejected outright
/// rather than silently producing a wrong address.
pub fn encode_invite(ip: Ipv4Addr, port: u16) -> String {
    let mut value = ((u32::from(ip) as u64) << 16) | (port as u64);
    let base = ALPHABET.len() as u64;

    // Build least-significant digit first, then reverse.
    let mut digits = [ALPHABET[0]; CODE_LEN];
    for slot in digits.iter_mut().rev() {
        *slot = ALPHABET[(value % base) as usize];
        value /= base;
    }

    let mut out = String::with_capacity(CODE_LEN + 1);
    for (i, ch) in digits.iter().enumerate() {
        if i == DASH_AT {
            out.push('-');
        }
        out.push(*ch as char);
    }
    out
}

/// Decode an invite code back into the address and port it names.
///
/// Accepts the code in any case and with dashes or spaces anywhere. Returns
/// `None` when the code is the wrong length, contains a character outside the
/// alphabet, or does not carry the pairing port, which is what a typo produces.
pub fn decode_invite(code: &str) -> Option<(Ipv4Addr, u16)> {
    let cleaned: Vec<u8> = code
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'-')
        .map(|b| b.to_ascii_uppercase())
        .collect();

    if cleaned.len() != CODE_LEN {
        return None;
    }

    let base = ALPHABET.len() as u64;
    let mut value: u64 = 0;
    for ch in cleaned {
        let digit = ALPHABET.iter().position(|a| *a == ch)? as u64;
        // CODE_LEN base-25 digits cannot overflow u64, so this cannot wrap.
        value = value * base + digit;
    }

    let port = (value & 0xFFFF) as u16;
    if port != PAIRING_PORT {
        return None;
    }

    let ip_bits = u32::try_from(value >> 16).ok()?;
    Some((Ipv4Addr::from(ip_bits), port))
}

/// Best local IPv4 address to show the user for manual pairing.
///
/// Returns `None` when no usable interface was found, in which case discovery
/// has nothing to bind to either and manual pairing cannot help.
pub fn get_local_ip() -> Option<Ipv4Addr> {
    let ip = crate::network::discovery::select_interface();
    if ip.is_unspecified() || ip.is_loopback() {
        return None;
    }
    Some(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_then_decode_roundtrips() {
        let ip = Ipv4Addr::new(192, 168, 1, 42);
        let code = encode_invite(ip, PAIRING_PORT);
        assert_eq!(decode_invite(&code), Some((ip, PAIRING_PORT)));
    }

    /// The high addresses are the ones a too-short code would truncate, so they
    /// are the cases worth pinning down.
    #[test]
    fn roundtrips_across_the_full_address_range() {
        for ip in [
            Ipv4Addr::new(0, 0, 0, 1),
            Ipv4Addr::new(10, 0, 0, 7),
            Ipv4Addr::new(172, 16, 254, 3),
            Ipv4Addr::new(192, 168, 255, 255),
            Ipv4Addr::new(255, 255, 255, 255),
        ] {
            let code = encode_invite(ip, PAIRING_PORT);
            assert_eq!(decode_invite(&code), Some((ip, PAIRING_PORT)), "failed for {ip}");
        }
    }

    #[test]
    fn encoded_code_is_eleven_characters_plus_one_dash() {
        let code = encode_invite(Ipv4Addr::new(192, 168, 1, 42), PAIRING_PORT);
        assert_eq!(code.len(), CODE_LEN + 1);
        assert_eq!(code.matches('-').count(), 1);
        assert_eq!(code.as_bytes()[DASH_AT], b'-');
    }

    #[test]
    fn decode_rejects_a_non_alphabet_string() {
        assert_eq!(decode_invite("BBBBB-BBBBBB"), None);
        assert_eq!(decode_invite("!!!!!-!!!!!!"), None);
    }

    #[test]
    fn decode_rejects_wrong_length() {
        assert_eq!(decode_invite(""), None);
        assert_eq!(decode_invite("ACDEF"), None);
        assert_eq!(decode_invite("ACDEF-GHJKLMN"), None);
    }

    #[test]
    fn decode_accepts_lowercase_and_stray_separators() {
        let ip = Ipv4Addr::new(192, 168, 1, 42);
        let code = encode_invite(ip, PAIRING_PORT);
        let mangled = format!(" {} ", code.to_lowercase().replace('-', " - "));
        assert_eq!(decode_invite(&mangled), Some((ip, PAIRING_PORT)));
    }

    /// A code naming any port other than the pairing port is not one of ours.
    #[test]
    fn decode_rejects_a_code_carrying_the_wrong_port() {
        let code = encode_invite(Ipv4Addr::new(192, 168, 1, 42), 1234);
        assert_eq!(decode_invite(&code), None);
    }

    /// Single-character typos should be caught rather than resolving to some
    /// other plausible address the user would then fail to reach.
    #[test]
    fn a_single_character_typo_is_overwhelmingly_rejected() {
        let code = encode_invite(Ipv4Addr::new(192, 168, 1, 42), PAIRING_PORT);
        let bytes: Vec<u8> = code.bytes().filter(|b| *b != b'-').collect();

        let mut accepted = 0;
        let mut total = 0;
        for pos in 0..bytes.len() {
            for replacement in ALPHABET.iter() {
                if *replacement == bytes[pos] {
                    continue;
                }
                let mut typo = bytes.clone();
                typo[pos] = *replacement;
                total += 1;
                let text = String::from_utf8(typo).expect("alphabet is ASCII");
                if decode_invite(&text).is_some() {
                    accepted += 1;
                }
            }
        }
        assert!(total > 0);
        assert!(
            accepted * 100 < total,
            "expected typos to be rejected, {accepted} of {total} slipped through"
        );
    }

    #[test]
    fn alphabet_has_no_ambiguous_characters() {
        for bad in *b"BIOSZ012568" {
            assert!(
                !ALPHABET.contains(&bad),
                "ambiguous character in alphabet: {}",
                bad as char
            );
        }
        let unique: std::collections::HashSet<u8> = ALPHABET.iter().copied().collect();
        assert_eq!(unique.len(), ALPHABET.len(), "alphabet has a duplicate");
    }

    #[test]
    fn get_local_ip_does_not_panic() {
        let _ = get_local_ip();
    }
}
