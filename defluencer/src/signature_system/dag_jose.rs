use cid::Cid;

use linked_data::signature::{AlgorithmType, CurveType, Header, KeyType, RawJWS, RawSignature};

use multibase::Base;

use signature::Verifier;

use crate::errors::Error;

/// Verify a dag-jose block.
pub fn verify_jws(raw_jws: RawJWS) -> Result<(), Error> {
    use signature::Signature;

    let message = format!("{}.{}", raw_jws.payload, raw_jws.signatures[0].protected);

    let jws: JsonWebSignature = raw_jws.try_into()?;

    let algo = match &jws.signatures[0].header.algorithm {
        Some(algo) => algo,
        None => return Err(Error::Jose),
    };

    let jwk = match &jws.signatures[0].header.json_web_key {
        Some(key) => key,
        None => return Err(Error::Jose),
    };

    match (algo, &jwk.key_type, &jwk.curve) {
        (AlgorithmType::ES256K, KeyType::EllipticCurve, CurveType::Secp256k1) => {
            // Lazy Hack: Deserialize then serialize as the other type
            let jwk_string = serde_json::to_string(jwk)?;
            let public_key = elliptic_curve::PublicKey::from_jwk_str(&jwk_string)?;

            let verif_key = k256::ecdsa::VerifyingKey::from(public_key);

            let signature = k256::ecdsa::Signature::from_bytes(&jws.signatures[0].signature)?;

            verif_key.verify(message.as_bytes(), &signature)?;
        }
        (AlgorithmType::EdDSA, KeyType::OctetString, CurveType::Ed25519) => {
            let public_key = Base::Base64Url.decode(&jwk.x)?;
            let public_key = ed25519_dalek::PublicKey::from_bytes(&public_key)?;

            let signature = ed25519_dalek::Signature::from_bytes(&jws.signatures[0].signature)?;

            public_key.verify(message.as_bytes(), &signature)?;
        }
        _ => return Err(Error::Jose),
    }

    Ok(())
}

#[derive(Debug)]
pub struct Signature {
    pub header: Header,
    pub signature: Vec<u8>,
}

#[derive(Debug)]
pub struct JsonWebSignature {
    pub payload: Cid,
    pub signatures: Vec<Signature>,
    pub link: Cid,
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

impl TryFrom<RawJWS> for JsonWebSignature {
    type Error = Error;

    fn try_from(raw: RawJWS) -> Result<Self, Error> {
        let payload = Base::Base64Url.decode(raw.payload)?;
        let payload = Cid::try_from(payload)?;

        let mut signatures = Vec::with_capacity(raw.signatures.len());

        for raw in raw.signatures {
            let signature: Signature = match raw.try_into() {
                Ok(sig) => sig,
                Err(e) => return Err(e),
            };

            signatures.push(signature);
        }

        let link = raw.link.link;

        Ok(Self {
            payload,
            signatures,
            link,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::signature_system::{
        dag_jose::{JsonWebSignature, Signature},
        IPNSSignature, SignatureSystem,
    };

    use super::*;

    use ipfs_api::IpfsService;
    use linked_data::signature::{AlgorithmType, CurveType, JsonWebKey, KeyType};
    use rand_core::OsRng;

    use ed25519_dalek::Keypair;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ed25519_roundtrip() {
        let mut csprng = OsRng::default();
        let keypair: Keypair = Keypair::generate(&mut csprng);
        let ori_pub_key = keypair.public.clone();

        let ipfs = IpfsService::default();

        let system = IPNSSignature::new(ipfs.clone(), keypair);

        let cid =
            Cid::try_from("bafybeig6xv5nwphfmvcnektpnojts33jqcuam7bmye2pb54adnrtccjlsu").unwrap();

        let result = system.sign(cid).await.unwrap();

        let raw: RawJWS = ipfs.dag_get(result, Option::<&str>::None).await.unwrap();

        let signing_input = format!("{}.{}", raw.payload, raw.signatures[0].protected);

        let JsonWebSignature {
            payload,
            mut signatures,
            link: _,
        } = raw.try_into().unwrap();

        assert_eq!(payload, cid);

        let Signature { header, signature } = signatures.remove(0);

        let Header {
            algorithm,
            json_web_key,
        } = header;

        assert!(algorithm.unwrap() == AlgorithmType::EdDSA);

        let JsonWebKey {
            key_type,
            curve,
            x,
            y: _,
        } = json_web_key.unwrap();

        assert!(key_type == KeyType::OctetString);
        assert!(curve == CurveType::Ed25519);

        let data = Base::Base64Url.decode(x).unwrap();

        let public_key = ed25519_dalek::PublicKey::from_bytes(&data).unwrap();

        assert_eq!(ori_pub_key, public_key);

        let signature = ed25519_dalek::Signature::from_bytes(&signature).unwrap();

        assert!(public_key
            .verify(signing_input.as_bytes(), &signature)
            .is_ok())
    }

    #[test]
    fn k256_roundtrip() {
        todo!()
    }
}
