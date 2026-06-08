//! JSON serialization for `BTreeMap<Input, Output>`.
//!
//! Serializes as a JSON array of `[input, output]` pairs in BTreeMap order
//! (sorted by transaction_id then output_index).
//!
//! ```json
//! [
//!   [
//!     { "transaction_id": "<hex>", "output_index": 1 },
//!     {
//!       "address": "<bech32>",
//!       "value": {
//!         "lovelace": 9883179343,
//!         "assets": { "<policy_hex>": { "<asset_name_hex>": 100 } }
//!       },
//!       "datum":  { "kind": "null" }
//!               | { "kind": "hash",   "hash":  "<hex>" }
//!               | { "kind": "inline", "bytes": "<hex>" },
//!       "script": { "version": "PlutusV3", "hash": "<hex>", "cbor": "<hex>" }
//!     }
//!   ],
//!   ...
//! ]
//! ```
//!
//! # Cargo features required
//!
//! ```toml
//! serde      = { version = "1", features = ["derive"] }
//! serde_json = "1"
//! serde_with = { version = "3", features = ["hex", "macros"] }
//! ```

use std::collections::BTreeMap;
use std::io;

use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};

use cardano_sdk::{Address, Hash, PlutusScript, PlutusVersion, address::kind, cbor::ToCbor};

// ─────────────────────────────────────────────────────────────── public API ──

/// Serialize a `BTreeMap<Input, Output>` to a pretty-printed JSON string.
pub fn to_json(
    utxos: &BTreeMap<cardano_sdk::Input, cardano_sdk::Output>,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&as_pairs(utxos))
}

/// Like [`to_json`], but any string value longer than `max_chars` is truncated
/// to `max_chars` characters and suffixed with `…`. Useful for suppressing
/// large script `cbor` fields in terminal output.
pub fn to_json_truncated(
    utxos: &BTreeMap<cardano_sdk::Input, cardano_sdk::Output>,
    max_chars: usize,
) -> Result<String, serde_json::Error> {
    let mut buf = Vec::new();
    let mut ser =
        serde_json::Serializer::with_formatter(&mut buf, TruncatingFormatter::new(max_chars));
    as_pairs(utxos).serialize(&mut ser)?;
    // serde_json always emits valid UTF-8
    Ok(String::from_utf8(buf).expect("valid utf-8"))
}

fn as_pairs(utxos: &BTreeMap<cardano_sdk::Input, cardano_sdk::Output>) -> Vec<(Input, Output)> {
    utxos
        .iter()
        .map(|(k, v)| (Input::from(k), Output::from(v)))
        .collect()
}

// ──────────────────────────────────────── TruncatingFormatter ────────────────
//
// Wraps PrettyFormatter, intercepting string fragments and cutting them at
// `max_chars` characters. The `…` suffix is written in `end_string` so it
// lands outside serde_json's escape layer (giving `"deadbeef…"` not `"\u2026"`).
//
// WARNING :: This also truncates keys as well as values!

pub struct TruncatingFormatter<'a> {
    inner: serde_json::ser::PrettyFormatter<'a>,
    max_chars: usize,
    written: usize, // chars written for the current string value
    truncated: bool,
}

impl<'a> TruncatingFormatter<'a> {
    pub fn new(max_chars: usize) -> Self {
        Self {
            inner: serde_json::ser::PrettyFormatter::new(),
            max_chars,
            written: 0,
            truncated: false,
        }
    }
}

impl serde_json::ser::Formatter for TruncatingFormatter<'_> {
    fn begin_string<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.written = 0;
        self.truncated = false;
        self.inner.begin_string(w)
    }

    fn end_string<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        if self.truncated {
            w.write_all("…".as_bytes())?;
        }
        self.inner.end_string(w)
    }

    fn write_string_fragment<W: io::Write + ?Sized>(
        &mut self,
        w: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        if self.truncated {
            return Ok(());
        }
        let remaining = self.max_chars.saturating_sub(self.written);
        if fragment.chars().count() <= remaining {
            self.written += fragment.chars().count();
            w.write_all(fragment.as_bytes())
        } else {
            let cut = fragment
                .char_indices()
                .nth(remaining)
                .map(|(i, _)| i)
                .unwrap_or(fragment.len());
            w.write_all(fragment[..cut].as_bytes())?;
            self.truncated = true;
            Ok(())
        }
    }

    // ── delegate layout methods to inner ──────────────────────────────────────
    fn begin_array<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_array(w)
    }
    fn end_array<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_array(w)
    }
    fn begin_array_value<W: io::Write + ?Sized>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.inner.begin_array_value(w, first)
    }
    fn end_array_value<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_array_value(w)
    }
    fn begin_object<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_object(w)
    }
    fn end_object<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_object(w)
    }
    fn begin_object_key<W: io::Write + ?Sized>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.inner.begin_object_key(w, first)
    }
    fn end_object_key<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_object_key(w)
    }
    fn begin_object_value<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.begin_object_value(w)
    }
    fn end_object_value<W: io::Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
        self.inner.end_object_value(w)
    }
}

// ───────────────────────────────────────────────── intermediate types ─────────

#[serde_as]
#[derive(Serialize)]
struct Input {
    #[serde_as(as = "serde_with::hex::Hex")]
    transaction_id: Hash<32>,
    output_index: u64,
}

impl From<&cardano_sdk::Input> for Input {
    fn from(input: &cardano_sdk::Input) -> Self {
        Self {
            transaction_id: input.transaction_id(),
            output_index: input.output_index(),
        }
    }
}

#[serde_as]
#[derive(Serialize)]
struct Output {
    #[serde_as(as = "DisplayFromStr")]
    address: Address<kind::Any>,
    value: Value,
    datum: Datum,
    #[serde(skip_serializing_if = "Option::is_none")]
    script: Option<Script>,
}

impl From<&cardano_sdk::Output> for Output {
    fn from(output: &cardano_sdk::Output) -> Self {
        Self {
            address: output.address().clone(),
            value: Value::from(output.value()),
            datum: Datum::from(output.datum()),
            script: output.script().map(Script::from),
        }
    }
}

#[serde_as]
#[derive(Serialize)]
struct Value {
    lovelace: u64,
    #[serde_as(as = "BTreeMap<serde_with::hex::Hex, BTreeMap<serde_with::hex::Hex, _>>")]
    assets: BTreeMap<Hash<28>, BTreeMap<Vec<u8>, u64>>,
}

impl From<&cardano_sdk::Value<u64>> for Value {
    fn from(value: &cardano_sdk::Value<u64>) -> Self {
        let mut assets: BTreeMap<Hash<28>, BTreeMap<Vec<u8>, u64>> = BTreeMap::new();
        for (policy, asset_name_qty) in value.assets() {
            for (asset_name, qty) in asset_name_qty {
                assets
                    .entry(policy.clone())
                    .or_default()
                    .insert(asset_name.clone(), *qty);
            }
        }
        Self {
            lovelace: value.lovelace(),
            assets,
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Datum {
    Null,
    Hash {
        #[serde_as(as = "serde_with::hex::Hex")]
        hash: Hash<32>,
    },
    Inline {
        #[serde_as(as = "serde_with::hex::Hex")]
        cbor: Vec<u8>,
    },
}

impl From<Option<&cardano_sdk::Datum>> for Datum {
    fn from(datum: Option<&cardano_sdk::Datum>) -> Self {
        match datum {
            None => Self::Null,
            Some(cardano_sdk::Datum::Hash(h)) => Self::Hash { hash: *h },
            Some(cardano_sdk::Datum::Inline(d)) => Self::Inline { cbor: d.to_cbor() },
        }
    }
}

#[serde_as]
#[derive(Serialize)]
struct Script {
    version: &'static str,
    #[serde_as(as = "serde_with::hex::Hex")]
    hash: Hash<28>,
    #[serde_as(as = "serde_with::hex::Hex")]
    cbor: Vec<u8>,
}

impl From<&PlutusScript> for Script {
    fn from(script: &PlutusScript) -> Self {
        Self {
            version: match script.version() {
                PlutusVersion::V1 => "PlutusV1",
                PlutusVersion::V2 => "PlutusV2",
                PlutusVersion::V3 => "PlutusV3",
            },
            hash: Hash::<28>::from(script),
            cbor: script.script().to_vec(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────── tests ───

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(byte: u8, index: u64) -> cardano_sdk::Input {
        cardano_sdk::Input::new(Hash::from([byte; 32]), index)
    }

    fn make_output(lovelace: u64) -> cardano_sdk::Output {
        let address = "addr_test1vr8hlhudn0z8hkmm9fpel3rxqhasjv8djm5v6fve9rxl8wc5ruleq"
            .parse::<Address<kind::Any>>()
            .unwrap();
        cardano_sdk::Output::new(address, cardano_sdk::Value::new(lovelace))
    }

    #[test]
    fn input_fields() {
        let v = serde_json::to_value(Input::from(&make_input(0xab, 3))).unwrap();
        assert_eq!(v["transaction_id"], "ab".repeat(32));
        assert_eq!(v["output_index"], 3u64);
    }

    #[test]
    fn array_of_pairs_shape() {
        let mut utxos = BTreeMap::new();
        utxos.insert(make_input(0x01, 0), make_output(5_000_000));

        let v: serde_json::Value = serde_json::from_str(&to_json(&utxos).unwrap()).unwrap();
        let pair = &v.as_array().unwrap()[0].as_array().unwrap();

        assert!(pair[0]["transaction_id"].is_string());
        assert!(pair[0]["output_index"].is_number());
        assert!(pair[1]["address"].is_string());
        assert_eq!(pair[1]["value"]["lovelace"], 5_000_000u64);
        assert!(pair[1]["value"]["assets"].as_object().unwrap().is_empty());
        assert_eq!(pair[1]["datum"]["kind"], "null");
        assert!(pair[1].get("script").is_none());
    }

    #[test]
    fn pair_order_matches_btreemap() {
        let mut utxos = BTreeMap::new();
        utxos.insert(make_input(0xff, 0), make_output(1));
        utxos.insert(make_input(0x00, 0), make_output(2));

        let v: serde_json::Value = serde_json::from_str(&to_json(&utxos).unwrap()).unwrap();
        let arr = v.as_array().unwrap();
        assert!(arr[0][0]["transaction_id"].as_str() < arr[1][0]["transaction_id"].as_str());
    }

    #[test]
    fn truncation_cuts_long_strings() {
        let mut utxos = BTreeMap::new();
        utxos.insert(make_input(0x01, 0), make_output(5_000_000));

        let v: serde_json::Value =
            serde_json::from_str(&to_json_truncated(&utxos, 16).unwrap()).unwrap();
        // transaction_id is 64 hex chars; truncated to 8 + …
        eprintln!("{}", v);
        let txid = v[0][0]["transaction_id"].as_str().unwrap();
        assert!(txid.ends_with('…'));
        assert_eq!(txid.chars().count(), 17); // 8 + ellipsis
    }
}
