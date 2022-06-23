#![cfg(not(target_arch = "wasm32"))]

use std::{collections::VecDeque, sync::Arc};

use k256::{
    ecdsa::recoverable::{Id, Signature},
    PublicKey,
};

use ledger_transport::{APDUAnswer, APDUCommand, APDUErrorCode};
use ledger_transport_hid::{LedgerHIDError, TransportNativeHID};
use ledger_zondax_generic::{App, AppExt, AppInfo, DeviceInfo, LedgerAppError, Version};

use rs_merkle::{Hasher, MerkleTree};

use sha2::Digest;

use bitcoin::consensus::encode::{Decodable, Encodable, VarInt};

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
    pub fn get_public_address(&self, index: u32) -> Result<(PublicKey, String), Error> {
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

        let public_key = k256::PublicKey::from_sec1_bytes(pubkey)?;
        let address = std::str::from_utf8(addr)?.to_owned();

        Ok((public_key, address))
    }

    /// The hash displayed on the screen is the SHA-256 of the message.
    ///
    /// The signature is standard ETH signature scheme.
    /// Message with prefix hashed with Keccak256.
    pub fn sign_personal_message(&self, message: &[u8], index: u32) -> Result<Signature, Error> {
        use signature::Signature;

        let response = self.sign(message, index)?;

        // R & S returned from ledger in same order as k256 signature
        let signature = k256::ecdsa::Signature::from_bytes(&response.data()[1..])?;
        // V returned at byte index 0 instead of last
        // k256 crate only use id 0 or 1 so for ETH minus 27
        let id = Id::new(response.data()[0] - 27)?;

        let signature = k256::ecdsa::recoverable::Signature::new(&signature, id)?;

        Ok(signature)
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

#[derive(Clone)]
pub struct BitcoinLedgerApp {
    transport: Arc<TransportNativeHID>,
}

impl Default for BitcoinLedgerApp {
    fn default() -> Self {
        let hidapi = ledger_transport_hid::hidapi::HidApi::new().expect("HID API");
        let transport = TransportNativeHID::new(&hidapi).expect("HID Transport");

        Self {
            transport: Arc::new(transport),
        }
    }
}

impl App for BitcoinLedgerApp {
    const CLA: u8 = 0xE1;
}

impl BitcoinLedgerApp {
    /// The public key is shown on the screen. After confirmation by the user.
    ///
    /// Returns the public key & full serialized extended version as per BIP-32.
    pub fn get_extended_pubkey(&self, index: u32) -> Result<(PublicKey, String), Error> {
        let response = self.addr(index)?;

        let addr = std::str::from_utf8(response.data())?;

        let decoded = multibase::Base::Base58Btc.decode(addr)?;

        /* #[cfg(debug_assertions)]
        println!(
            "Version: {:X?}\nDepth: {}\nFingerprint: {:X?}\nChild Number: {:X?}\nChain Code: {:?}\nPublick Key: {:?}",
            &decoded[0..4],
            decoded[4],
            &decoded[5..9],
            &decoded[9..13],
            &decoded[13..45],
            &decoded[45..78]
        ); */

        let public_key = k256::PublicKey::from_sec1_bytes(&decoded[45..78])?;

        Ok((public_key, addr.to_owned()))
    }

    fn addr(&self, index: u32) -> Result<APDUAnswer<Vec<u8>>, LedgerAppError<LedgerHIDError>> {
        // https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/doc/bitcoin.md#get_extended_pubkey

        let mut data = Vec::with_capacity(22);

        data.push(1_u8); // Show on screen
        data.push(5_u8); // Number of BIP 32 derivations to perform

        // Derivation Path, hardend key start with 0x8xxxxxxx
        data.extend(0x8000002C_u32.to_be_bytes()); // Purpose 4 bytes
        data.extend(0x80000000_u32.to_be_bytes()); // Coin type 4 bytes
        data.extend(0x80000000_u32.to_be_bytes()); // Account 4 bytes
        data.extend(0x00000000_u32.to_be_bytes()); // Change 4 bytes
        data.extend(index.to_be_bytes()); // Index 4 bytes

        let command = APDUCommand {
            cla: BitcoinLedgerApp::CLA,
            ins: 0x00,
            p1: 0x00,
            p2: 0x00,
            data,
        };

        /*  #[cfg(debug_assertions)]
        println!("GET_EXTENDED_PUBKEY"); */

        let response = self.transport.exchange(&command)?;

        match response.error_code() {
            Ok(APDUErrorCode::NoError) => return Ok(response),
            Ok(err) => return Err(LedgerAppError::AppSpecific(err as _, err.description())),
            Err(err) => match err {
                0x6A86 => {
                    return Err(LedgerAppError::AppSpecific(
                        0x6A86,
                        String::from("Either P1 or P2 is incorrect"),
                    ))
                }
                0x6A87 => {
                    return Err(LedgerAppError::AppSpecific(
                        0x6A87,
                        String::from("Lc or minimum APDU length is incorrect"),
                    ))
                }
                0xB000 => {
                    return Err(LedgerAppError::AppSpecific(
                        0xB000,
                        String::from("Wrong response length (buffer size problem)"),
                    ))
                }
                0xB007 => {
                    return Err(LedgerAppError::AppSpecific(
                        0xB007,
                        String::from("Aborted because unexpected state reached"),
                    ))
                }
                0xB008 => {
                    return Err(LedgerAppError::AppSpecific(
                        0xB008,
                        String::from("Invalid signature or HMAC"),
                    ))
                }

                err => return Err(LedgerAppError::Unknown(err as _)),
            },
        }
    }

    /// The display hash is the Sha2-256 of the message.
    ///
    /// Returns EcDSA signature.
    pub fn sign_message(&self, message: &[u8], index: u32) -> Result<Signature, Error> {
        use signature::Signature;

        let response = self.sign(message, index)?;

        /* #[cfg(debug_assertions)]
        println!("Response Data: {:?}", response.data()); */

        // R & S returned from ledger in same order as k256 signature
        let signature = k256::ecdsa::Signature::from_bytes(&response.data()[1..])?;
        // V returned at byte index 0 instead of last
        // k256 crate only use id 0 or 1 so for BTC minus 31
        let id = Id::new(response.data()[0] - 31)?;

        let signature = k256::ecdsa::recoverable::Signature::new(&signature, id)?;

        Ok(signature)
    }

    fn sign(
        &self,
        message: &[u8],
        index: u32,
    ) -> Result<APDUAnswer<Vec<u8>>, LedgerAppError<LedgerHIDError>> {
        // https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/doc/bitcoin.md#sign_message
        // https://docs.rs/bitcoin/0.28.1/bitcoin/consensus/encode/struct.VarInt.html

        if message.is_empty() {
            return Err(LedgerAppError::InvalidEmptyMessage);
        }

        let mut data = Vec::with_capacity(255);

        data.push(5_u8); // Number of BIP 32 derivations to perform

        // Derivation Path, hardend key start with 0x8xxxxxxx
        data.extend(0x8000002C_u32.to_be_bytes()); // Purpose 4 bytes
        data.extend(0x80000000_u32.to_be_bytes()); // Coin type 4 bytes
        data.extend(0x80000000_u32.to_be_bytes()); // Account 4 bytes
        data.extend(0x00000000_u32.to_be_bytes()); // Change 4 bytes
        data.extend(index.to_be_bytes()); // Index 4 bytes

        let msg_length = {
            let mut temp = Vec::with_capacity(9); // Bicoin style Varint
            VarInt(message.len() as u64)
                .consensus_encode(&mut temp)
                .expect("VarInt encoded message length");
            temp
        };

        data.extend(msg_length); // Message length

        let chunks = message.chunks(64);

        let mut datums = Vec::with_capacity(chunks.len());
        let mut hashes: Vec<[u8; 32]> = Vec::with_capacity(chunks.len());

        let mut hasher = sha2::Sha256::new();

        for chunk in chunks {
            let mut data = Vec::with_capacity(65);
            data.push(0x00);
            data.extend(chunk);

            hasher.update(&data);
            let hash = hasher.finalize_reset();

            datums.push(data);
            hashes.push(hash.into());
        }

        let merkle_tree = MerkleTree::<BitcoinMerkle>::from_leaves(&hashes);

        let merkle_root = merkle_tree.root().expect("Merkle Root");

        data.extend(merkle_root);

        let command = APDUCommand {
            cla: BitcoinLedgerApp::CLA,
            ins: 0x10,
            p1: 0x00,
            p2: 0x00,
            data,
        };

        /* #[cfg(debug_assertions)]
        println!(
            "Merkle Root: {:?}\nTree depth: {}\nSIGN_MESSAGE\nRaw Command Data: {:?}",
            merkle_root,
            merkle_tree.depth(),
            command.serialize()
        ); */

        let mut response = self.transport.exchange(&command)?;

        let mut proof_queue: VecDeque<[u8; 32]> = VecDeque::with_capacity(10);

        loop {
            /* #[cfg(debug_assertions)]
            println!("Raw Response Data: {:?}", response.data()); */

            match response.error_code() {
                Ok(APDUErrorCode::NoError) => return Ok(response),
                Ok(err) => return Err(LedgerAppError::AppSpecific(err as _, err.description())),
                Err(err) => match err {
                    0x6A86 => return Err(LedgerAppError::AppSpecific(0x6A86, String::from("Either P1 or P2 is incorrect"))),
                    0x6A87 => return Err(LedgerAppError::AppSpecific(0x6A87, String::from("Lc or minimum APDU length is incorrect"))),
                    0xB000 => return Err(LedgerAppError::AppSpecific(0xB000, String::from("Wrong response length (buffer size problem)"))),
                    0xB007 => return Err(LedgerAppError::AppSpecific(0xB007, String::from("Aborted because unexpected state reached"))),
                    0xB008 => return Err(LedgerAppError::AppSpecific(0xB008, String::from("Invalid signature or HMAC"))),
                    0xE000 /* SW_INTERRUPTED_EXECUTION */ => {/* proceed with logic below */}
                    err => return Err(LedgerAppError::Unknown(err as _))
                },
            }

            let mut data = Vec::with_capacity(255);

            match response.data()[0] {
                0x40 => {
                    // https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/doc/bitcoin.md#40-get_preimage

                    /* #[cfg(debug_assertions)]
                    println!("GET_PREIMAGE"); */

                    if response.data()[1] != 0 {
                        panic!("Must equal zero, reserved for future uses.");
                    }

                    let hash = &response.data()[2..34];

                    /* #[cfg(debug_assertions)]
                    println!("Hash: {:?}", hash); */

                    let index = hashes.iter().position(|item| item == hash).expect("Hash");

                    let mut varint = Vec::with_capacity(9);
                    VarInt(datums[index].len() as u64)
                        .consensus_encode(&mut varint)
                        .expect("VarInt encoded datum length");

                    data.extend(varint.iter());
                    data.push(datums[index].len() as u8);
                    data.extend(datums[index].iter());

                    /* #[cfg(debug_assertions)]
                    println!(
                        "Preimage length: {:?}\nProof length: {}\nPreimage Data: {:?}",
                        &data[0..varint.len()],
                        data[varint.len()],
                        &data[(varint.len() + 1)..]
                    ); */
                }
                0x41 => {
                    // https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/doc/bitcoin.md#get_merkle_leaf_proof

                    /* #[cfg(debug_assertions)]
                    println!("GET_MERKLE_LEAF_PROOF"); */

                    /* let _root_hash = {
                        let mut temp = [0u8; 32];
                        temp.copy_from_slice(&response.data()[1..33]);
                        temp
                    }; */

                    let offset = {
                        let slice = &response.data()[33..];
                        let varint = VarInt::consensus_decode(slice).expect("Varint");
                        varint.len()
                    };

                    let leaf_index = {
                        let slice = &response.data()[33 + offset..];
                        let varint = VarInt::consensus_decode(slice).expect("Varint");
                        varint.0 as usize
                    };

                    /* #[cfg(debug_assertions)]
                    println!(
                        "Root: {:?}\nTree size: {}\nLeaf Index: {}",
                        root_hash, tree_size, leaf_index
                    ); */

                    let hash = hashes[leaf_index];

                    data.extend(hash);

                    let proof = merkle_tree.proof(&[leaf_index]);

                    proof_queue.extend(proof.proof_hashes());

                    data.push(proof_queue.len() as u8);

                    let mut space_left = 6;
                    let mut proofs = Vec::with_capacity(space_left);

                    while let Some(proof) = proof_queue.pop_front() {
                        proofs.push(proof);
                        space_left -= 1;

                        if space_left == 0 {
                            break;
                        }
                    }

                    data.push(proofs.len() as u8);
                    data.extend(proofs.iter().flatten());

                    /* #[cfg(debug_assertions)]
                    println!(
                        "Leaf Hash: {:?}\nProof length: {}\nIncluded Proofs: {}\nProofs Data: {:?}",
                        &data[0..32],
                        data[32],
                        data[33],
                        &data[34..]
                    ); */
                }
                0x42 => {
                    // https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/doc/bitcoin.md#get_merkle_leaf_index

                    /* #[cfg(debug_assertions)]
                    println!("GET_MERKLE_LEAF_INDEX");

                    #[cfg(debug_assertions)]
                    println!("Root: {:?}", &response.data()[1..33]); */

                    let leaf_hash = &response.data()[33..65];

                    match hashes.iter().position(|item| item == leaf_hash) {
                        Some(idx) => {
                            let index = {
                                let mut msg_len = Vec::with_capacity(9); // Bicoin style Varint
                                VarInt(idx as u64)
                                    .consensus_encode(&mut msg_len)
                                    .expect("VarInt encoded message length");
                                msg_len
                            };

                            data.push(1);
                            data.extend(index);
                        }
                        None => {
                            data.push(0);
                            data.push(0);
                        }
                    };

                    /* #[cfg(debug_assertions)]
                    println!("Matching Leaf: {}\nLeaf Index: {}", data[0], data[1]); */
                }
                0xA0 => {
                    // https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/doc/bitcoin.md#get_more_elements

                    /* #[cfg(debug_assertions)]
                    println!("GET_MORE_ELEMENTS"); */

                    let mut space_left = 7;
                    let mut proofs = Vec::with_capacity(space_left);

                    while let Some(proof) = proof_queue.pop_front() {
                        proofs.push(proof);
                        space_left -= 1;

                        if space_left == 0 {
                            break;
                        }
                    }

                    data.push(proofs.len() as u8);
                    data.push(32_u8);
                    data.extend(proofs.iter().flatten());

                    /* #[cfg(debug_assertions)]
                    println!(
                        "Returned Elements: {}\nSize: {}\nData: {:?}",
                        data[0],
                        data[1],
                        &data[2..]
                    ); */
                }
                _ => panic!("Unknown Command Code"),
            }

            let command = APDUCommand {
                cla: 0xF8,
                ins: 0x01,
                p1: 0x00,
                p2: 0x00,
                data,
            };

            /* #[cfg(debug_assertions)]
            println!("Raw Command Data: {:?}", command.serialize()); */

            response = self.transport.exchange(&command)?;
        }
    }
}

#[derive(Clone)]
pub struct BitcoinMerkle {}

impl Hasher for BitcoinMerkle {
    // https://github.com/LedgerHQ/app-bitcoin-new/blob/develop/doc/merkle.md

    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> [u8; 32] {
        use sha2::digest::FixedOutput;

        let mut hasher = sha2::Sha256::new();

        hasher.update(data);
        <[u8; 32]>::from(hasher.finalize_fixed())
    }

    fn concat_and_hash(left: &Self::Hash, right: Option<&Self::Hash>) -> Self::Hash {
        let mut vec = Vec::with_capacity(65);
        vec.push(0x01); // for internal nodes

        match right {
            Some(right_node) => {
                vec.extend(left);
                vec.extend(right_node);

                Self::hash(&vec)
            }
            None => {
                *left
                //vec.extend(left);
                //vec.extend(left);

                //Self::hash(&vec)
            }
        }
    }
}

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

        //strip 0x00 padding
        let mut len = 0;
        for item in response.data().iter().rev() {
            if *item == 0 {
                len += 1;
            } else {
                break;
            }
        }

        let data = &response.data()[0..(73 - len)];

        let signature = k256::ecdsa::Signature::from_der(data)?;

        Ok(signature)
    }

    fn sign(&self, message: &[u8]) -> Result<APDUAnswer<Vec<u8>>, LedgerAppError<LedgerHIDError>> {
        // https://github.com/LedgerHQ/app-ethereum/blob/master/doc/ethapp.asc#sign-eth-personal-message

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

    use signature::DigestVerifier;
    //use signature::Signature;

    const SIGNING_INPUT: [u8; 12] = *b"Hello World!";

    #[test]
    fn sign() {
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
            let mut sig = app.sign_personal_message(&SIGNING_INPUT).unwrap();

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
    fn example() {
        use signature::DigestSigner;
        //use signature::Signer;

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
