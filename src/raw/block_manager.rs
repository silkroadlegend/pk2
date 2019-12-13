use std::collections::HashMap;
use std::io;
use std::path::{Component, Path};

use super::block_chain::PackBlockChain;
use super::entry::{DirectoryEntry, PackEntry};
use super::ChainIndex;
use crate::constants::PK2_ROOT_BLOCK;
use crate::error::{Error, Pk2Result};
use crate::Blowfish;

/// Simple BlockManager backed by a hashmap.
pub struct BlockManager {
    chains: HashMap<ChainIndex, PackBlockChain, NoHashHasherBuilder>,
}

impl BlockManager {
    /// Parses the complete index of a pk2 file
    pub fn new<F: io::Read + io::Seek>(bf: Option<&Blowfish>, mut file: F) -> Pk2Result<Self> {
        let mut chains = HashMap::with_capacity_and_hasher(32, NoHashHasherBuilder);
        let mut offsets = vec![PK2_ROOT_BLOCK];
        while let Some(offset) = offsets.pop() {
            let block_chain = Self::read_chain_from_file_at(bf, &mut file, offset)?;
            // put all folder offsets of this chain into the stack to parse them next
            offsets.extend(block_chain.entries().filter_map(|entry| {
                entry
                    .as_directory()
                    .filter(|dir| dir.is_normal_link())
                    .map(DirectoryEntry::children_position)
            }));
            // TODO collisions
            chains.insert(offset, block_chain);
        }
        Ok(BlockManager { chains })
    }

    /// Reads a [`PackBlockChain`] from the given file at the specified offset.
    /// Note: FIXME Can potentially end up in a neverending loop with a
    /// specially crafted file.
    fn read_chain_from_file_at<F: io::Read + io::Seek>(
        bf: Option<&Blowfish>,
        mut file: F,
        ChainIndex(mut offset): ChainIndex,
    ) -> Pk2Result<PackBlockChain> {
        let mut blocks = Vec::new();
        loop {
            let block = crate::io::read_block_at(bf, &mut file, offset)?;
            let nc = block.entries().last().and_then(PackEntry::next_block);
            blocks.push(block);
            match nc {
                Some(nc) => offset = nc.get(),
                None => break Ok(PackBlockChain::from_blocks(blocks)),
            }
        }
    }

    #[inline]
    pub fn get(&self, chain: ChainIndex) -> Option<&PackBlockChain> {
        self.chains.get(&chain)
    }

    #[inline]
    pub fn get_mut(&mut self, chain: ChainIndex) -> Option<&mut PackBlockChain> {
        self.chains.get_mut(&chain)
    }

    #[inline]
    pub fn insert(&mut self, chain: ChainIndex, block: PackBlockChain) {
        self.chains.insert(chain, block);
    }

    /// Resolves a path from the specified chain to a parent chain and the entry
    /// Returns Ok(None) if the path is empty, otherwise (blockchain,
    /// entry_index, entry)
    pub fn resolve_path_to_entry_and_parent(
        &self,
        current_chain: ChainIndex,
        path: &Path,
    ) -> Pk2Result<(&PackBlockChain, usize, &PackEntry)> {
        let mut components = path.components();

        if let Some(c) = components.next_back() {
            let parent_index =
                self.resolve_path_to_block_chain_index_at(current_chain, components.as_path())?;
            let parent = &self.chains[&parent_index];
            let name = c.as_os_str().to_str();
            parent
                .entries()
                .enumerate()
                .find(|(_, entry)| entry.name() == name)
                .ok_or(Error::NotFound)
                .map(|(idx, entry)| (parent, idx, entry))
        } else {
            Err(Error::InvalidPath)
        }
    }

    /// Resolves a path to a [`PackBlockChain`] index starting from the given
    /// blockchain returning the index of the last blockchain.
    pub fn resolve_path_to_block_chain_index_at(
        &self,
        current_chain: ChainIndex,
        path: &Path,
    ) -> Pk2Result<ChainIndex> {
        path.components().try_fold(current_chain, |idx, component| {
            let comp = component
                .as_os_str()
                .to_str()
                .ok_or(Error::NonUnicodePath)?;
            self.chains
                .get(&idx)
                .ok_or(Error::InvalidChainIndex)
                .and_then(|chain| chain.find_block_chain_index_of(comp))
        })
    }

    /// Traverses the path until it hits a non-existent component and returns
    /// the rest of the path as a peekable as well as the chain index of the
    /// last valid part.
    pub fn validate_dir_path_until<'p>(
        &self,
        mut chain: ChainIndex,
        path: &'p Path,
    ) -> Pk2Result<(ChainIndex, std::iter::Peekable<std::path::Components<'p>>)> {
        let mut components = path.components().peekable();
        while let Some(component) = components.peek() {
            let name = component
                .as_os_str()
                .to_str()
                .ok_or(Error::NonUnicodePath)?;
            match self
                .chains
                .get(&chain)
                .ok_or(Error::InvalidChainIndex)
                .and_then(|chain| chain.find_block_chain_index_of(name))
            {
                Ok(i) => chain = i,
                Err(Error::NotFound) => {
                    if component == &Component::ParentDir {
                        // lies outside of the archive
                        return Err(Error::InvalidPath);
                    } else {
                        // found a non-existent part, we are done here
                        break;
                    }
                }
                Err(e) => return Err(e),
            }
            let _ = components.next();
        }
        Ok((chain, components))
    }
}

#[derive(Default)]
struct NoHashHasherBuilder;
impl std::hash::BuildHasher for NoHashHasherBuilder {
    type Hasher = NoHashHasher;
    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        NoHashHasher(0)
    }
}

struct NoHashHasher(u64);
impl std::hash::Hasher for NoHashHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, _: &[u8]) {
        panic!("ChainIndex has been hashed wrong. This is a bug!");
    }

    #[inline]
    fn write_u64(&mut self, chain: u64) {
        self.0 = chain;
    }
}
