use std::{collections::HashMap, sync::{Arc, Weak}};
use anyhow::{anyhow, Result};
use sea_orm::{
    Database,
    ConnectOptions,
    DatabaseTransaction,
    TransactionTrait,
    ActiveModelTrait,
    EntityTrait,
    ActiveValue::Set,
};
use clap::Parser;
use actix_web::{guard, web, App, HttpServer, HttpResponse};
use async_graphql::{extensions, Object, EmptyMutation, EmptySubscription, Schema, Context, http::{playground_source, GraphQLPlaygroundConfig}};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};

mod entity;

use entity::post;
use entity::author;

#[derive(Debug, Parser)]
struct Args {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Debug, Parser)]
enum SubCommand {
    PrepareDummyData,
    HttpServer { hostname: String, port: u16 },
}

#[derive(Debug)]
struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn hello(&self) -> &'static str {
        "Hello, graphql!"
    }

    async fn posts(&self, ctx: &Context<'_>) -> Result<Vec<post::Model>> {
        let trx = ctx.data::<Weak<DatabaseTransaction>>().map_err(|err| anyhow!("no transaction: {:?}", err))?
            .upgrade().ok_or_else(|| anyhow!("transaction is already dropped"))?;

        let posts = post::Entity::find().all(trx.as_ref()).await?;
        Ok(posts)
    }

    async fn authors(&self, ctx: &Context<'_>) -> Result<Vec<author::Model>> {
        let trx = ctx.data::<Weak<DatabaseTransaction>>().map_err(|err| anyhow!("no transaction: {:?}", err))?
            .upgrade().ok_or_else(|| anyhow!("transaction is already dropped"))?;

        let authors = author::Entity::find().all(trx.as_ref()).await?;
        Ok(authors)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    match args.subcmd {
        SubCommand::PrepareDummyData => {
            let opt = ConnectOptions::new("sqlite:db/main.db");
            let conn = Database::connect(opt).await?;

            let trx = conn.begin().await?;
            match prepare_dummy_data(&trx).await {
                Ok(_) => trx.commit().await?,
                Err(e) => {
                    trx.rollback().await?;
                    return Err(e);
                }
            }
        },
        SubCommand::HttpServer { hostname, port } => {
            HttpServer::new(|| {
                let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
                    .extension(extensions::Logger)
                    .finish();

                App::new()
                    .service(web::resource("/").guard(guard::Get()).to(hello))
                    .service(
                        web::resource("/graphql")
                            .app_data(web::Data::new(schema))
                            .guard(guard::Post()).to(handle_graphql)
                    )
                    .service(web::resource("/playground").guard(guard::Get()).to(graphql_playgound))
            }).bind((hostname, port))?.run().await?;
        },
    }

    Ok(())
}

async fn hello() -> &'static str {
    "Hello, world!"
}

async fn graphql_playgound() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

async fn handle_graphql(schema: web::Data<Schema<QueryRoot, EmptyMutation, EmptySubscription>>, req: GraphQLRequest) -> GraphQLResponse {
    let req = req.into_inner();

    fn err_msg_to_res(msg: String) -> async_graphql::Response {
        let server_error = async_graphql::ServerError::new(msg, None);
        async_graphql::Response::from_errors(vec![server_error])
    }

    let conn_opt = ConnectOptions::new("sqlite:db/main.db");
    let conn = match Database::connect(conn_opt).await {
        Ok(conn) => conn,
        Err(err) => return err_msg_to_res(err.to_string()).into(),
    };

    let trx = match conn.begin().await {
        Ok(trx) => trx,
        Err(err) => return err_msg_to_res(err.to_string()).into(),
    };
    let trx = Arc::new(trx);
    let res = schema.execute(req.data(Arc::downgrade(&trx))).await;

    let trx = Arc::try_unwrap(trx).expect("only one reference to the transaction should exist");
    if res.is_err() {
        let _ = trx.rollback().await;
        return res.into();
    }

    match trx.commit().await {
        Ok(_) => {},
        Err(err) => return err_msg_to_res(err.to_string()).into(),
    }
    res.into()
}

async fn prepare_dummy_data(trx: &DatabaseTransaction) -> Result<()> {
    let authors = vec![
        "Alice", "Bob", "Carol", "Dave", "Eve",
    ];
    let posts = vec![
        ("Hello", "Hello, world!", "Alice"),
        ("SeaORM", "SeaORM is an async & dynamic ORM for Rust.", "Alice"),
        ("Rust", "Rust is a systems programming language.", "Bob"),
        ("SQLite", "SQLite is a C-language library that implements a small, fast, self-contained, high-reliability, full-featured, SQL database engine.", "Carol"),
        ("PostgreSQL", "PostgreSQL is a powerful, open source object-relational database system.", "Dave"),
        ("MySQL", "MySQL is an open-source relational database management system.", "Eve"),
    ];

    let mut name_author_map = HashMap::new();
    for author in authors {
        let author = author::ActiveModel {
            name: Set(author.to_owned()),
            ..Default::default()
        };
        let author = author.insert(trx).await?;
        name_author_map.insert(author.name.clone(), author);
    }
    for (title, text, author) in posts {
        let author = name_author_map.get(author).unwrap();
        let post = post::ActiveModel {
            author_id: Set(author.id),
            title: Set(title.to_owned()),
            text: Set(text.to_owned()),
            ..Default::default()
        };
        post.insert(trx).await?;
    }
    Ok(())
}

