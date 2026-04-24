//! Tipos de erro por camada. Ver spec §5.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("arquivo vazio não é suportado")]
    EmptyInput,

    #[error("nenhuma imagem hospedeira fornecida")]
    NoHostImages,

    #[error("imagem hospedeira {index} inválida: esperado {expected} bytes de RGB, obtido {got}")]
    InvalidHostImage {
        index: usize,
        expected: usize,
        got: usize,
    },

    #[error(
        "capacidade insuficiente: precisa de {needed} bytes, disponível {available} nas fotos"
    )]
    InsufficientCapacity { needed: usize, available: usize },

    #[error("Argon2id falhou: {0}")]
    KdfFailed(String),

    #[error("AEAD encrypt falhou: {0}")]
    AeadFailed(String),

    #[error("PNG I/O falhou: {0}")]
    PngError(String),

    #[error("erro do subsistema zion-codec: {0}")]
    CodecError(#[from] zion_codec::EncodeError),
}

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("nenhuma imagem stego fornecida")]
    NoStegoImages,

    #[error("imagem stego {index} inválida: esperado {expected} bytes de RGB, obtido {got}")]
    InvalidStegoImage {
        index: usize,
        expected: usize,
        got: usize,
    },

    #[error("plaintext header inválido em imagem {index}: magic não é ZSTG")]
    BadMagic { index: usize, got: [u8; 4] },

    #[error("versão não suportada em imagem {index}: got={got}, max_supported={max_supported}")]
    UnsupportedVersion {
        index: usize,
        got: u8,
        max_supported: u8,
    },

    #[error(
        "AEAD autenticação falhou no símbolo {symbol_index} — passphrase errada ou imagem adulterada"
    )]
    AeadAuthFailed { symbol_index: u16 },

    #[error("Argon2id falhou: {0}")]
    KdfFailed(String),

    #[error("file_id divergente entre imagens stego (sets misturados)")]
    InconsistentFileId,

    #[error("PNG I/O falhou: {0}")]
    PngError(String),

    #[error("erro de reassembly do zion-codec: {0}")]
    ReassemblyFailed(#[from] zion_codec::FileError),

    #[error("erro de decode de símbolo: {0}")]
    SymbolDecodeError(#[from] zion_codec::SymbolError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_error_display_includes_fields() {
        let e = EmbedError::InsufficientCapacity {
            needed: 1000,
            available: 500,
        };
        let s = format!("{e}");
        assert!(s.contains("1000"));
        assert!(s.contains("500"));
    }

    #[test]
    fn extract_error_display_includes_fields() {
        let e = ExtractError::AeadAuthFailed { symbol_index: 3 };
        assert!(format!("{e}").contains('3'));
    }
}
