use hex::{decode, encode};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self.value {
            0..=0xFC => vec![self.value as u8],
            0xFD..=0xFFFF => {
                let mut v = vec![0xFD];
                v.extend_from_slice(&(self.value as u16).to_le_bytes());
                v
            }
            0x10000..=0xFFFFFFFF => {
                let mut v = vec![0xFE];
                v.extend_from_slice(&(self.value as u32).to_le_bytes());
                v
            }
            _ => {
                let mut v = vec![0xFF];
                v.extend_from_slice(&self.value.to_le_bytes());
                v
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }
        match bytes[0] {
            n @ 0x00..=0xFC => Ok((Self::new(n as u64), 1)),
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let mut arr = [0u8; 2];
                arr.copy_from_slice(&bytes[1..3]);
                Ok((Self::new(u16::from_le_bytes(arr) as u64), 3))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let mut arr = [0u8; 4];
                arr.copy_from_slice(&bytes[1..5]);
                Ok((Self::new(u32::from_le_bytes(arr) as u64), 5))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[1..9]);
                Ok((Self::new(u64::from_le_bytes(arr)), 9))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&encode(self.0))
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = decode(&s).map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Invalid Txid length"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Txid(arr))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.txid.0.to_vec();
        v.extend_from_slice(&self.vout.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[0..32]);
        let mut vout = [0u8; 4];
        vout.copy_from_slice(&bytes[32..36]);
        Ok((OutPoint::new(txid, u32::from_le_bytes(vout)), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = CompactSize::new(self.bytes.len() as u64).to_bytes();
        v.extend_from_slice(&self.bytes);
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (len_prefix, consumed) = CompactSize::from_bytes(bytes)?;
        let total_len = consumed + (len_prefix.value as usize);
        if bytes.len() < total_len {
            return Err(BitcoinError::InsufficientBytes);
        }
        let data = bytes[consumed..total_len].to_vec();
        Ok((Self::new(data), total_len))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.previous_output.to_bytes();
        v.extend_from_slice(&self.script_sig.to_bytes());
        v.extend_from_slice(&self.sequence.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (outpoint, oconsumed) = OutPoint::from_bytes(bytes)?;
        let (script_sig, sconsumed) = Script::from_bytes(&bytes[oconsumed..])?;
        let total = oconsumed + sconsumed;
        if bytes.len() < total + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut seq = [0u8; 4];
        seq.copy_from_slice(&bytes[total..total + 4]);
        Ok((
            Self::new(outpoint, script_sig, u32::from_le_bytes(seq)),
            total + 4,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.version.to_le_bytes().to_vec();
        v.extend_from_slice(&CompactSize::new(self.inputs.len() as u64).to_bytes());
        for input in &self.inputs {
            v.extend_from_slice(&input.to_bytes());
        }
        v.extend_from_slice(&self.lock_time.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let version = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let (cs, cconsumed) = CompactSize::from_bytes(&bytes[4..])?;
        let mut inputs = vec![];
        let mut offset = 4 + cconsumed;
        for _ in 0..cs.value {
            let (input, consumed) = TransactionInput::from_bytes(&bytes[offset..])?;
            inputs.push(input);
            offset += consumed;
        }
        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        Ok((Self::new(version, inputs, lock_time), offset + 4))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        for input in &self.inputs {
            writeln!(
                f,
                "Previous Output Txid: {}",
                encode(input.previous_output.txid.0)
            )?;
            writeln!(f, "Previous Output Vout: {}", input.previous_output.vout)?;
            writeln!(
                f,
                "Script Sig ({} bytes): {:02X?}",
                input.script_sig.len(),
                input.script_sig.bytes
            )?;
            writeln!(f, "Sequence: {:08X}", input.sequence)?;
        }
        writeln!(f, "Lock Time: {}", self.lock_time)
    }
}
