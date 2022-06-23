use cid::Cid;

use linked_data::{
    signature::{AlgorithmType, CurveType, Header, KeyType, RawJWS, RawSignature},
    types::Address,
};

use multibase::Base;

use sha3::Keccak256;

use signature::Verifier;

use crate::errors::Error;

#[derive(Debug, Default)]
pub struct JsonWebSignature {
    signing_input: String,
    signatures: Vec<Signature>,
    pub link: Cid,
}

#[derive(Debug)]
struct Signature {
    pub header: Header,
    pub signature: Vec<u8>,
}

impl TryFrom<RawJWS> for JsonWebSignature {
    type Error = Error;

    fn try_from(raw: RawJWS) -> Result<Self, Error> {
        let signing_input = format!("{}.{}", raw.payload, raw.signatures[0].protected);

        let payload = Base::Base64Url.decode(raw.payload)?;
        let link = Cid::try_from(payload)?;

        let mut signatures = Vec::with_capacity(raw.signatures.len());

        for raw in raw.signatures {
            let signature: Signature = match raw.try_into() {
                Ok(sig) => sig,
                Err(e) => return Err(e),
            };

            signatures.push(signature);
        }

        Ok(Self {
            signing_input,
            signatures,
            link,
        })
    }
}

impl TryFrom<RawSignature> for Signature {
    type Error = Error;

    fn try_from(raw: RawSignature) -> Result<Self, Self::Error> {
        let mut header = Header {
            algorithm: None,
            json_web_key: None,
        };

        if !raw.protected.is_empty() {
            let data = Base::Base64Url.decode(raw.protected)?;
            let protected: Header = serde_json::from_slice(&data)?;

            header.algorithm = protected.algorithm;
            header.json_web_key = protected.json_web_key;
        }

        if let Some(raw) = raw.header {
            if header.algorithm.is_none() && raw.algorithm.is_some() {
                header.algorithm = raw.algorithm;
            }

            if header.json_web_key.is_none() && raw.json_web_key.is_some() {
                header.json_web_key = raw.json_web_key;
            }
        }

        let signature = Base::Base64Url.decode(raw.signature)?;

        Ok(Self { header, signature })
    }
}

impl JsonWebSignature {
    /// Verify a dag-jose block.
    pub fn verify(&self) -> Result<(), Error> {
        use signature::Signature;

        let algo = match &self.signatures[0].header.algorithm {
            Some(algo) => algo,
            None => return Err(Error::Jose),
        };

        let jwk = match &self.signatures[0].header.json_web_key {
            Some(key) => key,
            None => return Err(Error::Jose),
        };

        match (algo, &jwk.key_type, &jwk.curve) {
            (AlgorithmType::ES256K, KeyType::EllipticCurve, CurveType::Secp256k1) => {
                // Lazy Hack: Deserialize then serialize as the other type
                let jwk_string = serde_json::to_string(&jwk)?;
                let public_key = elliptic_curve::PublicKey::from_jwk_str(&jwk_string)?;

                let verif_key = k256::ecdsa::VerifyingKey::from(public_key);

                let signature = k256::ecdsa::Signature::from_bytes(&self.signatures[0].signature)?;

                verif_key.verify(self.signing_input.as_bytes(), &signature)?;
            }
            /* (AlgorithmType::EdDSA, KeyType::OctetString, CurveType::Ed25519) => {
                let public_key = Base::Base64Url.decode(&jwk.x)?;
                let public_key = ed25519_dalek::PublicKey::from_bytes(&public_key)?;

                let signature =
                    ed25519_dalek::Signature::from_bytes(&self.signatures[0].signature)?;

                public_key.verify(self.signing_input.as_bytes(), &signature)?;
            } */
            _ => return Err(Error::Jose),
        }

        Ok(())
    }

    pub fn get_eth_address(&self) -> Option<Address> {
        use sha3::Digest;

        let jwk = self.signatures[0].header.json_web_key.as_ref()?;

        if jwk.curve != CurveType::Secp256k1 {
            return None;
        }

        let mut hasher = Keccak256::new();

        hasher.update(jwk.x.as_bytes());

        if let Some(y) = jwk.y.as_ref() {
            hasher.update(y.as_bytes());
        }

        let gen_array = hasher.finalize();

        let mut address = [0u8; 20];
        for (i, byte) in gen_array.into_iter().skip(12).enumerate() {
            address[i] = byte;
        }

        Some(address)
    }
}
