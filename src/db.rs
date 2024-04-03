use std::{pin::Pin, error::Error as StdError, fmt};    
use anyhow::{Result, Error};
use sea_orm::{
    Database,
    DatabaseConnection,
    ConnectOptions,
    DatabaseTransaction,
    TransactionError,
    TransactionTrait,
};
use futures::{Future, FutureExt};    

pub async fn connect() -> Result<DatabaseConnection> {
    let conn_opt = ConnectOptions::new("sqlite:db/main.db");
    let conn = Database::connect(conn_opt).await?;
    Ok(conn)
}

pub async fn transaction<T>(    
    db: &DatabaseConnection,    
    transaction_fn: impl for<'trx> FnOnce(&'trx DatabaseTransaction) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'trx>> + Send + 'static,    
) -> Result<T>    
    where    
        T: Send,    
{    
    #[derive(Debug)]    
    struct WrappedError(Error);    
    impl StdError for WrappedError { }    
    impl fmt::Display for WrappedError {    
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {    
            write!(f, "{}", self.0)    
        }    
    }    
    
    let r = db.transaction::<_, T, WrappedError>(|trx| {    
        async move {    
            transaction_fn(trx).await.map_err(WrappedError)    
        }.boxed()    
    }).await;    
    
    match r {    
        Ok(r) => Ok(r),    
        Err(err) => match err {    
            TransactionError::Connection(err) => {    
                Err(err.into())    
            },    
            TransactionError::Transaction(err) => {    
                Err(err.0)    
            },    
        },    
    }    
}    

