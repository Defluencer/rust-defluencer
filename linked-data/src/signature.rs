use crate::types::IPLDLink;

use serde::{Deserialize, Serialize};

// https://ipld.io/specs/codecs/dag-jose/fixtures/
// https://ipld.io/specs/codecs/dag-jose/spec/
// https://www.rfc-editor.org/rfc/rfc7515
// https://www.rfc-editor.org/rfc/rfc7517
// https://www.rfc-editor.org/rfc/rfc7518
// https://www.iana.org/assignments/jose/jose.xhtml

/// Raw Json Web Signature
///
/// Don't forget to specify --input-codec="dag-json" and --output-codec="dag-jose"
/// when adding to IPFS.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RawJWS {
    pub payload: String,

    pub signatures: Vec<RawSignature>,

    #[serde(skip_serializing)]
    pub link: IPLDLink,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RawSignature {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<Header>,

    /// Default empty string
    pub protected: String,

    pub signature: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Header {
    #[serde(rename = "alg", skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<AlgorithmType>, // https://www.rfc-editor.org/rfc/rfc7515#section-4.1.1

    #[serde(rename = "jwk", skip_serializing_if = "Option::is_none")]
    pub json_web_key: Option<JsonWebKey>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AlgorithmType {
    #[serde(rename = "ES256K")]
    ES256K,

    #[serde(rename = "EdDSA")]
    EdDSA,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JsonWebKey {
    /*
    #[serde(rename = "use")]
    pub public_key_use: Option<String>, // https://datatracker.ietf.org/doc/html/rfc7517#section-4.2
    #[serde(rename = "key_ops")]
    pub key_operation: Option<String>, // https://datatracker.ietf.org/doc/html/rfc7517#section-4.3
    #[serde(rename = "alg")]
    pub algorithm: Option<String>, // https://datatracker.ietf.org/doc/html/rfc7517#section-4.4
    #[serde(rename = "kid")]
    pub key_id: Option<String>, // https://datatracker.ietf.org/doc/html/rfc7517#section-4.5
    */
    /*
        Parameter specific to EC
    */
    #[serde(rename = "kty")]
    pub key_type: KeyType, // https://datatracker.ietf.org/doc/html/rfc7518#section-6.1

    #[serde(rename = "crv")]
    pub curve: CurveType, // https://datatracker.ietf.org/doc/html/rfc7518#section-6.2.1.1

    pub x: String, // https://datatracker.ietf.org/doc/html/rfc7518#section-6.2.1.2

    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<String>, // https://datatracker.ietf.org/doc/html/rfc7518#section-6.2.1.3
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum KeyType {
    #[serde(rename = "EC")]
    EllipticCurve,

    #[serde(rename = "RSA")]
    RSA,

    #[serde(rename = "oct")]
    OctetSequence,

    #[serde(rename = "OKP")]
    OctetString,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum CurveType {
    #[serde(rename = "Ed25519")]
    Ed25519,

    #[serde(rename = "secp256k1")]
    Secp256k1,
}
