use async_trait::async_trait;

use signature::{SignatureEncoding, Signer};

use async_signature::AsyncSigner;

use crate::{AlgorithmType, JsonWebKey};

/// Impl'd the trait is not enough, one must follow the JOSE specs below.
///
/// https://ipld.io/specs/codecs/dag-jose/spec/
///
/// https://ipld.io/specs/codecs/dag-jose/fixtures/
///
/// https://www.rfc-editor.org/rfc/rfc7515
///
/// https://www.rfc-editor.org/rfc/rfc7517
///
/// https://www.rfc-editor.org/rfc/rfc7518
///
/// https://www.iana.org/assignments/jose/jose.xhtml
///
/// https://www.rfc-editor.org/rfc/rfc8037.html
pub trait BlockSigner<U>: Signer<U>
where
    U: SignatureEncoding,
{
    fn algorithm(&self) -> AlgorithmType;

    fn web_key(&self) -> JsonWebKey;
}

#[async_trait]
pub trait AsyncBlockSigner<S>: AsyncSigner<S>
where
    S: SignatureEncoding + Send + 'static,
{
    async fn algorithm(&self) -> AlgorithmType;

    async fn web_key(&self) -> JsonWebKey;
}
