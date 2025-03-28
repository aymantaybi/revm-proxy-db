use std::{
    fs::File,
    io::{Read, Write},
};

use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{AccountInfo, Address, Bytecode, B256, U256},
    DatabaseRef,
};
use serde::de::DeserializeOwned;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum NewFetch {
    Basic {
        address: Address,
        account_info: AccountInfo,
    },
    Storage {
        address: Address,
        index: U256,
        value: U256,
    },
}

pub struct ProxyDB<ExtDB> {
    pub db: ExtDB,
    pub sender: Option<UnboundedSender<NewFetch>>,
}

impl<ExtDB> ProxyDB<ExtDB>
where
    ExtDB: DatabaseRef,
{
    pub fn new(db: ExtDB) -> Self {
        Self { db, sender: None }
    }
}

impl<ExtDB> DatabaseRef for ProxyDB<ExtDB>
where
    ExtDB: DatabaseRef,
{
    #[doc = " The database error type."]
    type Error = ExtDB::Error;

    #[doc = " Get basic account information."]
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let account_info = self.db.basic_ref(address)?;
        if let Some(account_info) = &account_info {
            self.sender.as_ref().inspect(|sender| {
                let _ = sender.send(NewFetch::Basic {
                    address,
                    account_info: account_info.clone(),
                });
            });
        }
        Ok(account_info)
    }

    #[doc = " Get account code by its hash."]
    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.db.code_by_hash_ref(code_hash)
    }

    #[doc = " Get storage value of address at index."]
    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let value = self.db.storage_ref(address, index)?;
        self.sender.as_ref().inspect(|sender| {
            let _ = sender.send(NewFetch::Storage {
                address,
                index,
                value,
            });
        });
        Ok(value)
    }

    #[doc = " Get block hash by block number."]
    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.db.block_hash_ref(number)
    }
}

pub fn save_cache_db_to_file<ExtDB>(path: String, cache_db: &CacheDB<ExtDB>) -> eyre::Result<()> {
    let CacheDB {
        accounts,
        contracts,
        ..
    } = cache_db;
    let db = CacheDB {
        accounts: accounts.clone(),
        contracts: contracts.clone(),
        logs: Default::default(),
        block_hashes: Default::default(),
        db: EmptyDB::new(),
    };
    let json = serde_json::to_string(&db)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn load_cache_db_from_file<ExtDB>(path: String) -> eyre::Result<CacheDB<ExtDB>>
where
    ExtDB: DeserializeOwned,
{
    let mut file = File::open(path)?;
    let mut json = String::new();
    let _ = file.read_to_string(&mut json)?;
    let cache_db = serde_json::from_str::<CacheDB<ExtDB>>(&json)?;
    Ok(cache_db)
}
