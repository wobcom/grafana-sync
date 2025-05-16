use std::marker::PhantomData;

use chacha20poly1305::{aead::Nonce, AeadInPlace, XChaCha20Poly1305};
use integer_encoding::VarInt;
use log::{trace, warn};
use tokio_util::{bytes::{Buf, BufMut, BytesMut}, codec::{Decoder, Encoder}};

use crate::{FrameError, PeerMessage, PingPong, BINCONF};

pub struct EncryptionCodec<M> {
    cipher: XChaCha20Poly1305,
    nonce: u64,

    _message_type: PhantomData<M>,
}

type NonceType = Nonce<XChaCha20Poly1305>;

const NONCE_LEN: usize = size_of::<NonceType>();
const HEADER_LEN: usize = size_of::<u32>();
const MAX_FRAME_LEN: usize = 64 * 1024; // 64KiB should be fine for now. 
                                        // 244 oversized stalled messages needed to OOM DOS

impl<M> EncryptionCodec<M> {
    pub fn encrypt_frame(&mut self, data: &mut Vec<u8>, dst: &mut BytesMut) -> Result<(), FrameError> {
        let nonce_num = self.nonce;
        self.nonce = self.nonce.wrapping_add(1);

        let mut nonce_bytes = [0u8; NONCE_LEN];
        let nonce_bytes_raw = nonce_num.to_be_bytes();
        nonce_bytes[(NONCE_LEN - size_of_val(&nonce_bytes_raw))..].copy_from_slice(&nonce_bytes_raw);
        let nonce = NonceType::from_slice(&nonce_bytes);

        self.cipher.encrypt_in_place(nonce, b"", data)
            .map_err(|_| FrameError::Encryption)?;

        trace!("encrypted to {} bytes", data.len());
        
        let mut varint_nonce = [0u8; 9];
        let varint_nonce_len = nonce_num.encode_var(&mut varint_nonce);
        let varint_nonce = &varint_nonce[9 - varint_nonce_len..];

        let packet_len = u32::try_from(data.len() + varint_nonce_len)?;

        assert_eq!(packet_len as usize, data.len() + varint_nonce.len());

        dst.put_u32(packet_len);
        dst.extend_from_slice(varint_nonce);
        dst.extend_from_slice(data);

        trace!("sent frame of size: {}", HEADER_LEN + varint_nonce_len + data.len());

        Ok(())
    }

    pub fn decrypt_frame(&mut self, data: &mut BytesMut) -> Result<Option<Vec<u8>>, FrameError> {
        if data.len() < HEADER_LEN {
            trace!("size not big enough to read length (yet)");
            return Ok(None);
        }

        let length = {
            let mut b = [0u8; HEADER_LEN];
            b.copy_from_slice(&data[..HEADER_LEN]);
            u32::from_be_bytes(b) as usize
        };

        if length <= HEADER_LEN + 1 {
            return Err(FrameError::PossiblyMaliciousFrame);
        }

        if length > MAX_FRAME_LEN {
            return Err(FrameError::FrameTooBig);
        }

        if data.len() < length + HEADER_LEN {
            trace!("size not big enough (yet). Expecting {length} bytes. Have {}", data.len());
            return Ok(None);
        }

        trace!("received message of {length} bytes");

        data.advance(HEADER_LEN);

        let mut frame = data.split_to(length);

        trace!("left: {} bytes", frame.len());

        let (nonce_num, nonce_num_len) = u64::decode_var(&frame)
            .ok_or(FrameError::InvalidNonce)?;
        
        frame.advance(nonce_num_len);

        let mut nonce_bytes = [0u8; NONCE_LEN];
        let nonce_bytes_raw = nonce_num.to_be_bytes();
        nonce_bytes[(NONCE_LEN - size_of_val(&nonce_bytes_raw))..].copy_from_slice(&nonce_bytes_raw);

        let nonce = NonceType::from_slice(&nonce_bytes);
        let mut packet = frame.to_vec();

        trace!("decrypting {} bytes", packet.len());

        self.cipher.decrypt_in_place(nonce, b"", &mut packet)
            .map_err(|_| FrameError::Decryption)?;

        Ok(Some(packet))
    }
}

impl<M> From<XChaCha20Poly1305> for EncryptionCodec<M> {
    fn from(value: XChaCha20Poly1305) -> Self {
        EncryptionCodec {
            cipher: value,
            nonce: 0,

            _message_type: PhantomData,
        }
    }
}

impl<M> Encoder<PeerMessage<M>> for EncryptionCodec<M>
where
    M: serde::Serialize + PingPong,
{
    type Error = crate::FrameError;

    fn encode(&mut self, item: PeerMessage<M>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut data = bincode::serde::encode_to_vec(item, BINCONF)?;

        self.encrypt_frame(&mut data, dst)?;

        Ok(())
    }
}

impl<M> Decoder for EncryptionCodec<M> 
where
    M: serde::de::DeserializeOwned + PingPong,
{
    type Item = PeerMessage<M>;

    type Error = crate::FrameError;

    fn decode(&mut self, src: &mut BytesMut) 
        -> Result<Option<Self::Item>, Self::Error> 
    {
        let Some(plaintext) = self.decrypt_frame(src)? else {
            return Ok(None);
        };

        let (packet, size) = bincode::serde::decode_from_slice(&plaintext, BINCONF)?;

        if size != plaintext.len() {
            warn!("Encoded data did not fill the frame");
        }

        Ok(Some(packet))
    }
}

