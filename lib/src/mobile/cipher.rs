use std::num::NonZeroU32;
use std::ptr::hash;

use base64::{Engine, engine::general_purpose};
use cipher::{AsyncStreamCipher, KeyIvInit};
use cipher::generic_array::GenericArray;
use rand::Rng;
use ring::pbkdf2;
use ring::pbkdf2::PBKDF2_HMAC_SHA1;
use md5::{Md5, Digest};

fn create_secret_key(str: &[u8], salt: &[u8]) -> [u8; 16] {
    let mut store = [0u8; 16];
    pbkdf2::derive(PBKDF2_HMAC_SHA1, NonZeroU32::new(1000).unwrap(), salt, str, &mut store);
    store
}

fn base64_encode(input: &[u8]) -> String {
    general_purpose::STANDARD.encode(input)
        .replace('\n', "")
        .replace('+', "-")
        .replace('=', "_")
        .replace('/', "~")
}

fn base64_decode(input: String) -> Vec<u8> {
    let input = input.replace("\n", "")
                .replace('-', "+")
                .replace('_', "=")
                .replace('~', "/");
    general_purpose::STANDARD.decode(input).unwrap()
}

type Aes128CfbEnc = cfb_mode::Encryptor<aes::Aes128>;
type Aes128CfbDec = cfb_mode::Decryptor<aes::Aes128>;

const DEFAULT_KEY: [u8; 58] = [54, 51, 54, 100, 51, 50, 53, 52, 51, 52, 53, 57, 50, 49, 53, 97, 54, 54, 52, 57, 53, 99, 50, 50, 52, 102, 53, 97, 51, 100, 50, 99, 54, 51, 54, 98, 53, 102, 53, 100, 50, 57, 51, 51, 53, 102, 52, 51, 51, 101, 51, 54, 50, 50, 52, 102, 52, 100];

fn encrypt(input: String) -> String {
    let input_bytes = input.as_bytes();
    let mut rng = rand::thread_rng();

    let cnkey = DEFAULT_KEY;
    let enc_key_salt = rng.gen::<[u8; 16]>();

    let enc_key: [u8; 16] = create_secret_key(&cnkey,
                                              &enc_key_salt);
    let obsc_key: [u8; 16] = create_secret_key(&cnkey,
                                               input_bytes);

    let iv: [u8; 16] = rand::random();
    let to_encrypt: Vec<u8> = [&obsc_key, input_bytes].concat();
    let mut encrypted_data = to_encrypt;
    Aes128CfbEnc::new(&enc_key.into(), &iv.into()).encrypt(&mut encrypted_data);

    let mut output = [iv, enc_key_salt].concat();
    output.append(&mut encrypted_data);

    // dbg!(&output);

    base64_encode(&output)
}

fn decrypt(input: String) -> String {
    let input_bytes = base64_decode(input);

    let iv = input_bytes[..16].to_vec();
    let salt = input_bytes[16..32].to_vec();
    let encrypted_data = input_bytes[32..].to_vec();

    let key = create_secret_key(&DEFAULT_KEY, &salt);
    let mut decrypted_data = encrypted_data;
    Aes128CfbDec::new(&key.into(), GenericArray::from_slice(iv.as_slice()))
        .decrypt(&mut decrypted_data);
    let decrypted_data = decrypted_data[16..].to_vec();
    String::from_utf8(decrypted_data).unwrap()
}

pub fn encrypt_arguments(prg_name: String, session_id: String, args: Vec<&str>) -> String {
    let args = &format!("{prg_name},{session_id},{}", args.join(","));
    let hash = Md5::digest(args.as_bytes());
    let hash = hex::encode(hash).to_uppercase();
    encrypt(hash + "," + args)
}


#[cfg(test)]
mod tests {
    use crate::mobile::cipher::{base64_encode, create_secret_key, decrypt, encrypt, encrypt_arguments};

    #[test]
    fn test_key_gen() {
        let key = create_secret_key(b"test", b"salt");
        assert_eq!(&format!("{:X?}", key), "[B6, 22, F0, 75, E5, D1, 16, 9C, A0, 9A, 9B, 3F, A, DB, A8, 7E]");
    }

    #[test]
    fn test_base64() {
        assert_eq!(base64_encode(b"test"), "dGVzdA__")
    }


    #[test]
    fn test_encryption() {
        let enc = encrypt("test".to_string());
        let dec = decrypt(enc);
        assert_eq!(dec, "test")
    }

    #[test]
    fn test_arg_encryption() {
        let enc = encrypt_arguments(
            "GETEXAMS".to_string(), "322587234897118".to_string(), vec!["000000", "STD"]);
        let dec = decrypt(enc);
        assert_eq!(dec.split(',').skip(1).collect::<Vec<&str>>(), vec!["GETEXAMS", "322587234897118", "000000", "STD"])
    }

    #[test]
    fn test_decryption() {
        let dec = decrypt("D29eG5fjMQg2-pLsosNNJXtyUUecTow~L8L7GXBXXjbk-iG3c12j3PlHWCyvTs81hS241A__".to_string());
        assert_eq!(dec, "test")
    }

}