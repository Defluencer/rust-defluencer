mod errors;
mod tests;

pub use errors::Error;

use cid::Cid;

use multibase::Base;

use signatory::{ecdsa::Secp256k1Signer, ed25519::Ed25519Signer};

use serde::{Deserialize, Serialize};

// https://ipld.io/specs/codecs/dag-jose/fixtures/
// https://ipld.io/specs/codecs/dag-jose/spec/
// https://www.rfc-editor.org/rfc/rfc7515
// https://www.rfc-editor.org/rfc/rfc7517
// https://www.rfc-editor.org/rfc/rfc7518
// https://www.iana.org/assignments/jose/jose.xhtml

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AlgorithmType {
    #[serde(rename = "ES256K")]
    ES256K,

    #[serde(rename = "EdDSA")]
    EdDSA,
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
pub struct Header {
    #[serde(rename = "alg", skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<AlgorithmType>, // https://www.rfc-editor.org/rfc/rfc7515#section-4.1.1

    #[serde(rename = "jwk", skip_serializing_if = "Option::is_none")]
    pub json_web_key: Option<JsonWebKey>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Signature {
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<Header>,

    protected: String, // Default empty string

    signature: String,
}

/// Json Web Signature
///
/// Don't forget to specify --input-codec="dag-json" and --output-codec="dag-jose"
/// when adding to IPFS.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JsonWebSignature {
    payload: String,

    signatures: Vec<Signature>,
}

impl JsonWebSignature {
    pub fn get_link(&self) -> Cid {
        let data = Base::Base64Url
            .decode(&self.payload)
            .expect("Base 64 Decoding");

        let cid = Cid::read_bytes(&*data).expect("Valid Cid");

        cid
    }

    /// Returns the input data used when signing.
    pub fn get_signature_inputs(&self) -> String {
        format!("{}.{}", self.payload, self.signatures[0].protected)
    }

    pub fn get_header(&self) -> Header {
        let mut header = Header {
            algorithm: None,
            json_web_key: None,
        };

        if !self.signatures[0].protected.is_empty() {
            let data = Base::Base64Url
                .decode(&self.signatures[0].protected)
                .expect("Base 64 Decoding");
            let protected: Header = serde_json::from_slice(&data).expect("Deserialization");

            header.algorithm = protected.algorithm;
            header.json_web_key = protected.json_web_key;
        }

        if let Some(raw) = &self.signatures[0].header {
            if header.algorithm.is_none() && raw.algorithm.is_some() {
                header.algorithm = raw.algorithm.clone();
            }

            if header.json_web_key.is_none() && raw.json_web_key.is_some() {
                header.json_web_key = raw.json_web_key.clone();
            }
        }

        header
    }

    /// Verify a dag-jose block.
    pub fn verify(&self) -> Result<(), Error> {
        use signature::{Signature, Verifier};

        let header = self.get_header();

        let (algo, jwk) = match (header.algorithm, header.json_web_key) {
            (Some(algo), Some(jwk)) => (algo, jwk),
            _ => return Err(Error::Header),
        };

        let signing_input = self.get_signature_inputs();

        let signature = Base::Base64Url
            .decode(&self.signatures[0].signature)
            .expect("Base 64 Decoding");

        match (algo, &jwk.key_type, &jwk.curve) {
            (AlgorithmType::ES256K, KeyType::EllipticCurve, CurveType::Secp256k1) => {
                let public_key_x = Base::Base64Url.decode(&jwk.x).expect("Base 64 Decoding");
                let public_key_y = Base::Base64Url
                    .decode(&jwk.y.expect("Uncompressed Public Key"))
                    .expect("Base 64 Decoding");
                let public_key = [vec![0x04], public_key_x, public_key_y].concat();

                let verif_key =
                    signatory::ecdsa::secp256k1::VerifyingKey::from_sec1_bytes(&public_key)
                        .expect("Valid Public Key");

                let signature =
                    signatory::ecdsa::Signature::from_bytes(&signature).expect("Valid Signature");

                verif_key.verify(signing_input.as_bytes(), &signature)?;
            }
            (AlgorithmType::EdDSA, KeyType::OctetString, CurveType::Ed25519) => {
                let public_key = Base::Base64Url.decode(&jwk.x).expect("Base 64 Decoding");
                let public_key = signatory::ed25519::VerifyingKey::from_bytes(&public_key)
                    .expect("Valid Public Key");

                let signature =
                    signatory::ed25519::Signature::from_bytes(&signature).expect("Valid Signature");

                public_key.verify(signing_input.as_bytes(), &signature)?;
            }
            _ => return Err(Error::Crypto),
        }

        Ok(())
    }

    /// Return a new block with signed using ed25519
    pub fn new_with_ed25519(cid: Cid, signer: impl Ed25519Signer) -> Result<Self, Error> {
        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(AlgorithmType::EdDSA),
            json_web_key: None,
        };

        let protected = serde_json::to_vec(&protected).expect("Serialization");
        let protected = Base::Base64Url.encode(protected);

        let message = format!("{}.{}", payload, protected);

        let signature = signer.try_sign(message.as_bytes())?;

        let public_key = signer.verifying_key();

        // https://www.rfc-editor.org/rfc/rfc8037.html
        let jwk = JsonWebKey {
            key_type: KeyType::OctetString,
            curve: CurveType::Ed25519,
            x: Base::Base64Url.encode(public_key.to_bytes()),
            y: None,
        };

        let header = Some(Header {
            algorithm: None,
            json_web_key: Some(jwk),
        });

        let signature = Base::Base64Url.encode(signature);

        let jws = Self {
            payload,
            signatures: vec![Signature {
                header,
                protected,
                signature,
            }],
        };

        Ok(jws)
    }

    /// Return a new block with signed using secp256k1
    pub fn new_with_secp256k1(cid: Cid, signer: impl Secp256k1Signer) -> Result<Self, Error> {
        use signatory::ecdsa::elliptic_curve::sec1::ToEncodedPoint;

        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(AlgorithmType::ES256K),
            json_web_key: None,
        };

        let protected = serde_json::to_vec(&protected).expect("Serialization");
        let protected = Base::Base64Url.encode(protected);

        let message = format!("{}.{}", payload, protected);

        let signature = signer.try_sign(message.as_bytes())?;

        let public_key = {
            let verif_key = signer.verifying_key();
            let mut temp = verif_key.to_encoded_point(false).to_bytes().into_vec();
            temp.remove(0); //remove SEC1 tag
            temp
        };

        let jwk = JsonWebKey {
            key_type: KeyType::EllipticCurve,
            curve: CurveType::Secp256k1,
            x: Base::Base64Url.encode(&public_key[0..32]),
            y: Some(Base::Base64Url.encode(&public_key[32..64])),
        };

        let header = Some(Header {
            algorithm: None,
            json_web_key: Some(jwk),
        });

        let signature = Base::Base64Url.encode(signature);

        let jws = Self {
            payload,
            signatures: vec![Signature {
                header,
                protected,
                signature,
            }],
        };

        Ok(jws)
    }
}
