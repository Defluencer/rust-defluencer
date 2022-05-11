use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Error;

use signature::Signature;

use ledger_transport::{APDUCommand, APDUErrorCode};
use ledger_transport_hid::{LedgerHIDError, TransportNativeHID};
use ledger_zondax_generic::{App, LedgerAppError};

// https://github.com/LedgerHQ/app-ethereum/blob/master/doc/ethapp.asc#sign-eth-personal-message

#[derive(Clone)]
struct EthereumLedgerApp {
    transport: Arc<TransportNativeHID>,
}

impl Default for EthereumLedgerApp {
    fn default() -> Self {
        let hidapi = ledger_transport_hid::hidapi::HidApi::new().expect("HID API");
        let transport = TransportNativeHID::new(&hidapi).expect("HID Transport");

        Self {
            transport: Arc::new(transport),
        }
    }
}

impl App for EthereumLedgerApp {
    const CLA: u8 = 0xE0;
}

impl EthereumLedgerApp {
    /* pub async fn device_info(&self) -> Result<DeviceInfo, LedgerAppError<LedgerHIDError>> {
        EthereumLedgerApp::get_device_info(self.transport.as_ref()).await
    } */

    /* pub async fn app_info(&self) -> Result<AppInfo, LedgerAppError<LedgerHIDError>> {
        EthereumLedgerApp::get_app_info(self.transport.as_ref()).await
    } */

    /* pub async fn version(&self) -> Result<Version, LedgerAppError<LedgerHIDError>> {
        EthereumLedgerApp::get_version(self.transport.as_ref()).await
    } */

    pub async fn sign(&self, message: &[u8]) -> Result<Vec<u8>, LedgerAppError<LedgerHIDError>> {
        if message.is_empty() {
            return Err(LedgerAppError::InvalidEmptyMessage);
        }

        let mut data = Vec::with_capacity(250);

        data.push(5u8); // Number of BIP 32 derivations to perform

        // Derivation Path
        data.extend(0x2Cu32.to_be_bytes()); // Purpose 4 bytes
        data.extend(0x3Cu32.to_be_bytes()); // Coin type 4 bytes
        data.extend(0u32.to_be_bytes()); // Account 4 bytes
        data.extend(0u32.to_be_bytes()); // Change 4 bytes
        data.extend(0u32.to_be_bytes()); // Index 4 bytes

        data.extend((message.len() as u32).to_be_bytes()); // Message length

        let space_left = 250 - data.len();

        if message.len() > space_left {
            data.extend(&message[0..space_left]);
        } else {
            data.extend(message);
        }

        let command = APDUCommand {
            cla: EthereumLedgerApp::CLA,
            ins: 0x08,
            p1: 0x00,
            p2: 0x00,
            data,
        };

        let mut response = self.transport.exchange(&command)?;

        match response.error_code() {
            Ok(APDUErrorCode::NoError) => {}
            Ok(err) => return Err(LedgerAppError::AppSpecific(err as _, err.description())),
            Err(err) => return Err(LedgerAppError::Unknown(err as _)),
        }

        if message.len() <= space_left {
            return Ok(response.data().to_vec());
        }

        let chunks = message[space_left..].chunks(255);

        for chunk in chunks {
            let command = APDUCommand {
                cla: command.cla,
                ins: command.ins,
                p1: 0x80,
                p2: 0,
                data: chunk.to_vec(),
            };

            response = self.transport.exchange(&command)?;

            match response.error_code() {
                Ok(APDUErrorCode::NoError) => {}
                Ok(err) => return Err(LedgerAppError::AppSpecific(err as _, err.description())),
                Err(err) => return Err(LedgerAppError::Unknown(err as _)),
            }
        }

        Ok(response.data().to_vec())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct EthereumSigner {
    app: EthereumLedgerApp,
}

#[cfg(not(target_arch = "wasm32"))]
impl EthereumSigner {
    pub fn new() -> Self {
        let app = EthereumLedgerApp::default();

        Self { app }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait(?Send)]
impl super::Signer for EthereumSigner {
    async fn sign(
        &self,
        signing_input: Vec<u8>,
    ) -> Result<(k256::PublicKey, k256::ecdsa::Signature), Error> {
        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(&signing_input);

        let sig = self.app.sign(&signing_input).await?;

        let signature = k256::ecdsa::recoverable::Signature::from_bytes(&sig)?;

        let recovered_key = signature.recover_verifying_key(&eth_message)?; // The fn hash the message

        let public_key = k256::PublicKey::from(recovered_key);
        let signature = k256::ecdsa::Signature::from(signature);

        Ok((public_key, signature))
    }
}

#[cfg(target_arch = "wasm32")]
use web3::{transports::eip_1193::Eip1193, Web3};

#[cfg(target_arch = "wasm32")]
use linked_data::types::Address;

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct EthereumSigner {
    addr: Address,
    web3: Web3<Eip1193>,
}

#[cfg(target_arch = "wasm32")]
impl EthereumSigner {
    pub fn new(addr: Address, web3: Web3<Eip1193>) -> Self {
        Self { addr, web3 }
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl super::Signer for EthereumSigner {
    async fn sign(
        &self,
        signing_input: Vec<u8>,
    ) -> Result<(k256::PublicKey, k256::ecdsa::Signature), Error> {
        let mut eth_message =
            format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
        eth_message.extend_from_slice(&signing_input);

        let sig = self
            .web3
            .personal()
            .sign(signing_input.into(), self.addr.into(), "")
            .await?;

        let signature = k256::ecdsa::recoverable::Signature::from_bytes(&sig.to_fixed_bytes())?;

        let recovered_key = signature.recover_verifying_key(&eth_message)?; // The fn hash the message

        let public_key = k256::PublicKey::from(recovered_key);
        let signature = k256::ecdsa::Signature::from(signature);

        Ok((public_key, signature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sha2::{Digest, Sha256};

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn sign() {
        let app = EthereumLedgerApp::default();

        let input = "Hello World!";
        let mut message = format!("\x19Ethereum Signed Message:\n{}", input.len()).into_bytes();
        message.extend_from_slice(input.as_bytes());

        let mut hasher = Sha256::new();
        hasher.update(message.clone());
        let hash = hasher.finalize();

        println!("Hash: {}", hex::encode(hash));

        let sig = app.sign(&message).await.expect("Signature");

        println!("{:?}", sig);
    }
}
