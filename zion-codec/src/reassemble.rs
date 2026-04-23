//! Multi-symbol reassembly. Collects `DecodedSymbol`s and rebuilds the file.
//! See spec sections 3, 7.3, and 7.4.

use std::collections::BTreeMap;

use crate::constants::{BLOCK_SIZE_RAW, MAX_BLOCK_COUNT, MAX_TOTAL_BLOCKS, MAX_TOTAL_SYMBOLS};
use crate::decode::DecodedSymbol;
use crate::error::FileError;
use crate::format::BlockEntry;
use crate::zstd_layer::decode_block;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoredSymbol {
    block_start: u32,
    block_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReassemblyMetadata {
    file_id: [u8; 16],
    file_size: u64,
    total_blocks: u32,
    total_symbols: u16,
    global_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashStatus {
    Verified,
    Mismatch { expected: [u8; 32], got: [u8; 32] },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedFile {
    pub bytes: Vec<u8>,
    pub hash_status: HashStatus,
}

/// Accumulated reassembly state.
///
/// Usage: `FileReassembler::new()`, then `add_symbol()` for each symbol
/// collected from the optical layer, then `finalize()` returns the file bytes.
#[derive(Debug, Default)]
pub struct FileReassembler {
    metadata: Option<ReassemblyMetadata>,
    /// `block_index` -> `BlockEntry` (or `None` if lost due to a block error)
    blocks: BTreeMap<u32, Option<BlockEntry>>,
    symbols: BTreeMap<u16, StoredSymbol>,
}

impl FileReassembler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a decoded symbol to the reassembly state.
    ///
    /// # Errors
    /// - `SymbolIndexOutOfRange` if `symbol_index >= total_symbols`.
    /// - `BlockRangeExceedsTotal` if the block range exceeds `total_blocks`.
    /// - `SymbolFileIdMismatch` if the symbol belongs to another file.
    /// - `InconsistentFileMetadata` if any aggregated field diverges.
    ///
    /// Duplicate symbols with the same `symbol_index` are treated as idempotent
    /// in v1 unless their payloads conflict.
    pub fn add_symbol(&mut self, decoded: DecodedSymbol) -> Result<(), FileError> {
        let header = decoded.header.clone();
        let hdr = &header;

        validate_header_invariants(hdr)?;
        if hdr.symbol_index >= hdr.total_symbols {
            return Err(FileError::SymbolIndexOutOfRange {
                got: hdr.symbol_index,
                total: hdr.total_symbols,
            });
        }
        if u64::from(hdr.block_start) + u64::from(hdr.block_count) > u64::from(hdr.total_blocks) {
            return Err(FileError::BlockRangeExceedsTotal {
                block_start: hdr.block_start,
                block_count: hdr.block_count,
                total_blocks: hdr.total_blocks,
            });
        }

        self.ensure_file_metadata(hdr)?;

        let normalized_blocks = normalize_symbol_blocks(decoded, hdr.file_size, hdr.total_blocks);
        if let Some(existing) = self.symbols.get(&hdr.symbol_index) {
            if existing.block_start != hdr.block_start || existing.block_count != hdr.block_count {
                return Err(FileError::DuplicateSymbol {
                    symbol_index: hdr.symbol_index,
                    divergent: true,
                });
            }
            return self.merge_duplicate_symbol(hdr.symbol_index, *existing, normalized_blocks);
        }

        self.ensure_no_overlapping_blocks(hdr.symbol_index, hdr.block_start, hdr.block_count)?;

        self.insert_symbol(
            hdr.symbol_index,
            StoredSymbol {
                block_start: hdr.block_start,
                block_count: hdr.block_count,
            },
            normalized_blocks,
        );
        Ok(())
    }

    /// Finalize the reassembly and return the original file bytes.
    ///
    /// # Errors
    /// - `MissingBlocks` if any block was not collected or was lost.
    /// - `GlobalHashMismatch` if the rebuilt file's BLAKE3-256 hash does not
    ///   match the hash recorded in the headers.
    ///
    pub fn finalize(self) -> Result<Vec<u8>, FileError> {
        let finalized = self.finalize_report()?;
        match finalized.hash_status {
            HashStatus::Verified => Ok(finalized.bytes),
            HashStatus::Mismatch { expected, got } => {
                Err(FileError::GlobalHashMismatch { expected, got })
            }
        }
    }

    /// Finalize the reassembly while preserving the forensic result when only
    /// the global hash diverges.
    ///
    /// # Errors
    /// - `MissingBlocks` if any block was not collected or was lost.
    pub fn finalize_report(self) -> Result<FinalizedFile, FileError> {
        let metadata = self.metadata.ok_or(FileError::MissingBlocks {
            ranges: vec![(0, 0)],
        })?;

        let mut missing: Vec<(u32, u32)> = Vec::new();
        let mut current_range: Option<(u32, u32)> = None;
        for idx in 0..metadata.total_blocks {
            let missing_this = !matches!(self.blocks.get(&idx), Some(Some(_)));
            if missing_this {
                current_range = Some(match current_range {
                    Some((start, end)) if end + 1 == idx => (start, idx),
                    Some(prev) => {
                        missing.push(prev);
                        (idx, idx)
                    }
                    None => (idx, idx),
                });
            } else if let Some(prev) = current_range.take() {
                missing.push(prev);
            }
        }
        if let Some(prev) = current_range {
            missing.push(prev);
        }
        if !missing.is_empty() {
            return Err(FileError::MissingBlocks { ranges: missing });
        }

        let mut out = Vec::with_capacity(usize::try_from(metadata.file_size).unwrap_or(usize::MAX));
        for idx in 0..metadata.total_blocks {
            let entry =
                self.blocks
                    .get(&idx)
                    .and_then(Option::as_ref)
                    .ok_or(FileError::MissingBlocks {
                        ranges: vec![(idx, idx)],
                    })?;
            let expected_raw_size =
                expected_block_size(metadata.file_size, metadata.total_blocks, idx);
            let raw = decode_block(entry, idx, expected_raw_size).map_err(|_| {
                FileError::MissingBlocks {
                    ranges: vec![(idx, idx)],
                }
            })?;
            out.extend_from_slice(&raw);
        }

        out.truncate(usize::try_from(metadata.file_size).unwrap_or(usize::MAX));

        let actual_hash: [u8; 32] = *blake3::hash(&out).as_bytes();
        let hash_status = if actual_hash == metadata.global_hash {
            HashStatus::Verified
        } else {
            HashStatus::Mismatch {
                expected: metadata.global_hash,
                got: actual_hash,
            }
        };

        Ok(FinalizedFile {
            bytes: out,
            hash_status,
        })
    }

    fn ensure_file_metadata(
        &mut self,
        header: &crate::format::SymbolHeader,
    ) -> Result<(), FileError> {
        match self.metadata {
            None => {
                self.metadata = Some(ReassemblyMetadata {
                    file_id: header.file_id,
                    file_size: header.file_size,
                    total_blocks: header.total_blocks,
                    total_symbols: header.total_symbols,
                    global_hash: header.global_hash,
                });
            }
            Some(existing) => {
                if existing.file_id != header.file_id {
                    return Err(FileError::SymbolFileIdMismatch {
                        expected: existing.file_id,
                        got: header.file_id,
                    });
                }
                if existing.file_size != header.file_size {
                    return Err(FileError::InconsistentFileMetadata { field: "file_size" });
                }
                if existing.total_blocks != header.total_blocks {
                    return Err(FileError::InconsistentFileMetadata {
                        field: "total_blocks",
                    });
                }
                if existing.total_symbols != header.total_symbols {
                    return Err(FileError::InconsistentFileMetadata {
                        field: "total_symbols",
                    });
                }
                if existing.global_hash != header.global_hash {
                    return Err(FileError::InconsistentFileMetadata {
                        field: "global_hash",
                    });
                }
            }
        }
        Ok(())
    }

    fn merge_duplicate_symbol(
        &mut self,
        symbol_index: u16,
        existing: StoredSymbol,
        normalized_blocks: Vec<Option<BlockEntry>>,
    ) -> Result<(), FileError> {
        if normalized_blocks.len() != usize::from(existing.block_count) {
            return Err(FileError::DuplicateSymbol {
                symbol_index,
                divergent: true,
            });
        }

        let mut divergent = false;
        for (i, incoming) in normalized_blocks.into_iter().enumerate() {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "block_count <= MAX_BLOCK_COUNT fits u32"
            )]
            let global_idx = existing.block_start + i as u32;
            let entry = self.blocks.entry(global_idx).or_insert(None);
            match (entry.as_ref(), incoming) {
                (Some(existing_block), Some(incoming_block))
                    if existing_block != &incoming_block =>
                {
                    divergent = true;
                }
                (None, Some(incoming_block)) => {
                    *entry = Some(incoming_block);
                }
                _ => {}
            }
        }

        if divergent {
            return Err(FileError::DuplicateSymbol {
                symbol_index,
                divergent: true,
            });
        }

        Ok(())
    }

    fn insert_symbol(
        &mut self,
        symbol_index: u16,
        stored_symbol: StoredSymbol,
        normalized_blocks: Vec<Option<BlockEntry>>,
    ) {
        self.symbols.insert(symbol_index, stored_symbol);
        for (i, stored) in normalized_blocks.into_iter().enumerate() {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "block_count <= MAX_BLOCK_COUNT fits u32"
            )]
            let global_idx = stored_symbol.block_start + i as u32;
            self.blocks.insert(global_idx, stored);
        }
    }

    fn ensure_no_overlapping_blocks(
        &self,
        symbol_index: u16,
        block_start: u32,
        block_count: u16,
    ) -> Result<(), FileError> {
        for i in 0..block_count {
            let block_index = block_start + u32::from(i);
            if self.blocks.contains_key(&block_index) {
                return Err(FileError::OverlappingBlockRange {
                    symbol_index,
                    block_index,
                });
            }
        }
        Ok(())
    }
}

fn validate_header_invariants(header: &crate::format::SymbolHeader) -> Result<(), FileError> {
    if header.file_size == 0 {
        return Err(FileError::InconsistentFileMetadata { field: "file_size" });
    }
    if header.total_blocks == 0
        || header.total_blocks != expected_total_blocks(header.file_size)
        || header.total_blocks > MAX_TOTAL_BLOCKS
    {
        return Err(FileError::InconsistentFileMetadata {
            field: "total_blocks",
        });
    }
    if header.total_symbols == 0 || header.total_symbols > MAX_TOTAL_SYMBOLS {
        return Err(FileError::InconsistentFileMetadata {
            field: "total_symbols",
        });
    }
    if header.block_count == 0 || header.block_count > MAX_BLOCK_COUNT {
        return Err(FileError::InconsistentFileMetadata {
            field: "block_count",
        });
    }
    Ok(())
}

fn normalize_symbol_blocks(
    decoded: DecodedSymbol,
    file_size: u64,
    total_blocks: u32,
) -> Vec<Option<BlockEntry>> {
    let header = decoded.header;
    let mut normalized = vec![None; usize::from(header.block_count)];
    for (i, block_result) in decoded.blocks.into_iter().enumerate() {
        if i >= normalized.len() {
            break;
        }
        if let Ok(entry) = block_result {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "block_count <= MAX_BLOCK_COUNT fits u32"
            )]
            let global_idx = header.block_start + i as u32;
            let expected_raw_size = expected_block_size(file_size, total_blocks, global_idx);
            if decode_block(&entry, global_idx, expected_raw_size).is_ok() {
                normalized[i] = Some(entry);
            }
        }
    }
    normalized
}

fn expected_total_blocks(file_size: u64) -> u32 {
    let block_size = BLOCK_SIZE_RAW as u64;
    u32::try_from(file_size.div_ceil(block_size)).unwrap_or(u32::MAX)
}

fn expected_block_size(file_size: u64, total_blocks: u32, block_index: u32) -> u16 {
    if block_index + 1 < total_blocks {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "BLOCK_SIZE_RAW is 8192 and fits u16"
        )]
        return BLOCK_SIZE_RAW as u16;
    }

    let full_prefix = u64::from(total_blocks.saturating_sub(1)) * BLOCK_SIZE_RAW as u64;
    u16::try_from(file_size.saturating_sub(full_prefix)).unwrap_or(u16::MAX)
}
