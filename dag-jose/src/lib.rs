mod errors;
mod tests;
mod traits;

pub use errors::Error;

use cid::Cid;

use linked_data::types::IPLDLink;

use multibase::Base;

use serde::{Deserialize, Serialize};

pub use traits::{AsyncBlockSigner, BlockSigner};

use signature::SignatureEncoding;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AlgorithmType {
    //https://www.rfc-editor.org/rfc/rfc7518.html#section-3.1
    #[serde(rename = "ES256")]
    ES256,

    //https://datatracker.ietf.org/doc/html/draft-ietf-cose-webauthn-algorithms-04#section-3.2
    #[serde(rename = "ES256K")]
    ES256K,

    //https://datatracker.ietf.org/doc/html/draft-ietf-jose-cfrg-curves#section-3.1
    #[serde(rename = "EdDSA")]
    EdDSA,

    //https://identity.foundation/EcdsaSecp256k1RecoverySignature2020/#ES256K-R
    #[serde(rename = "ES256K-R")]
    ES256KR,
}

// https://datatracker.ietf.org/doc/html/rfc7518#section-6.1
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

    // https://datatracker.ietf.org/doc/html/rfc7518#section-6.2.1.1
    #[serde(rename = "P-256")]
    P256,
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
    pub key_type: KeyType,

    #[serde(rename = "crv")]
    pub curve: CurveType,

    pub x: String, // https://datatracker.ietf.org/doc/html/rfc7518#section-6.2.1.2

    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<String>, // https://datatracker.ietf.org/doc/html/rfc7518#section-6.2.1.3
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Header {
    #[serde(rename = "alg", skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<AlgorithmType>,

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
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JsonWebSignature {
    payload: String,

    signatures: Vec<Signature>,

    #[serde(skip_serializing)]
    link: IPLDLink,
}

impl JsonWebSignature {
    pub fn get_link(&self) -> Result<Cid, Error> {
        let data = Base::Base64Url.decode(&self.payload)?;
        let cid = Cid::read_bytes(&*data)?;
        Ok(cid)
    }

    /// Returns the input data used when signing.
    pub fn get_signature_inputs(&self) -> String {
        format!("{}.{}", self.payload, self.signatures[0].protected)
    }

    pub fn get_header(&self) -> Result<Header, Error> {
        let mut header = Header {
            algorithm: None,
            json_web_key: None,
        };

        if !self.signatures[0].protected.is_empty() {
            let data = Base::Base64Url.decode(&self.signatures[0].protected)?;
            let protected: Header = serde_json::from_slice(&data)?;

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

        Ok(header)
    }

    /// Verify a dag-jose block.
    pub fn verify(&self) -> Result<(), Error> {
        use signature::Verifier;

        let header = self.get_header()?;

        let (algo, jwk) = match (header.algorithm, header.json_web_key) {
            (Some(algo), Some(jwk)) => (algo, jwk),
            _ => return Err(Error::Header),
        };

        let signing_input = self.get_signature_inputs();

        let signature = Base::Base64Url.decode(&self.signatures[0].signature)?;

        match (algo, &jwk.key_type, &jwk.curve) {
            (AlgorithmType::ES256, KeyType::EllipticCurve, CurveType::P256) => {
                let Some(y) = jwk.y else {
                    return Err(Error::Key);
                };
                
                let mut public_key = vec![0x04]; // Uncompressed key prefix
                public_key.extend(Base::Base64Url.decode(jwk.x)?);
                public_key.extend(Base::Base64Url.decode(y)?);

                let verif_key = p256::ecdsa::VerifyingKey::try_from(public_key.as_slice())?;
                let signature = p256::ecdsa::Signature::try_from(signature.as_slice())?;

                verif_key.verify(signing_input.as_bytes(), &signature)?;
            }
            (AlgorithmType::ES256K, KeyType::EllipticCurve, CurveType::Secp256k1) => {
                let Some(y) = jwk.y else {
                    return Err(Error::Key);
                };
                
                let mut public_key = vec![0x04]; // Uncompressed key prefix
                public_key.extend(Base::Base64Url.decode(jwk.x)?);
                public_key.extend(Base::Base64Url.decode(y)?);

                let verif_key = k256::ecdsa::VerifyingKey::try_from(public_key.as_slice())?;
                let signature = k256::ecdsa::Signature::try_from(signature.as_slice())?;

                verif_key.verify(signing_input.as_bytes(), &signature)?;
            }
            (AlgorithmType::EdDSA, KeyType::OctetString, CurveType::Ed25519) => {
                let public_key = Base::Base64Url.decode(jwk.x)?;

                let public_key = ed25519_dalek::VerifyingKey::try_from(public_key.as_slice())?;
                let signature = ed25519::Signature::try_from(signature.as_slice())?;

                public_key.verify(signing_input.as_bytes(), &signature)?;
            }
            _ => unimplemented!(),
        }

        Ok(())
    }

    pub fn new<S, U>(cid: Cid, signer: S) -> Result<Self, Error>
    where
        S: BlockSigner<U>,
        U: SignatureEncoding,
    {
        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(signer.algorithm()),
            json_web_key: None,
        };

        let protected = serde_json::to_vec(&protected)?;
        let protected = Base::Base64Url.encode(protected);

        let message = format!("{}.{}", payload, protected);

        let signature = signer.try_sign(message.as_bytes())?;

        let jwk = signer.web_key();

        let header = Some(Header {
            algorithm: None,
            json_web_key: Some(jwk),
        });

        let signature = Base::Base64Url.encode(signature.to_bytes());

        let jws = Self {
            payload,
            signatures: vec![Signature {
                header,
                protected,
                signature,
            }],
            link: IPLDLink::default(), // Skipped when serializing anyway
        };

        Ok(jws)
    }

    pub async fn new_async<S, U>(cid: Cid, signer: S) -> Result<Self, Error>
    where
        S: AsyncBlockSigner<U>,
        U: SignatureEncoding + Send + 'static,
    {
        //TODO code reuse

        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(signer.algorithm().await),
            json_web_key: None,
        };

        let protected = serde_json::to_vec(&protected)?;
        let protected = Base::Base64Url.encode(protected);

        let message = format!("{}.{}", payload, protected);

        let signature = signer.sign_async(message.as_bytes()).await?;

        let jwk = signer.web_key().await;

        let header = Some(Header {
            algorithm: None,
            json_web_key: Some(jwk),
        });

        let signature = Base::Base64Url.encode(signature.to_bytes());

        let jws = Self {
            payload,
            signatures: vec![Signature {
                header,
                protected,
                signature,
            }],
            link: IPLDLink::default(), // Skipped when serializing anyway
        };

        Ok(jws)
    }

    //TODO add a new feature "web" for the logic below

    pub fn signing_input(cid: Cid, algorithm: AlgorithmType) -> Result<String, Error> {
        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(algorithm),
            json_web_key: None,
        };

        let protected = serde_json::to_vec(&protected)?;
        let protected = Base::Base64Url.encode(protected);

        let message = format!("{}.{}", payload, protected);

        Ok(message)
    }

    pub fn from_parts(
        cid: Cid,
        algorithm: AlgorithmType,
        jwk: JsonWebKey,
        signature: impl SignatureEncoding,
    ) -> Result<Self, Error> {
        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(algorithm),
            json_web_key: None,
        };

        let protected = serde_json::to_vec(&protected)?;
        let protected = Base::Base64Url.encode(protected);

        let header = Some(Header {
            algorithm: None,
            json_web_key: Some(jwk),
        });

        let signature = Base::Base64Url.encode(signature.to_bytes());

        let jws = Self {
            payload,
            signatures: vec![Signature {
                header,
                protected,
                signature,
            }],
            link: IPLDLink::default(), // Skipped when serializing anyway
        };

        Ok(jws)
    }
}
