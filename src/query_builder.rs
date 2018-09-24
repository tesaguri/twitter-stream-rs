use std::fmt::{self, Display, Formatter, Write};
use std::str;
use std::time::{SystemTime, UNIX_EPOCH};

use byteorder::{BigEndian, ByteOrder};
use hmac::{Hmac, Mac};
use hyper::Uri;
use percent_encoding::{EncodeSet as EncodeSet_, PercentEncode as PercentEncode_};
use rand::thread_rng;
use rand::distributions::{Alphanumeric, Distribution};
use sha1::Sha1;

/// Builds URI query / x-www-form-urlencoded string and OAuth header string.
pub struct QueryBuilder {
    header: String,
    query: String,
    mac: MacWrite<Hmac<Sha1>>,
    next_append: Append,
    #[cfg(debug_assertions)]
    prev_key: String,
}

pub struct QueryOutcome {
    /// `Authorization` header string.
    pub header: String,
    /// A URI with query string or a x-www-form-urlencoded string.
    pub query: String,
}

bitflags! {
    struct Append: u8 {
        const QUESTION  = 0b001;
        const AMPERSAND = 0b010;
        const COMMA     = 0b100;
    }
}

struct Base64PercentEncode<'a>(&'a [u8]);

struct DoublePercentEncode<'a>(&'a str);

struct MacWrite<M>(M);

struct PercentEncode<D>(D);

// https://tools.ietf.org/html/rfc3986#section-2.1
#[derive(Clone)]
struct EncodeSet;

impl QueryBuilder {
    /// Returns a `QueryBuilder` that appends query string to `uri`.
    pub fn new(cs: &str, as_: &str, method: &str, uri: &Uri) -> Self {
        Self::new_(cs, as_, method, uri, true)
    }

    /// Returns a `QueryBuilder` that builds a x-www-form-urlencoded string.
    pub fn new_form(cs: &str, as_: &str, method: &str, uri: &Uri) -> Self {
        Self::new_(cs, as_, method, uri, false)
    }

    fn new_(cs: &str, as_: &str, method: &str, uri: &Uri, q: bool) -> Self {
        let standard_header_len = str::len("\
            OAuth \
            oauth_consumer_key=\"XXXXXXXXXXXXXXXXXXXXXXXXX\",\
            oauth_nonce=\"XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX\",\
            oauth_signature_method=\"HMAC-SHA1\",\
            oauth_timestamp=\"NNNNNNNNNN\",\
            oauth_token=\"NNNNNNNNNNNNNNNNNNN-\
                XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX\",\
            oauth_version=\"1.0\",\
            oauth_signature=\"\
                %XX%XX%XX%XX%XX%XX%XX%XX%XX%XX%XX%XX%XX%XX\
                %XX%XX%XX%XX%XX%XX%XX%XX%XX%XX%XX%XX%XX%XX\"\
        ");

        let mut header = String::with_capacity(standard_header_len);
        header.push_str("OAuth ");

        let mut signing_key = String::with_capacity(
            3 * (cs.len() + as_.len()) + 1
        );
        write!(signing_key, "{}&{}", percent_encode(cs), percent_encode(as_))
            .unwrap();
        let mut mac = MacWrite(
            Hmac::new_varkey(signing_key.as_bytes()).unwrap()
        );

        let query = if q { uri.to_string() } else { String::new() };

        struct PercentEncodeUri<'a>(&'a Uri);
        impl<'a> Display for PercentEncodeUri<'a> {
            fn fmt(&self, f: &mut Formatter) -> fmt::Result {
                if let Some(scheme) = self.0.scheme_part() {
                    write!(f, "{}%3A%2F%2F", scheme)?;
                }
                if let Some(authority) = self.0.authority_part() {
                    write!(f, "{}", percent_encode(authority.as_ref()))?;
                }
                write!(f, "{}", percent_encode(self.0.path()))?;
                // Query part is not used here
                Ok(())
            }
        }
        write!(mac, "{}&{}&", method, PercentEncodeUri(uri)).unwrap();

        let next_append = if q { Append::QUESTION } else { Append::empty() };

        #[cfg(debug_assertions)] {
            QueryBuilder {
                header, query, mac, next_append,
                prev_key: String::new(),
            }
        } #[cfg(not(debug_assertions))] {
            QueryBuilder { header, query, mac, next_append }
        }
    }

    pub fn append(&mut self, k: &str, v: &str) {
        self.check_dictionary_order(k);
        self.append_delim();
        write!(self.query, "{}={}", k, percent_encode(v)).unwrap();
        self.mac_input(k, v);
    }

    pub fn append_encoded<V: Display>(&mut self, k: &str, v: V) {
        self.check_dictionary_order(k);
        self.append_delim();
        write!(self.query, "{}={}", k, v).unwrap();
        self.mac_input_encoded(k, PercentEncode(v));
    }

    pub fn append_oauth_params(&mut self, ck: &str, ak: &str) {
        let nonce = unsafe {
            let mut buf = [0; 32];
            Alphanumeric.sample_iter(&mut thread_rng())
                .zip(&mut buf)
                .for_each(|(c, r)| *r = c as u8);
            debug_assert!(buf.is_ascii());
            ::string::String::<[u8; 32]>::from_utf8_unchecked(buf)
        };
        let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            #[cold] Err(_) => 0,
        };
        self.append_oauth_params_(ck, ak, &nonce, timestamp);
    }

    fn append_oauth_params_(
        &mut self,
        ck: &str,
        ak: &str,
        nonce: &str,
        timestamp: u64,
    ) {
        self.append_to_header("oauth_consumer_key", ck);
        self.append_to_header_encoded("oauth_nonce", nonce);
        self.append_to_header_encoded("oauth_signature_method", "HMAC-SHA1");
        self.append_to_header_encoded("oauth_timestamp", timestamp);
        self.append_to_header("oauth_token", ak);
        self.append_to_header_encoded("oauth_version", "1.0");
    }

    fn append_to_header(&mut self, k: &str, v: &str) {
        self.check_dictionary_order(k);
        write!(self.header, r#"{}="{}","#, k, percent_encode(v)).unwrap();
        self.mac_input(k, v);
    }

    fn append_to_header_encoded<V: Display>(&mut self, k: &str, v: V) {
        self.check_dictionary_order(k);
        write!(self.header, r#"{}="{}","#, k, v).unwrap();
        self.mac_input_encoded(k, PercentEncode(v));
    }

    fn append_delim(&mut self) {
        if self.next_append.contains(Append::QUESTION) {
            self.query.push('?');
            self.next_append.remove(Append::QUESTION);
        }
        if self.next_append.contains(Append::AMPERSAND) {
            self.query.push('&');
        } else {
            self.next_append.insert(Append::AMPERSAND);
        }
    }

    fn mac_input(&mut self, k: &str, v: &str) {
        self.mac_input_encoded(k, DoublePercentEncode(v));
    }

    fn mac_input_encoded<V: Display>(&mut self, k: &str, v: V) {
        if self.next_append.contains(Append::COMMA) {
            self.mac.write_str("%26").unwrap();
        } else {
            self.next_append.insert(Append::COMMA);
        }
        write!(self.mac, "{}%3D{}", k, v).unwrap();
    }

    fn check_dictionary_order(&mut self, _k: &str) {
        #[cfg(debug_assertions)] {
            assert!(*self.prev_key < *_k,
                "keys must be inserted in dictionary order",
            );
            self.prev_key = _k.to_owned();
        }
    }

    pub fn build(mut self) -> QueryOutcome {
        let s = self.mac.0.result().code();
        write!(self.header, r#"oauth_signature="{}""#, Base64PercentEncode(&s))
            .unwrap();
        let QueryBuilder { header, query, .. } = self;
        QueryOutcome { header, query }
    }
}

impl<'a> Display for Base64PercentEncode<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        const ENCODE: [&str; 0b0100_0000] = [
            "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M",
            "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
            "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
            "%2B", "%2F",
        ];

        assert_eq!(self.0.len(), 20);

        macro_rules! write_enc {
            ($bytes:expr, $shr:expr) => {{
                f.write_str(ENCODE[(($bytes >> $shr) & 0b11_1111) as usize])?;
            }};
        }

        let bytes = BigEndian::read_u128(self.0);
        for i in 0..16 {
            write_enc!(bytes, 128 - 6 - 6 * i);
        }
        let bytes = BigEndian::read_u64(&self.0[12..20]);
        for i in 0..10 {
            write_enc!(bytes, 64 - 6 - 6 * i);
        }
        f.write_str(ENCODE[((bytes << 2) & 0b11_1111) as usize])?;

        // '='
        f.write_str("%3D")
    }
}

impl<'a> Display for DoublePercentEncode<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut bytes = self.0.as_bytes();
        while let Some((&b, rem)) = bytes.split_first() {
            if EncodeSet.contains(b) {
                f.write_str(double_encode_byte(b))?;
                bytes = rem;
                continue;
            }

            // Write as much characters as possible at once:
            if let Some((i, &b)) = bytes.iter().enumerate().skip(1)
                .find(|&(_, &b)| EncodeSet.contains(b))
            {
                let rem = &bytes[i+1..];
                let s = &bytes[..i];
                debug_assert!(s.is_ascii());
                f.write_str(unsafe { str::from_utf8_unchecked(s)})?;
                f.write_str(double_encode_byte(b))?;
                bytes = rem;
            } else {
                debug_assert!(bytes.is_ascii());
                return f.write_str(unsafe { str::from_utf8_unchecked(bytes) });
            }
        }

        Ok(())
    }
}

impl<D: Display> Display for PercentEncode<D> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        struct Adapter<'a, 'b: 'a>(&'a mut Formatter<'b>);
        impl<'a, 'b: 'a> Write for Adapter<'a, 'b> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                Display::fmt(&percent_encode(s), self.0)
            }
        }
        write!(Adapter(f), "{}", self.0)
    }
}

fn double_encode_byte(b: u8) -> &'static str {
    const ENCODE: &[u8; 0x100*5] = b"\
        %2500%2501%2502%2503%2504%2505%2506%2507\
        %2508%2509%250A%250B%250C%250D%250E%250F\
        %2510%2511%2512%2513%2514%2515%2516%2517\
        %2518%2519%251A%251B%251C%251D%251E%251F\
        %2520%2521%2522%2523%2524%2525%2526%2527\
        %2528%2529%252A%252B%252C%252D%252E%252F\
        %2530%2531%2532%2533%2534%2535%2536%2537\
        %2538%2539%253A%253B%253C%253D%253E%253F\
        %2540%2541%2542%2543%2544%2545%2546%2547\
        %2548%2549%254A%254B%254C%254D%254E%254F\
        %2550%2551%2552%2553%2554%2555%2556%2557\
        %2558%2559%255A%255B%255C%255D%255E%255F\
        %2560%2561%2562%2563%2564%2565%2566%2567\
        %2568%2569%256A%256B%256C%256D%256E%256F\
        %2570%2571%2572%2573%2574%2575%2576%2577\
        %2578%2579%257A%257B%257C%257D%257E%257F\
        %2580%2581%2582%2583%2584%2585%2586%2587\
        %2588%2589%258A%258B%258C%258D%258E%258F\
        %2590%2591%2592%2593%2594%2595%2596%2597\
        %2598%2599%259A%259B%259C%259D%259E%259F\
        %25A0%25A1%25A2%25A3%25A4%25A5%25A6%25A7\
        %25A8%25A9%25AA%25AB%25AC%25AD%25AE%25AF\
        %25B0%25B1%25B2%25B3%25B4%25B5%25B6%25B7\
        %25B8%25B9%25BA%25BB%25BC%25BD%25BE%25BF\
        %25C0%25C1%25C2%25C3%25C4%25C5%25C6%25C7\
        %25C8%25C9%25CA%25CB%25CC%25CD%25CE%25CF\
        %25D0%25D1%25D2%25D3%25D4%25D5%25D6%25D7\
        %25D8%25D9%25DA%25DB%25DC%25DD%25DE%25DF\
        %25E0%25E1%25E2%25E3%25E4%25E5%25E6%25E7\
        %25E8%25E9%25EA%25EB%25EC%25ED%25EE%25EF\
        %25F0%25F1%25F2%25F3%25F4%25F5%25F6%25F7\
        %25F8%25F9%25FA%25FB%25FC%25FD%25FE%25FF\
    ";
    let b = usize::from(b);
    unsafe { str::from_utf8_unchecked(&ENCODE[b*5..(b+1)*5]) }
}

impl<M: Mac> Write for MacWrite<M> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.input(s.as_bytes());
        Ok(())
    }
}

impl EncodeSet_ for EncodeSet {
    fn contains(&self, b: u8) -> bool {
        const ENCODE_MAP: [bool; 0x100] = [
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true, false, false,  true,
            false, false, false, false, false, false, false, false,
            false, false,  true,  true,  true,  true,  true,  true,
             true, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
            false, false, false,  true,  true,  true,  true, false,
             true, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
            false, false, false,  true,  true,  true, false,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
             true,  true,  true,  true,  true,  true,  true,  true,
        ];

        ENCODE_MAP[usize::from(b)]
    }
}

fn percent_encode(input: &str) -> PercentEncode_<EncodeSet> {
    ::percent_encoding::utf8_percent_encode(input, EncodeSet)
}

#[cfg(test)]
mod tests {
    extern crate base64;

    use percent_encoding::percent_encode_byte;

    use super::*;

    // These values are taken from Twitter's document:
    // https://developer.twitter.com/en/docs/basics/authentication/guides/creating-a-signature.html
    const CK: &str = "xvz1evFS4wEEPTGEFPHBog";
    const CS: &str = "kAcSOqF21Fu85e7zjz7ZN2U4ZRhfV3WpwPAoE3Z7kBw";
    const AK: &str = "370773112-GmHxMAgYyLbNEtIKZeRNFsMKPR9EyMZeS9weJAEb";
    const AS: &str = "LswwdoUaIvS8ltyTt5jkRh4J50vUPVVHtR2YPi5kE";
    const NONCE: &str = "kYjzVBB8Y0ZFabxSWbWovY3uYSQ2pTgmZeNu2VS4cg";
    const TIMESTAMP: u64 = 1318622958;

    #[test]
    fn base64_percent_encode() {
        macro_rules! test {
            ($bin:expr) => {
                assert_eq!(
                    percent_encode(&base64::encode($bin))
                        .to_string(),
                    Base64PercentEncode($bin).to_string(),
                )
            };
        }
        test!(b"\x84+R\x99\x88~\x88v\x02\x12\xA0V\xACN\xC2\xEE\x16&\xB5I");
        test!(b"\x00\x10\xB1\xCB=5\xDB\xEF\xBF_/\x7F2~~M\xFD>\xFF~");
    }

    #[test]
    fn double_percent_encode() {
        for b in 0u8..=0xFF {
            assert_eq!(
                double_encode_byte(b),
                &percent_encode(percent_encode_byte(b))
                    .to_string(),
            );
        }
    }

    #[test]
    fn encode_set() {
        for b in 0u8..=0xFF {
            let expected = match b {
                b'0'...b'9'
                    | b'A'...b'Z'
                    | b'a'...b'z'
                    | b'-' | b'.' | b'_' | b'~' => false,
                _ => true,
            };
            assert_eq!(EncodeSet.contains(b), expected,
                "byte = {} ({:?})", b, char::from(b)
            );
        }
    }

    #[test]
    fn query_builder() {
        macro_rules! test {
            ($((
                $method:expr, $ep:expr,
                $ck:expr, $ak:expr, $cs:expr, $as_:expr,
                $nonce:expr, $timestamp:expr,
                { $($query1:tt)* }, { $($query2:tt)* } $(,)*
            ) -> ($expected_sign:expr, $expected_query:expr $(,)*);)*) => {$(
                let ep = Uri::from_static($ep);
                let mut qb = if $method == "POST" {
                    QueryBuilder::new_form($cs, $as_, $method, &ep)
                } else {
                    QueryBuilder::new($cs, $as_, $method, &ep)
                };

                test_inner! { qb; $($query1)* }
                qb.append_oauth_params_($ck, $ak, $nonce, $timestamp);
                test_inner! { qb; $($query2)* }

                let QueryOutcome { header, query } = qb.build();
                assert_eq!(
                    header,
                    format!("\
                        OAuth \
                        oauth_consumer_key=\"{}\",\
                        oauth_nonce=\"{}\",\
                        oauth_signature_method=\"HMAC-SHA1\",\
                        oauth_timestamp=\"{}\",\
                        oauth_token=\"{}\",\
                        oauth_version=\"1.0\",\
                        oauth_signature=\"{}\"\
                    ", $ck, $nonce, $timestamp, $ak, $expected_sign)
                );
                assert_eq!(query, $expected_query);
            )*};
        }

        macro_rules! test_inner {
            ($qb:ident; encoded $key:ident: $v:expr, $($rest:tt)*) => {
                $qb.append_encoded(stringify!($key), $v);
                test_inner! { $qb; $($rest)* }
            };
            ($qb:ident; $key:ident: $v:expr, $($rest:tt)*) => {
                $qb.append(stringify!($key), $v);
                test_inner! { $qb; $($rest)* }
            };
            ($_qb:ident;) => ();
        }

        test! {
            (
                "GET", "https://stream.twitter.com/1.1/statuses/sample.json",
                CK, AK, CS, AS, NONCE, TIMESTAMP,
                {}, { encoded stall_warnings: "true", },
            ) -> (
                "OGQqcy4l5xWBFX7t0DrkP5%2FD0rM%3D",
                "https://stream.twitter.com/1.1/statuses/sample.json?stall_warnings=true",
            );
            (
                "POST", "https://api.twitter.com/1.1/statuses/update.json",
                CK, AK, CS, AS, NONCE, TIMESTAMP,
                { encoded include_entities: "true", },
                { status: "Hello Ladies + Gentlemen, a signed OAuth request!", },
            ) -> (
                "hCtSmYh%2BiHYCEqBWrE7C7hYmtUk%3D",
                "include_entities=true&\
                    status=Hello%20Ladies%20%2B%20Gentlemen%2C%20a%20signed%20OAuth%20request%21",
            );
            ("POST", "https://example.com/post.json", CK, AK, CS, AS, NONCE, TIMESTAMP, {}, {})
                -> ("pN52L1gJ6sOyYOyv23cwfWFsIZc%3D", "");
            (
                "GET", "https://example.com/get.json",
                CK, AK, CS, AS, NONCE, TIMESTAMP,
                { encoded bar: "%E9%85%92%E5%A0%B4", foo: "ふー", }, {},
            ) -> (
                "Xp35hf3T21yhpEuxez7p6bV62Bw%3D",
                "https://example.com/get.json?bar=%E9%85%92%E5%A0%B4&foo=%E3%81%B5%E3%83%BC",
            );
        }
    }
}
