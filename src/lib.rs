/// ! Implementation of [multihash](https://github.com/multiformats/multihash) in Rust.

extern crate arrayvec;
extern crate ring;
extern crate tiny_keccak;
extern crate fnv;

pub mod algos;

use std::collections::hash_map;
use std::convert::From;
use std::hash::{Hash, Hasher};
use std::fmt::Debug;
use std::any::TypeId;
use fnv::FnvHashMap;

#[derive(Debug, Clone)]
pub struct Registry {
    by_code: FnvHashMap<u64, DynAlgo>,
    by_algo: FnvHashMap<DynAlgo, u64>,
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            by_code: FnvHashMap::default(),
            by_algo: FnvHashMap::default(),
        }
    }

    pub fn register(&mut self, code: u64, algo: DynAlgo) {
        self.by_code.insert(code, algo.clone());
        self.by_algo.insert(algo, code);
    }

    pub fn unregister(&mut self, code: u64) {
        if let Some(algo) = self.by_code.remove(&code) {
            self.by_algo.remove(&algo);
        }
    }

    pub fn iter<'a>(&'a self) -> hash_map::Iter<'a, u64, DynAlgo> {
        self.by_code.iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> hash_map::IterMut<'a, u64, DynAlgo> {
        self.by_code.iter_mut()
    }

    pub fn by_code(&self, code: u64) -> Option<&DynAlgo> {
        self.by_code.get(&code)
    }

    pub fn by_algo(&self, algo: DynAlgo) -> Option<u64> {
        self.by_algo.get(&algo).map(|c| *c)
    }

    pub fn serialize(&self, input: &DynMultihash, output: &mut Vec<u8>) {
        let code = self.by_algo(input.algo()).unwrap();
        let slice = input.as_ref();
        output.push(code as u8);
        output.push(slice.len() as u8);
        output.extend_from_slice(slice);
    }

    pub fn deserialize<'a>(&self, input: &'a [u8]) -> (DynMultihash, &'a [u8]) {
        let code = input[0] as u64;
        let len = input[1] as usize;
        let algo = self.by_code(code).unwrap();
        let mh = algo.inner.in_deserialize(&input[2..len]);
        (mh, &input[..len + 2])
    }
}

impl Default for Registry {
    fn default() -> Registry {
        let mut reg = Registry::new();
        reg.register(0x11, algos::SHA1.into());

        reg.register(0x12, algos::SHA2_256.into());
        reg.register(0x13, algos::SHA2_512.into());

        reg.register(0x17, algos::SHA3_224.into());
        reg.register(0x16, algos::SHA3_256.into());
        reg.register(0x15, algos::SHA3_384.into());
        reg.register(0x14, algos::SHA3_512.into());

        reg.register(0x18, algos::SHAKE_128.into());
        reg.register(0x19, algos::SHAKE_256.into());

        reg.register(0x1A, algos::Keccak_224.into());
        reg.register(0x1B, algos::Keccak_256.into());
        reg.register(0x1C, algos::Keccak_384.into());
        reg.register(0x1D, algos::Keccak_512.into());

        reg
    }
}

pub trait Digest: 'static + InnerDigest + Clone {
    type Algo: Algo;

    fn algo(&self) -> Self::Algo;
    fn update(&mut self, input: &[u8]);
    fn finish(self) -> <Self::Algo as Algo>::Hash;
}

#[doc(hidden)]
pub trait InnerDigest: Send + Sync {
    fn in_algo(&self) -> DynAlgo;
    fn in_update(&mut self, input: &[u8]);
    fn in_finish(self: Box<Self>) -> DynMultihash;
}

impl<T: Digest> InnerDigest for T {
    fn in_algo(&self) -> DynAlgo {
        self.algo().into()
    }

    fn in_update(&mut self, input: &[u8]) {
        self.update(input);
    }

    fn in_finish(self: Box<Self>) -> DynMultihash {
        self.finish().into()
    }
}

pub struct DynDigest {
    inner: Box<InnerDigest>,
}

impl DynDigest {
    pub fn algo(&self) -> DynAlgo {
        self.inner.in_algo()
    }

    pub fn update(&mut self, input: &[u8]) {
        self.inner.in_update(input)
    }

    pub fn finish(self) -> DynMultihash {
        self.inner.in_finish()
    }
}

impl<T: Digest> From<T> for DynDigest {
    fn from(digest: T) -> DynDigest {
        DynDigest { inner: Box::new(digest) as Box<InnerDigest> }
    }
}


pub trait Algo: 'static + InnerAlgo + Clone + Eq {
    type Hash: Multihash;
    type Digest: Digest;

    fn digest(&self) -> Self::Digest;
    fn deserialize(&self, input: &[u8]) -> Self::Hash;
    fn max_len() -> usize;

    fn additional_state(&self) -> &[u8] {
        &[]
    }
}

#[doc(hidden)]
pub trait InnerAlgo: Debug + Send + Sync {
    fn in_digest(&self) -> DynDigest;
    fn in_deserialize(&self, input: &[u8]) -> DynMultihash;
    fn in_max_len(&self) -> usize;
    fn in_type_id(&self) -> TypeId;
    fn in_clone(&self) -> DynAlgo;
    fn in_additional_state(&self) -> &[u8];
}

impl<T: Algo> InnerAlgo for T {
    fn in_digest(&self) -> DynDigest {
        self.digest().into()
    }

    fn in_deserialize(&self, input: &[u8]) -> DynMultihash {
        self.deserialize(input).into()
    }

    fn in_max_len(&self) -> usize {
        T::max_len()
    }

    fn in_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn in_clone(&self) -> DynAlgo {
        self.clone().into()
    }

    fn in_additional_state(&self) -> &[u8] {
        self.additional_state()
    }
}

#[derive(Debug)]
pub struct DynAlgo {
    inner: Box<InnerAlgo>,
}

impl DynAlgo {
    pub fn digest(&self) -> DynDigest {
        self.inner.in_digest()
    }

    pub fn deserialize(&self, input: &[u8]) -> DynMultihash {
        self.inner.in_deserialize(input)
    }

    pub fn max_len(&self) -> usize {
        self.inner.in_max_len()
    }
}

impl Hash for DynAlgo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.in_type_id().hash(state);
        state.write(self.inner.in_additional_state());
    }
}

impl PartialEq<DynAlgo> for DynAlgo {
    fn eq(&self, other: &DynAlgo) -> bool {
        self.inner.in_type_id() == other.inner.in_type_id() &&
        self.inner.in_additional_state() == other.inner.in_additional_state()
    }
}

impl Eq for DynAlgo {}

impl Clone for DynAlgo {
    fn clone(&self) -> DynAlgo {
        self.inner.in_clone()
    }
}

impl<T: Algo> From<T> for DynAlgo {
    fn from(algo: T) -> DynAlgo {
        DynAlgo { inner: Box::new(algo) as Box<InnerAlgo> }
    }
}


pub trait Multihash: 'static + AsRef<[u8]> + InnerMultihash + Clone {
    type Algo: Algo;

    fn algo(&self) -> Self::Algo;

    fn additional_state(&self) -> &[u8] {
        &[]
    }
}

#[doc(hidden)]
pub trait InnerMultihash: AsRef<[u8]> + Debug + Send + Sync {
    fn in_algo(&self) -> DynAlgo;
    fn in_clone(&self) -> DynMultihash;
    fn in_additional_state(&self) -> &[u8];
}

impl<T: Multihash> InnerMultihash for T {
    fn in_algo(&self) -> DynAlgo {
        self.algo().into()
    }

    fn in_clone(&self) -> DynMultihash {
        self.clone().into()
    }

    fn in_additional_state(&self) -> &[u8] {
        self.additional_state()
    }
}


#[derive(Debug)]
pub struct DynMultihash {
    inner: Box<InnerMultihash>,
}

impl DynMultihash {
    pub fn algo(&self) -> DynAlgo {
        self.inner.in_algo()
    }
}

impl AsRef<[u8]> for DynMultihash {
    fn as_ref(&self) -> &[u8] {
        (*self.inner).as_ref()
    }
}

impl Hash for DynMultihash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.algo(), state);
        self.as_ref().hash(state);
        state.write(self.inner.in_additional_state());
    }
}

impl PartialEq<DynMultihash> for DynMultihash {
    fn eq(&self, other: &DynMultihash) -> bool {
        self.inner.in_algo() == other.inner.in_algo() && self.as_ref() == other.as_ref() &&
        self.inner.in_additional_state() == other.inner.in_additional_state()
    }
}

impl Eq for DynMultihash {}

impl Clone for DynMultihash {
    fn clone(&self) -> DynMultihash {
        self.inner.in_clone()
    }
}

impl<T: Multihash> From<T> for DynMultihash {
    fn from(mh: T) -> DynMultihash {
        DynMultihash { inner: Box::new(mh) as Box<InnerMultihash> }
    }
}
