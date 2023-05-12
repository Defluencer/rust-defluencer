#![cfg(not(target_arch = "wasm32"))]

mod bitcoin;
mod ethereum;

pub use self::bitcoin::BitcoinLedgerApp;
pub use ethereum::EthereumLedgerApp;

use std::sync::Arc;

use ledger_transport::{APDUAnswer, APDUCommand, APDUErrorCode};
use ledger_transport_hid::{LedgerHIDError, TransportNativeHID};
use ledger_zondax_generic::{App, LedgerAppError};

use crate::errors::Error;

//https://github.com/LedgerHQ/rust-app
#[derive(Clone)]
pub struct TestLedgerApp {
    transport: Arc<TransportNativeHID>,
}

impl Default for TestLedgerApp {
    fn default() -> Self {
        let hidapi = ledger_transport_hid::hidapi::HidApi::new().expect("HID API");
        let transport = TransportNativeHID::new(&hidapi).expect("HID Transport");

        Self {
            transport: Arc::new(transport),
        }
    }
}

impl App for TestLedgerApp {
    const CLA: u8 = 8;
}

impl TestLedgerApp {
    pub fn sign_personal_message(&self, message: &[u8]) -> Result<k256::ecdsa::Signature, Error> {
        let response = self.sign(message)?;

        //strip 0x00 right padding
        let mut len = 0;
        for item in response.data().iter().rev() {
            if *item == 0 {
                len += 1;
            } else {
                break;
            }
        }

        let data = &response.data()[0..(73 - len)]; // DER encoded signatures are max 73 bytes

        let signature = k256::ecdsa::Signature::from_der(data)?;

        Ok(signature)
    }

    fn sign(&self, message: &[u8]) -> Result<APDUAnswer<Vec<u8>>, LedgerAppError<LedgerHIDError>> {
        if message.is_empty() {
            return Err(LedgerAppError::InvalidEmptyMessage);
        }

        let command = APDUCommand {
            cla: TestLedgerApp::CLA,
            ins: 3, // sign personnal message code
            p1: 0x00,
            p2: 0x00,
            data: message.to_vec(),
        };

        let response = self.transport.exchange(&command)?;

        match response.error_code() {
            Ok(APDUErrorCode::NoError) => {}
            Ok(err) => return Err(LedgerAppError::AppSpecific(err as _, err.description())),
            Err(err) => return Err(LedgerAppError::Unknown(err as _)),
        }

        Ok(response)
    }

    pub fn get_pubkey(&self) -> Result<k256::PublicKey, Error> {
        let response = self.pub_key()?;

        let public_key = k256::PublicKey::from_sec1_bytes(response.data())?;

        Ok(public_key)
    }

    fn pub_key(&self) -> Result<APDUAnswer<Vec<u8>>, LedgerAppError<LedgerHIDError>> {
        let data = Vec::with_capacity(0);

        let command = APDUCommand {
            cla: TestLedgerApp::CLA,
            ins: 2,
            p1: 0x00,
            p2: 0x00,
            data,
        };

        let response = self.transport.exchange(&command)?;

        match response.error_code() {
            Ok(APDUErrorCode::NoError) => {}
            Ok(err) => return Err(LedgerAppError::AppSpecific(err as _, err.description())),
            Err(err) => return Err(LedgerAppError::Unknown(err as _)),
        }

        Ok(response)
    }

    pub fn get_priv_key(&self) -> Result<k256::SecretKey, Error> {
        let response = self.priv_key()?;

        let priv_key = k256::SecretKey::from_be_bytes(response.data())?;

        Ok(priv_key)
    }

    fn priv_key(&self) -> Result<APDUAnswer<Vec<u8>>, LedgerAppError<LedgerHIDError>> {
        let data = Vec::with_capacity(0);

        let command = APDUCommand {
            cla: TestLedgerApp::CLA,
            ins: 0xFE,
            p1: 0x00,
            p2: 0x00,
            data,
        };

        let response = self.transport.exchange(&command)?;

        match response.error_code() {
            Ok(APDUErrorCode::NoError) => {}
            Ok(err) => return Err(LedgerAppError::AppSpecific(err as _, err.description())),
            Err(err) => return Err(LedgerAppError::Unknown(err as _)),
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pkcs8::EncodePublicKey;

    use sha2::{Digest, Sha256};

    const SIGNING_INPUT: [u8; 12] = *b"Hello World!";

    #[test]
    #[ignore]
    fn sign() {
        use k256::ecdsa::signature::DigestVerifier;

        let app = TestLedgerApp::default();

        let secret_key = app.get_priv_key().unwrap();
        println!(
            "Secret key: {}",
            *secret_key.to_pem(Default::default()).unwrap()
        );
        let sign_key = k256::ecdsa::SigningKey::from(secret_key);

        let pub_key = app.get_pubkey().unwrap();

        println!(
            "Public key: {}",
            pub_key.to_public_key_pem(Default::default()).unwrap()
        );

        let verif_key = k256::ecdsa::VerifyingKey::from(pub_key);

        assert_eq!(verif_key, k256::ecdsa::VerifyingKey::from(sign_key));

        println!(
            "Message hex: {}\nDigest: {}",
            hex::encode(SIGNING_INPUT),
            multibase::encode(
                multibase::Base::Base64Pad,
                Sha256::new_with_prefix(SIGNING_INPUT).finalize()
            )
        );

        let signature = {
            let hash = Sha256::new_with_prefix(SIGNING_INPUT).finalize();

            let mut sig = app.sign_personal_message(&hash).unwrap();

            if let Some(signature) = sig.normalize_s() {
                sig = signature;
            }

            sig
        };

        println!(
            "Signature: {}",
            multibase::Base::Base64Pad.encode(signature.to_der())
        );

        let digest = Sha256::new_with_prefix(SIGNING_INPUT);
        verif_key
            .verify_digest(digest, &signature)
            .expect("Key Verification");
    }

    #[test]
    #[ignore]
    fn example() {
        use k256::ecdsa::signature::{DigestSigner, DigestVerifier};

        let signing_key = k256::ecdsa::SigningKey::from_bytes(
            &hex::decode("58c185d9033b7624fe0a85f2d784050f7cbc5ec2516ead2631714f25a1ad0d62")
                .unwrap(),
        )
        .unwrap();

        let data = hex::decode("04fe5cc5684ddb951eadd9deca42d5a8b5e546269a63132e5584c5400efb70d61c31c73aaeb1cbbd716994bf68157f23682ef299ec03810d15eed3662a3146eef2").unwrap();
        let pub_key = k256::PublicKey::from_sec1_bytes(&data).unwrap();
        println!(
            "Public key: {}",
            pub_key.to_public_key_pem(Default::default()).unwrap()
        );

        let verif_key = k256::ecdsa::VerifyingKey::from(pub_key);

        let digest = Sha256::new_with_prefix(&SIGNING_INPUT);
        let signature: k256::ecdsa::Signature = signing_key.try_sign_digest(digest).unwrap();

        /* let sig = hex::decode("304402202d8774b341ad532576c8f2b68385059938b9543e8899947433a6f3ea43eb760302203de21062a909f2214511464e0d881140c2171227c0b600d35197c69199874de5").unwrap();
        let signature = k256::ecdsa::Signature::from_der(&sig).unwrap(); */

        println!(
            "Base 64 DER Signature: {}",
            multibase::Base::Base64Pad.encode(signature.to_der())
        );

        let digest = Sha256::new_with_prefix(SIGNING_INPUT);
        verif_key
            .verify_digest(digest, &signature)
            .expect("Key Verification");
    }
}
