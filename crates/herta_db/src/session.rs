//! A query's database identity is explicit and cannot silently leave its transaction.

use std::borrow::Cow;

use surrealdb::{
    engine::local::Db,
    method::{Query, Transaction},
};

use crate::DbClient;

#[derive(Clone, Copy)]
pub enum DbSession<'a> {
    Client(&'a DbClient),
    Transaction(&'a Transaction<Db>),
}

impl<'a> From<&'a DbClient> for DbSession<'a> {
    fn from(client: &'a DbClient) -> Self {
        Self::Client(client)
    }
}

impl<'a> From<&'a Transaction<Db>> for DbSession<'a> {
    fn from(transaction: &'a Transaction<Db>) -> Self {
        Self::Transaction(transaction)
    }
}

impl DbSession<'_> {
    pub fn query<'s>(&'s self, sql: impl Into<Cow<'s, str>>) -> Query<'s, Db> {
        match self {
            Self::Client(client) => client.inner().query(sql),
            Self::Transaction(transaction) => transaction.query(sql),
        }
    }

    pub fn is_transaction(&self) -> bool {
        matches!(self, Self::Transaction(_))
    }

    /// Preserve standalone DDL atomicity without beginning a second transaction
    /// when the caller already owns one.
    pub fn atomic_query(&self, sql: String) -> Query<'_, Db> {
        self.query(if self.is_transaction() {
            sql
        } else {
            format!("BEGIN TRANSACTION;\n{sql}\nCOMMIT TRANSACTION;")
        })
    }
}
