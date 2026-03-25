use yozora_core_tokenizer::{TokenizerId, UNKNOWN_TOKENIZER_ID};

const FNV1A64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x100000001b3;

pub(crate) fn calc_tokenizer_uid(name: &str) -> TokenizerId {
    let mut hash = FNV1A64_OFFSET_BASIS;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV1A64_PRIME);
    }

    if hash == UNKNOWN_TOKENIZER_ID {
        UNKNOWN_TOKENIZER_ID.wrapping_sub(1)
    } else {
        hash
    }
}
