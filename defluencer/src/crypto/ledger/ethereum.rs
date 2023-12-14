#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

use ledger_transport::{APDUAnswer, APDUCommand, APDUErrorCode};
use ledger_transport_hid::{LedgerHIDError, TransportNativeHID};
use ledger_zondax_generic::{App, AppExt, AppInfo, DeviceInfo, LedgerAppError, Version};

use crate::errors::Error;

#[derive(Clone)]
pub struct EthereumLedgerApp {
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
    pub async fn device_info(&self) -> Result<DeviceInfo, LedgerAppError<LedgerHIDError>> {
        EthereumLedgerApp::get_device_info(self.transport.as_ref()).await
    }

    pub async fn app_info(&self) -> Result<AppInfo, LedgerAppError<LedgerHIDError>> {
        EthereumLedgerApp::get_app_info(self.transport.as_ref()).await
    }

    pub async fn version(&self) -> Result<Version, LedgerAppError<LedgerHIDError>> {
        EthereumLedgerApp::get_version(self.transport.as_ref()).await
    }

    /// Return Public key and the address.
    pub fn get_public_address(&self, index: u32) -> Result<(VerifyingKey, String), Error> {
        let response = self.addr(index)?;
        let data = response.data();

        let pubkey_len = data[0] as usize;
        let pubkey_start = 1;
        let pubkey_end = pubkey_start + pubkey_len;

        let pubkey = &data[pubkey_start..pubkey_end];

        let adrr_len = data[pubkey_end] as usize;
        let addr_start = pubkey_end + 1;
        let addr_end = addr_start + adrr_len;

        let addr = &data[addr_start..addr_end];

        let public_key = VerifyingKey::from_sec1_bytes(pubkey)?;
        let address = std::str::from_utf8(addr)?.to_owned();

        Ok((public_key, address))
    }

    /// The message displayed on the screen is UTF-8 or hex encoded.
    ///
    /// The signature is standard ETH signature scheme.
    /// Message with prefix hashed with Keccak256.
    pub fn sign_personal_message(&self, message: &[u8], index: u32) -> Result<(Signature, RecoveryId), Error> {
        let response = self.sign(message, index)?;

        // V returned at byte index 0 instead of last
        // k256 crate only use id 0 or 1 so for ETH minus 27
        let id = RecoveryId::try_from(response.data()[0] - 27)?;

        // R & S returned from ledger in same order as k256 signature
        let signature = Signature::try_from(&response.data()[1..])?;

        Ok((signature, id))
    }

    fn addr(&self, index: u32) -> Result<APDUAnswer<Vec<u8>>, LedgerAppError<LedgerHIDError>> {
        // https://github.com/LedgerHQ/app-ethereum/blob/master/doc/ethapp.asc#get-eth-public-address

        let mut data = Vec::with_capacity(25);

        data.push(5_u8); // Number of BIP 32 derivations to perform

        // Derivation Path, hardend key start with 0x8xxxxxxx
        data.extend(0x8000002C_u32.to_be_bytes()); // Purpose 4 bytes
        data.extend(0x8000003C_u32.to_be_bytes()); // Coin type 4 bytes
        data.extend(0x80000000_u32.to_be_bytes()); // Account 4 bytes
        data.extend(0x00000000_u32.to_be_bytes()); // Change 4 bytes
        data.extend(index.to_be_bytes()); // Index 4 bytes

        let command = APDUCommand {
            cla: EthereumLedgerApp::CLA,
            ins: 0x02, // get address command code
            p1: 0x01,  // show addr ask user confirmation
            p2: 0x00,  // don't return chain code
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

    fn sign(
        &self,
        message: &[u8],
        index: u32,
    ) -> Result<APDUAnswer<Vec<u8>>, LedgerAppError<LedgerHIDError>> {
        // https://github.com/LedgerHQ/app-ethereum/blob/master/doc/ethapp.asc#sign-eth-personal-message

        if message.is_empty() {
            return Err(LedgerAppError::InvalidEmptyMessage);
        }

        let mut data = Vec::with_capacity(255);

        data.push(5_u8); // Number of BIP 32 derivations to perform

        // Derivation Path, hardend key start with 0x8xxxxxxx
        data.extend(0x8000002C_u32.to_be_bytes()); // Purpose 4 bytes
        data.extend(0x8000003C_u32.to_be_bytes()); // Coin type 4 bytes
        data.extend(0x80000000_u32.to_be_bytes()); // Account 4 bytes
        data.extend(0x00000000_u32.to_be_bytes()); // Change 4 bytes
        data.extend(index.to_be_bytes()); // Index 4 bytes

        data.extend((message.len() as u32).to_be_bytes()); // Message length

        let space_left = 255 - data.len();

        if message.len() > space_left {
            data.extend(&message[0..space_left]);
        } else {
            data.extend(message);
        }

        let command = APDUCommand {
            cla: EthereumLedgerApp::CLA,
            ins: 0x08, // sign personnal message code
            p1: 0x00,  // first data block
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
            return Ok(response);
        }

        let chunks = message[space_left..].chunks(255);

        for chunk in chunks {
            let command = APDUCommand {
                cla: command.cla,
                ins: command.ins,
                p1: 0x80, // subsequent data block
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

        Ok(response)
    }
}
